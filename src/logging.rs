//! 日志系统模块
//!
//! 提供结构化日志配置和管理功能

use log::LevelFilter;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 日志轮转策略
#[derive(Debug, Clone)]
pub enum LogRotation {
    /// 不轮转
    Never,
    /// 按大小轮转
    Size { max_size_mb: u64 },
    /// 按时间轮转
    Time { interval: Duration },
    /// 按大小和时间轮转
    SizeAndTime {
        max_size_mb: u64,
        interval: Duration,
    },
}

/// 日志配置结构
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// 日志级别
    pub level: LevelFilter,
    /// 日志文件路径（可选）
    pub file_path: Option<PathBuf>,
    /// 是否输出到控制台
    pub console: bool,
    /// 是否使用JSON格式
    pub json_format: bool,
    /// 日志轮转配置
    pub rotation: LogRotation,
    /// 最大保留文件数
    pub max_files: usize,
    /// 模块级别日志控制
    pub module_levels: HashMap<String, LevelFilter>,
    /// 是否启用性能指标日志
    pub enable_metrics: bool,
    /// 性能指标日志间隔
    pub metrics_interval: Duration,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: LevelFilter::Info,
            file_path: None,
            console: true,
            json_format: false,
            rotation: LogRotation::Size { max_size_mb: 100 },
            max_files: 10,
            module_levels: HashMap::new(),
            enable_metrics: true,
            metrics_interval: Duration::from_secs(300), // 5分钟
        }
    }
}

/// 性能指标收集器
#[derive(Debug, Clone)]
pub struct MetricsCollector {
    /// 指标数据
    metrics: Arc<Mutex<HashMap<String, MetricValue>>>,
    /// 最后收集时间
    last_collection: Arc<Mutex<Instant>>,
}

/// 指标值类型
#[derive(Debug, Clone)]
pub enum MetricValue {
    /// 计数器
    Counter(u64),
    /// 计量器
    Gauge(f64),
    /// 直方图
    Histogram {
        sum: f64,
        count: u64,
        buckets: Vec<(f64, u64)>,
    },
    /// 摘要
    Summary {
        sum: f64,
        count: u64,
        quantiles: Vec<(f64, f64)>,
    },
}

impl MetricsCollector {
    /// 创建新的指标收集器
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(Mutex::new(HashMap::new())),
            last_collection: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// 增加计数器
    pub fn increment_counter(&self, name: &str, value: u64) {
        let mut metrics = self.metrics.lock().unwrap();
        let entry = metrics
            .entry(name.to_string())
            .or_insert(MetricValue::Counter(0));
        if let MetricValue::Counter(ref mut count) = entry {
            *count += value;
        }
    }

    /// 设置计量器值
    pub fn set_gauge(&self, name: &str, value: f64) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.insert(name.to_string(), MetricValue::Gauge(value));
    }

    /// 记录直方图值
    pub fn record_histogram(&self, name: &str, value: f64) {
        let mut metrics = self.metrics.lock().unwrap();
        let entry = metrics
            .entry(name.to_string())
            .or_insert(MetricValue::Histogram {
                sum: 0.0,
                count: 0,
                buckets: vec![
                    (1.0, 0),
                    (5.0, 0),
                    (10.0, 0),
                    (50.0, 0),
                    (100.0, 0),
                    (f64::INFINITY, 0),
                ],
            });

        if let MetricValue::Histogram {
            ref mut sum,
            ref mut count,
            ref mut buckets,
        } = entry
        {
            *sum += value;
            *count += 1;

            for (bucket_le, bucket_count) in buckets.iter_mut() {
                if value <= *bucket_le {
                    *bucket_count += 1;
                }
            }
        }
    }

    /// 获取所有指标
    pub fn get_metrics(&self) -> HashMap<String, MetricValue> {
        self.metrics.lock().unwrap().clone()
    }

    /// 重置指标
    pub fn reset(&self) {
        let mut metrics = self.metrics.lock().unwrap();
        metrics.clear();
        *self.last_collection.lock().unwrap() = Instant::now();
    }
}

/// 日志系统管理器
pub struct LoggingSystem {
    /// 指标收集器
    metrics_collector: Option<Arc<MetricsCollector>>,
    /// 配置
    config: LogConfig,
}

impl LoggingSystem {
    /// 创建新的日志系统
    pub fn new(config: LogConfig) -> Self {
        let metrics_collector = if config.enable_metrics {
            Some(Arc::new(MetricsCollector::new()))
        } else {
            None
        };

        Self {
            metrics_collector,
            config,
        }
    }

    /// 初始化日志系统
    ///
    /// # 参数
    /// * `config` - 日志配置
    ///
    /// # 返回
    /// * `Result<LoggingSystem, anyhow::Error>` - 初始化结果
    pub fn setup_logging(config: LogConfig) -> anyhow::Result<Self> {
        let mut builder = env_logger::Builder::new();

        // 设置日志级别
        builder.filter_level(config.level);

        // 设置模块级别日志控制
        for (module, level) in &config.module_levels {
            builder.filter_module(module, *level);
        }

        // 设置日志格式
        if config.json_format {
            builder.format(|buf, record| {
                use std::io::Write;
                let json_log = json!({
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "level": record.level().to_string(),
                    "target": record.target(),
                    "message": record.args().to_string(),
                    "module": record.module_path(),
                    "file": record.file(),
                    "line": record.line(),
                });
                writeln!(buf, "{}", json_log)
            });
        } else {
            builder.format(|buf, record| {
                use std::io::Write;
                writeln!(
                    buf,
                    "{} [{}] {} - {} ({}:{})",
                    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                    record.level(),
                    record.target(),
                    record.args(),
                    record.file().unwrap_or("unknown"),
                    record.line().unwrap_or(0)
                )
            });
        }

        // 初始化日志系统
        builder.try_init()?;

        let system = Self::new(config.clone());

        // 启动指标收集任务
        if let Some(ref collector) = system.metrics_collector {
            let collector_clone = Arc::clone(collector);
            let interval = config.metrics_interval;
            tokio::spawn(async move {
                let mut interval_timer = tokio::time::interval(interval);
                loop {
                    interval_timer.tick().await;
                    Self::log_metrics(&collector_clone).await;
                }
            });
        }

        log::info!("日志系统初始化完成");
        log::debug!("日志配置: {:?}", config);

        Ok(system)
    }

    /// 获取指标收集器
    pub fn metrics_collector(&self) -> Option<Arc<MetricsCollector>> {
        self.metrics_collector.clone()
    }

    /// 记录审计日志
    pub fn audit_log(
        &self,
        operation: &str,
        user: Option<&str>,
        result: &str,
        details: Option<&str>,
    ) {
        let audit_entry = json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "type": "audit",
            "operation": operation,
            "user": user.unwrap_or("system"),
            "result": result,
            "details": details.unwrap_or(""),
        });

        if self.config.json_format {
            log::info!("{}", audit_entry);
        } else {
            log::info!(
                "AUDIT: {} by {} - {} ({})",
                operation,
                user.unwrap_or("system"),
                result,
                details.unwrap_or("")
            );
        }
    }

    /// 记录性能日志
    pub fn performance_log(
        &self,
        operation: &str,
        duration_ms: u64,
        success: bool,
        metadata: Option<&HashMap<String, String>>,
    ) {
        let perf_entry = json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "type": "performance",
            "operation": operation,
            "duration_ms": duration_ms,
            "success": success,
            "metadata": metadata.unwrap_or(&HashMap::new()),
        });

        if self.config.json_format {
            log::info!("{}", perf_entry);
        } else {
            log::info!(
                "PERF: {} - {}ms ({})",
                operation,
                duration_ms,
                if success { "SUCCESS" } else { "FAILED" }
            );
        }

        // 更新指标
        if let Some(ref collector) = self.metrics_collector {
            collector.record_histogram(&format!("{}_duration_ms", operation), duration_ms as f64);
            collector.increment_counter(&format!("{}_total", operation), 1);
            if success {
                collector.increment_counter(&format!("{}_success", operation), 1);
            } else {
                collector.increment_counter(&format!("{}_failed", operation), 1);
            }
        }
    }

    /// 记录健康状态日志
    pub fn health_status_log(
        &self,
        service_name: &str,
        status: &str,
        response_time_ms: u64,
        details: Option<&str>,
    ) {
        let health_entry = json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "type": "health_check",
            "service": service_name,
            "status": status,
            "response_time_ms": response_time_ms,
            "details": details.unwrap_or(""),
        });

        if self.config.json_format {
            log::info!("{}", health_entry);
        } else {
            log::info!(
                "HEALTH: {} - {} ({}ms) {}",
                service_name,
                status,
                response_time_ms,
                details.unwrap_or("")
            );
        }

        // 更新指标
        if let Some(ref collector) = self.metrics_collector {
            collector.record_histogram(
                &format!("health_check_{}_response_time", service_name),
                response_time_ms as f64,
            );
            collector.increment_counter(&format!("health_check_{}_total", service_name), 1);
            if status == "healthy" {
                collector.increment_counter(&format!("health_check_{}_success", service_name), 1);
            } else {
                collector.increment_counter(&format!("health_check_{}_failed", service_name), 1);
            }
        }
    }

    /// 记录通知日志
    pub fn notification_log(
        &self,
        notification_type: &str,
        recipient: &str,
        success: bool,
        error: Option<&str>,
    ) {
        let notification_entry = json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "type": "notification",
            "notification_type": notification_type,
            "recipient": recipient,
            "success": success,
            "error": error.unwrap_or(""),
        });

        if self.config.json_format {
            log::info!("{}", notification_entry);
        } else {
            log::info!(
                "NOTIFICATION: {} to {} - {} {}",
                notification_type,
                recipient,
                if success { "SUCCESS" } else { "FAILED" },
                error.unwrap_or("")
            );
        }

        // 更新指标
        if let Some(ref collector) = self.metrics_collector {
            collector.increment_counter(&format!("notification_{}_total", notification_type), 1);
            if success {
                collector
                    .increment_counter(&format!("notification_{}_success", notification_type), 1);
            } else {
                collector
                    .increment_counter(&format!("notification_{}_failed", notification_type), 1);
            }
        }
    }

    /// 记录指标日志
    async fn log_metrics(collector: &Arc<MetricsCollector>) {
        let metrics = collector.get_metrics();
        if metrics.is_empty() {
            return;
        }

        let metrics_entry = json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "type": "metrics",
            "metrics": metrics.iter().map(|(k, v)| {
                let value = match v {
                    MetricValue::Counter(c) => json!({"type": "counter", "value": c}),
                    MetricValue::Gauge(g) => json!({"type": "gauge", "value": g}),
                    MetricValue::Histogram { sum, count, .. } => json!({
                        "type": "histogram",
                        "sum": sum,
                        "count": count,
                        "avg": if *count > 0 { sum / (*count as f64) } else { 0.0 }
                    }),
                    MetricValue::Summary { sum, count, .. } => json!({
                        "type": "summary",
                        "sum": sum,
                        "count": count,
                        "avg": if *count > 0 { sum / (*count as f64) } else { 0.0 }
                    }),
                };
                (k, value)
            }).collect::<HashMap<_, _>>()
        });

        log::info!("{}", metrics_entry);

        // 重置计数器类型的指标
        collector.reset();
    }
}

/// 获取默认日志文件路径
pub fn get_default_log_path() -> PathBuf {
    PathBuf::from("/var/log/service-vitals/service-vitals.log")
}
