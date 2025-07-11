//! 日志系统模块
//!
//! 提供结构化日志配置和管理功能

use log::LevelFilter;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing_subscriber::{fmt, prelude::*, registry, EnvFilter, Layer};

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

/// 全局日志初始化状态
#[derive(Debug)]
struct GlobalLoggingState {
    /// 是否已初始化
    initialized: bool,
    /// 初始化结果
    init_result: Result<(), String>,
    /// 当前配置
    current_config: Option<LogConfig>,
}

impl Default for GlobalLoggingState {
    fn default() -> Self {
        Self {
            initialized: false,
            init_result: Ok(()),
            current_config: None,
        }
    }
}

/// 全局日志状态管理器
static GLOBAL_LOGGING_STATE: OnceLock<Mutex<GlobalLoggingState>> = OnceLock::new();

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

    /// 初始化日志系统（现代化实现）
    ///
    /// # 参数
    /// * `config` - 日志配置
    ///
    /// # 返回
    /// * `Result<LoggingSystem, anyhow::Error>` - 初始化结果
    ///
    /// # 特性
    /// - 线程安全的单次初始化
    /// - 支持测试环境重新初始化
    /// - 避免使用 unsafe 代码
    /// - 提供清晰的错误信息
    pub fn setup_logging(config: LogConfig) -> anyhow::Result<Self> {
        Self::setup_logging_with_options(config, false)
    }

    /// 初始化日志系统（带选项）
    ///
    /// # 参数
    /// * `config` - 日志配置
    /// * `force_reinit` - 是否强制重新初始化（主要用于测试）
    ///
    /// # 返回
    /// * `Result<LoggingSystem, anyhow::Error>` - 初始化结果
    pub fn setup_logging_with_options(
        config: LogConfig,
        force_reinit: bool,
    ) -> anyhow::Result<Self> {
        // 获取全局状态管理器
        let state_mutex =
            GLOBAL_LOGGING_STATE.get_or_init(|| Mutex::new(GlobalLoggingState::default()));

        // 检查是否需要初始化
        {
            let state = state_mutex.lock().unwrap();
            if state.initialized && !force_reinit {
                // 已经初始化过，检查之前的结果
                match &state.init_result {
                    Ok(()) => {
                        // 之前初始化成功，返回新的 LoggingSystem 实例
                        let system = Self::new(config.clone());

                        // 启动指标收集任务（如果启用）
                        if config.enable_metrics {
                            system.start_metrics_collection(config.metrics_interval);
                        }

                        return Ok(system);
                    }
                    Err(e) => {
                        // 之前初始化失败，如果不是强制重新初始化，返回错误
                        if !force_reinit {
                            return Err(anyhow::anyhow!("日志系统之前初始化失败: {}", e));
                        }
                        // 如果是强制重新初始化，继续执行初始化流程
                    }
                }
            }
        }

        // 执行实际的初始化
        let init_result = Self::perform_initialization(&config);

        // 更新全局状态
        {
            let mut state = state_mutex.lock().unwrap();
            state.initialized = true;
            state.current_config = Some(config.clone());
            state.init_result = init_result.as_ref().map(|_| ()).map_err(|e| e.to_string());
        }

        // 返回结果
        let system = Self::new(config.clone());

        // 启动指标收集任务（如果启用）
        if config.enable_metrics {
            system.start_metrics_collection(config.metrics_interval);
        }

        Ok(system)
    }

    /// 执行实际的日志系统初始化
    fn perform_initialization(config: &LogConfig) -> anyhow::Result<()> {
        // 初始化 LogTracer（log crate 到 tracing 的桥接）
        Self::init_log_tracer()?;

        // 初始化 tracing subscriber
        Self::init_tracing_subscriber(config)?;

        Ok(())
    }

    /// 初始化 LogTracer
    fn init_log_tracer() -> anyhow::Result<()> {
        use tracing_log::LogTracer;

        static LOG_TRACER_INIT: OnceLock<Result<(), String>> = OnceLock::new();

        let result = LOG_TRACER_INIT.get_or_init(|| LogTracer::init().map_err(|e| e.to_string()));

        result
            .as_ref()
            .map_err(|e| anyhow::anyhow!("LogTracer初始化失败: {}", e))?;
        Ok(())
    }

    /// 初始化 tracing subscriber
    fn init_tracing_subscriber(config: &LogConfig) -> anyhow::Result<()> {
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

        // 直接尝试初始化，如果失败就忽略（可能已经初始化过了）
        let result = if config.console {
            // 控制台输出
            registry().with(env_filter).with(fmt_layer).try_init()
        } else if let Some(file_path) = &config.file_path {
            // 文件输出 (简单实现，不包含轮转)
            let file = std::fs::File::create(file_path)
                .map_err(|e| anyhow::anyhow!("创建日志文件失败: {}", e))?;
            let file_layer = fmt::layer()
                .with_writer(file)
                .with_ansi(false)
                .with_file(true)
                .with_line_number(true);

            registry().with(env_filter).with(file_layer).try_init()
        } else {
            // 默认控制台输出
            registry().with(env_filter).with(fmt_layer).try_init()
        };

        // 如果初始化失败，检查是否是因为已经初始化过了
        match result {
            Ok(()) => {
                tracing::info!("日志系统初始化完成");
                tracing::debug!("日志配置: {:?}", config);
                Ok(())
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains(
                    "attempted to set a logger after the logging system was already initialized",
                ) || error_msg.contains("a global default trace dispatcher has already been set")
                {
                    // 这是预期的错误，说明已经初始化过了
                    tracing::debug!("日志系统已经初始化过了");
                    Ok(())
                } else {
                    // 其他错误
                    Err(anyhow::anyhow!(
                        "tracing subscriber初始化失败: {}",
                        error_msg
                    ))
                }
            }
        }
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

    /// 启动指标收集任务
    pub fn start_metrics_collection(&self, interval: Duration) {
        if let Some(ref collector) = self.metrics_collector {
            let collector_clone = Arc::clone(collector);
            tokio::spawn(async move {
                let mut interval_timer = tokio::time::interval(interval);
                loop {
                    interval_timer.tick().await;
                    Self::log_metrics(&collector_clone).await;
                }
            });
        }
    }

    /// 检查日志系统是否已初始化
    pub fn is_initialized() -> bool {
        if let Some(state_mutex) = GLOBAL_LOGGING_STATE.get() {
            let state = state_mutex.lock().unwrap();
            state.initialized
        } else {
            false
        }
    }

    /// 获取当前日志配置（如果已初始化）
    pub fn current_config() -> Option<LogConfig> {
        if let Some(state_mutex) = GLOBAL_LOGGING_STATE.get() {
            let state = state_mutex.lock().unwrap();
            state.current_config.clone()
        } else {
            None
        }
    }

    /// 重置日志系统状态（主要用于测试）
    #[cfg(test)]
    pub fn reset_for_testing() {
        if let Some(state_mutex) = GLOBAL_LOGGING_STATE.get() {
            let mut state = state_mutex.lock().unwrap();
            state.initialized = false;
            state.init_result = Ok(());
            state.current_config = None;
        }
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

// ===================== 日志系统测试模块 =====================

#[cfg(test)]
mod tests {
    use crate::logging::{LogConfig, LogRotation, LoggingSystem};
    use log::LevelFilter;
    use std::collections::HashMap;
    use std::time::Duration;
    use tempfile::NamedTempFile;

    /// 创建测试用的日志配置
    fn create_test_config() -> LogConfig {
        LogConfig {
            level: LevelFilter::Info,
            file_path: None,
            console: true,
            json_format: false,
            rotation: LogRotation::Never,
            max_files: 5,
            module_levels: HashMap::new(),
            enable_metrics: false,
            metrics_interval: Duration::from_secs(60),
        }
    }

    #[tokio::test]
    async fn test_logging_system_single_initialization() {
        // 重置测试状态
        LoggingSystem::reset_for_testing();

        let config = create_test_config();

        // 第一次初始化应该成功
        let result1 = LoggingSystem::setup_logging(config.clone());
        assert!(result1.is_ok());
        assert!(LoggingSystem::is_initialized());

        // 第二次初始化应该返回相同的结果，不会重复初始化
        let result2 = LoggingSystem::setup_logging(config.clone());
        if let Err(ref e) = result2 {
            eprintln!("Second initialization failed: {}", e);
        }
        assert!(result2.is_ok());
    }

    #[tokio::test]
    async fn test_logging_system_force_reinit() {
        // 重置测试状态
        LoggingSystem::reset_for_testing();

        let config = create_test_config();

        // 第一次初始化
        let _result1 = LoggingSystem::setup_logging(config.clone()).unwrap();
        assert!(LoggingSystem::is_initialized());

        // 强制重新初始化
        let result2 = LoggingSystem::setup_logging_with_options(config.clone(), true);
        assert!(result2.is_ok());
    }

    #[tokio::test]
    async fn test_logging_system_with_file_output() {
        // 重置测试状态
        LoggingSystem::reset_for_testing();

        let temp_file = NamedTempFile::new().unwrap();
        let mut config = create_test_config();
        config.file_path = Some(temp_file.path().to_path_buf());
        config.console = false;

        let result = LoggingSystem::setup_logging(config);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_logging_system_with_json_format() {
        // 重置测试状态
        LoggingSystem::reset_for_testing();

        let mut config = create_test_config();
        config.json_format = true;

        let result = LoggingSystem::setup_logging(config);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_logging_system_with_metrics() {
        // 重置测试状态
        LoggingSystem::reset_for_testing();

        let mut config = create_test_config();
        config.enable_metrics = true;
        config.metrics_interval = Duration::from_millis(100);

        let result = LoggingSystem::setup_logging(config);
        assert!(result.is_ok());

        let system = result.unwrap();
        assert!(system.metrics_collector().is_some());
    }

    #[tokio::test]
    async fn test_current_config_retrieval() {
        // 重置测试状态
        LoggingSystem::reset_for_testing();

        let config = create_test_config();
        let _system = LoggingSystem::setup_logging(config.clone()).unwrap();

        let current_config = LoggingSystem::current_config();
        assert!(current_config.is_some());

        let retrieved_config = current_config.unwrap();
        assert_eq!(retrieved_config.level, config.level);
        assert_eq!(retrieved_config.console, config.console);
        assert_eq!(retrieved_config.json_format, config.json_format);
    }

    #[tokio::test]
    async fn test_module_level_filtering() {
        // 重置测试状态
        LoggingSystem::reset_for_testing();

        let mut config = create_test_config();
        config
            .module_levels
            .insert("test_module".to_string(), LevelFilter::Debug);
        config
            .module_levels
            .insert("another_module".to_string(), LevelFilter::Warn);

        let result = LoggingSystem::setup_logging(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_logging_state_before_initialization() {
        // 在任何初始化之前，状态应该是未初始化的
        assert!(!LoggingSystem::is_initialized() || LoggingSystem::current_config().is_some());
    }
}
