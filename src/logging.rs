//! 日志系统模块
//!
//! 提供结构化日志配置和管理功能

use log::LevelFilter;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing_subscriber::{
    fmt, prelude::*, registry, EnvFilter, Layer,
};
use tracing_log::LogTracer;

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
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
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
        // 设置 log crate 的日志转发到 tracing
        LogTracer::init().ok(); // 忽略错误，可能已经初始化过了

        // 创建环境过滤器
        let mut env_filter = EnvFilter::from_default_env()
            .add_directive(Self::convert_level_to_directive(config.level));

        // 添加模块级别过滤
        for (module, level) in &config.module_levels {
            let directive = format!("{}={}", module, Self::level_to_string(*level))
                .parse()
                .unwrap_or_else(|_| format!("{module}=info").parse().unwrap());
            env_filter = env_filter.add_directive(directive);
        }

        // 创建格式化层
        let fmt_layer = if config.json_format {
            fmt::layer()
                .json()
                .with_timer(fmt::time::ChronoUtc::rfc_3339())
                .with_file(true)
                .with_line_number(true)
                .boxed()
        } else {
            fmt::layer()
                .with_timer(fmt::time::ChronoUtc::rfc_3339())
                .with_ansi(true)
                .with_file(true)
                .with_line_number(true)
                .with_thread_ids(true)
                .with_thread_names(true)
                .boxed()
        };

        // 设置输出目标
        let result = if config.console {
            // 控制台输出
            registry()
                .with(env_filter)
                .with(fmt_layer)
                .try_init()
        } else if let Some(file_path) = &config.file_path {
            // 文件输出 (简单实现，不包含轮转)
            let file = std::fs::File::create(file_path)?;
            let file_layer = fmt::layer()
                .with_writer(file)
                .with_ansi(false)
                .with_file(true)
                .with_line_number(true);

            registry()
                .with(env_filter)
                .with(file_layer)
                .try_init()
        } else {
            // 默认控制台输出
            registry()
                .with(env_filter)
                .with(fmt_layer)
                .try_init()
        };

        // 忽略重复初始化错误
        if let Err(e) = result {
            tracing::warn!("日志系统可能已经初始化: {}", e);
        }

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

        tracing::info!("日志系统初始化完成");
        tracing::debug!("日志配置: {:?}", config);

        Ok(system)
    }

    /// 将 log::LevelFilter 转换为 tracing 的指令
    fn convert_level_to_directive(level: LevelFilter) -> tracing_subscriber::filter::Directive {
        use tracing_subscriber::filter::Directive;
        match level {
            LevelFilter::Off => "off".parse().unwrap(),
            LevelFilter::Error => Directive::from(tracing::Level::ERROR),
            LevelFilter::Warn => Directive::from(tracing::Level::WARN),
            LevelFilter::Info => Directive::from(tracing::Level::INFO),
            LevelFilter::Debug => Directive::from(tracing::Level::DEBUG),
            LevelFilter::Trace => Directive::from(tracing::Level::TRACE),
        }
    }

    /// 将 log::LevelFilter 转换为字符串
    fn level_to_string(level: LevelFilter) -> &'static str {
        match level {
            LevelFilter::Off => "off",
            LevelFilter::Error => "error",
            LevelFilter::Warn => "warn",
            LevelFilter::Info => "info",
            LevelFilter::Debug => "debug",
            LevelFilter::Trace => "trace",
        }
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
            tracing::info!("{audit_entry}");
        } else {
            tracing::info!(
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
            tracing::info!("{perf_entry}");
        } else {
            tracing::info!(
                "PERF: {} - {}ms ({})",
                operation,
                duration_ms,
                if success { "SUCCESS" } else { "FAILED" }
            );
        }

        // 更新指标
        if let Some(ref collector) = self.metrics_collector {
            collector.record_histogram(&format!("{operation}_duration_ms"), duration_ms as f64);
            collector.increment_counter(&format!("{operation}_total"), 1);
            if success {
                collector.increment_counter(&format!("{operation}_success"), 1);
            } else {
                collector.increment_counter(&format!("{operation}_failed"), 1);
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
            tracing::info!("{health_entry}");
        } else {
            tracing::info!(
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
                &format!("health_check_{service_name}_response_time"),
                response_time_ms as f64,
            );
            collector.increment_counter(&format!("health_check_{service_name}_total"), 1);
            if status == "healthy" {
                collector.increment_counter(&format!("health_check_{service_name}_success"), 1);
            } else {
                collector.increment_counter(&format!("health_check_{service_name}_failed"), 1);
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
            tracing::info!("{notification_entry}");
        } else {
            tracing::info!(
                "NOTIFICATION: {} to {} - {} {}",
                notification_type,
                recipient,
                if success { "SUCCESS" } else { "FAILED" },
                error.unwrap_or("")
            );
        }

        // 更新指标
        if let Some(ref collector) = self.metrics_collector {
            collector.increment_counter(&format!("notification_{notification_type}_total"), 1);
            if success {
                collector
                    .increment_counter(&format!("notification_{notification_type}_success"), 1);
            } else {
                collector.increment_counter(&format!("notification_{notification_type}_failed"), 1);
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

        tracing::info!("{metrics_entry}");

        // 重置计数器类型的指标
        collector.reset();
    }
}

/// 获取默认日志文件路径
pub fn get_default_log_path() -> PathBuf {
    PathBuf::from("/var/log/service-vitals/service-vitals.log")
}
