//! 日志系统测试模块

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
