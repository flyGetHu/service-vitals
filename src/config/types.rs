//! 配置数据结构定义
//!
//! 定义应用程序的配置结构体和验证逻辑

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 主配置结构，包含全局配置和服务列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 全局配置项
    pub global: GlobalConfig,
    /// 服务配置列表
    pub services: Vec<ServiceConfig>,
}

/// 全局配置结构
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlobalConfig {
    /// 默认飞书webhook URL
    pub default_feishu_webhook_url: Option<String>,
    /// 消息模板
    pub message_template: Option<String>,
    /// 检测间隔（秒）
    #[serde(default = "default_check_interval")]
    pub check_interval_seconds: u64,
    /// 日志级别
    #[serde(default = "default_log_level")]
    pub log_level: String,
    /// 请求超时时间（秒）
    #[serde(default = "default_timeout")]
    pub request_timeout_seconds: u64,
    /// 最大并发检测数
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_checks: usize,
    /// 失败重试次数
    #[serde(default = "default_retry_attempts")]
    pub retry_attempts: u32,
    /// 重试间隔（秒）
    #[serde(default = "default_retry_delay")]
    pub retry_delay_seconds: u64,
    /// 全局请求头
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Web 服务器配置
    pub web: Option<WebConfig>,
}

/// 服务配置结构
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceConfig {
    /// 服务名称
    pub name: String,
    /// 服务URL
    pub url: String,
    /// HTTP方法
    #[serde(default = "default_method")]
    pub method: String,
    /// 期望的状态码列表
    pub expected_status_codes: Vec<u16>,
    /// 服务特定的飞书webhook URL
    pub feishu_webhook_url: Option<String>,
    /// 失败阈值
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,
    /// 服务特定的检测间隔
    pub check_interval_seconds: Option<u64>,
    /// 是否启用
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 服务描述
    pub description: Option<String>,
    /// 服务特定的请求头
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// 请求体（用于POST/PUT请求）
    pub body: Option<serde_json::Value>,
    /// 告警冷却时间（秒，时间退避，默认300秒）
    #[serde(default = "default_alert_cooldown")]
    pub alert_cooldown_secs: u64,
}

// 默认值函数
fn default_check_interval() -> u64 {
    60
}
fn default_log_level() -> String {
    "info".to_string()
}
fn default_timeout() -> u64 {
    10
}
fn default_max_concurrent() -> usize {
    50
}
fn default_retry_attempts() -> u32 {
    3
}
fn default_retry_delay() -> u64 {
    5
}
fn default_method() -> String {
    "GET".to_string()
}
fn default_failure_threshold() -> u32 {
    1
}
fn default_enabled() -> bool {
    true
}

fn default_alert_cooldown() -> u64 {
    300 // 5分钟默认冷却时间
}

/// 配置验证函数
///
/// # 参数
/// * `config` - 要验证的配置
///
/// # 返回
/// * `Result<(), String>` - 验证结果，错误时返回错误信息
pub fn validate_config(config: &Config) -> Result<(), String> {
    // 验证全局配置
    if config.global.check_interval_seconds == 0 {
        return Err("检测间隔不能为0".to_string());
    }

    if config.global.request_timeout_seconds == 0 {
        return Err("请求超时时间不能为0".to_string());
    }

    if config.global.max_concurrent_checks == 0 {
        return Err("最大并发检测数不能为0".to_string());
    }

    // 验证日志级别
    let valid_log_levels = ["debug", "info", "warn", "error"];
    if !valid_log_levels.contains(&config.global.log_level.as_str()) {
        return Err(format!(
            "无效的日志级别: {}，支持的级别: {:?}",
            config.global.log_level, valid_log_levels
        ));
    }

    // 验证Web配置（如果启用）
    if let Some(ref web_config) = config.global.web {
        if web_config.enabled {
            // 验证端口范围
            if web_config.port == 0 {
                return Err(format!(
                    "无效的Web服务器端口: {}，端口不能为0",
                    web_config.port
                ));
            }

            // 验证绑定地址
            if web_config.bind_address.is_empty() {
                return Err("Web服务器绑定地址不能为空".to_string());
            }

            // 验证布局类型
            let valid_layout_types = ["cards", "table"];
            if !valid_layout_types.contains(&web_config.layout_type.as_str()) {
                return Err(format!(
                    "无效的界面布局类型: {}，支持的类型: {:?}",
                    web_config.layout_type, valid_layout_types
                ));
            }

            // 验证刷新间隔
            if web_config.refresh_interval_seconds == 0 {
                return Err("Web界面刷新间隔不能为0秒".to_string());
            }

            if web_config.refresh_interval_seconds > 300 {
                return Err("Web界面刷新间隔不能超过300秒".to_string());
            }
        }
    }

    // 验证服务配置
    if config.services.is_empty() {
        return Err("至少需要配置一个服务".to_string());
    }

    for service in &config.services {
        // 验证服务名称
        if service.name.trim().is_empty() {
            return Err("服务名称不能为空".to_string());
        }

        // 验证URL格式
        if !service.url.starts_with("http://") && !service.url.starts_with("https://") {
            return Err(format!("服务 {} 的URL格式无效", service.name));
        }

        // 验证状态码
        if service.expected_status_codes.is_empty() {
            return Err(format!("服务 {} 必须指定期望的状态码", service.name));
        }

        for &code in &service.expected_status_codes {
            if !(100..=599).contains(&code) {
                return Err(format!("服务 {} 的状态码 {} 无效", service.name, code));
            }
        }

        // 验证HTTP方法
        let valid_methods = ["GET", "POST", "PUT", "DELETE", "HEAD", "OPTIONS", "PATCH"];
        if !valid_methods.contains(&service.method.as_str()) {
            return Err(format!(
                "服务 {} 的HTTP方法 {} 无效，支持的方法: {:?}",
                service.name, service.method, valid_methods
            ));
        }

        // 验证失败阈值
        if service.failure_threshold == 0 {
            return Err(format!("服务 {} 的失败阈值不能为0", service.name));
        }

        // 验证检测间隔
        if let Some(interval) = service.check_interval_seconds {
            if interval == 0 {
                return Err(format!("服务 {} 的检测间隔不能为0", service.name));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> Config {
        Config {
            global: GlobalConfig {
                default_feishu_webhook_url: Some("https://example.com/webhook".to_string()),
                message_template: Some("Test template".to_string()),
                check_interval_seconds: 30,
                log_level: "info".to_string(),
                request_timeout_seconds: 10,
                max_concurrent_checks: 50,
                retry_attempts: 3,
                retry_delay_seconds: 5,
                headers: HashMap::new(),
                web: None,
            },
            services: vec![ServiceConfig {
                name: "Test Service".to_string(),
                url: "https://example.com/health".to_string(),
                method: "GET".to_string(),
                expected_status_codes: vec![200],
                feishu_webhook_url: None,
                failure_threshold: 1,
                check_interval_seconds: None,
                enabled: true,
                description: Some("Test service description".to_string()),
                headers: HashMap::new(),
                body: None,
                alert_cooldown_secs: Some(60),
            }],
        }
    }

    fn create_test_service() -> ServiceConfig {
        ServiceConfig {
            name: "Test Service".to_string(),
            url: "https://example.com/health".to_string(),
            method: "GET".to_string(),
            expected_status_codes: vec![200],
            feishu_webhook_url: None,
            failure_threshold: 1,
            check_interval_seconds: None,
            enabled: true,
            description: Some("Test service description".to_string()),
            headers: HashMap::new(),
            body: None,
            alert_cooldown_secs: Some(60),
        }
    }

    #[test]
    fn test_config_serialization() {
        let config = create_test_config();

        // 测试序列化
        let serialized = toml::to_string(&config).expect("序列化失败");
        assert!(!serialized.is_empty());

        // 测试反序列化
        let deserialized: Config = toml::from_str(&serialized).expect("反序列化失败");
        assert_eq!(
            config.global.check_interval_seconds,
            deserialized.global.check_interval_seconds
        );
        assert_eq!(config.services.len(), deserialized.services.len());
        assert_eq!(config.services[0].name, deserialized.services[0].name);
    }

    #[test]
    fn test_config_validation() {
        let config = create_test_config();
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_config_validation_empty_services() {
        let mut config = create_test_config();
        config.services.clear();

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("至少需要配置一个服务"));
    }

    #[test]
    fn test_config_validation_invalid_url() {
        let mut config = create_test_config();
        config.services[0].url = "invalid-url".to_string();

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("URL格式无效"));
    }

    #[test]
    fn test_config_validation_invalid_status_code() {
        let mut config = create_test_config();
        config.services[0].expected_status_codes = vec![999];

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("状态码"));
    }

    #[test]
    fn test_config_validation_invalid_method() {
        let mut config = create_test_config();
        config.services[0].method = "INVALID".to_string();

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("HTTP方法"));
    }

    #[test]
    fn test_default_values() {
        let global_config = GlobalConfig {
            default_feishu_webhook_url: None,
            message_template: None,
            check_interval_seconds: default_check_interval(),
            log_level: default_log_level(),
            request_timeout_seconds: default_timeout(),
            max_concurrent_checks: default_max_concurrent(),
            retry_attempts: default_retry_attempts(),
            retry_delay_seconds: default_retry_delay(),
            headers: HashMap::new(),
            web: None,
        };

        assert_eq!(global_config.check_interval_seconds, 60);
        assert_eq!(global_config.log_level, "info");
        assert_eq!(global_config.request_timeout_seconds, 10);
        assert_eq!(global_config.max_concurrent_checks, 50);
        assert_eq!(global_config.retry_attempts, 3);
        assert_eq!(global_config.retry_delay_seconds, 5);
    }

    #[test]
    fn test_web_config_default() {
        let web_config = WebConfig::default();

        assert!(!web_config.enabled);
        assert_eq!(web_config.port, 8080);
        assert_eq!(web_config.bind_address, "0.0.0.0");
        assert!(!web_config.show_problems_only);
        assert_eq!(web_config.layout_type, "cards");
        assert_eq!(web_config.refresh_interval_seconds, 3);
    }

    #[test]
    fn test_web_config_validation_valid() {
        let config = Config {
            global: GlobalConfig {
                default_feishu_webhook_url: None,
                message_template: None,
                check_interval_seconds: 60,
                log_level: "info".to_string(),
                request_timeout_seconds: 10,
                max_concurrent_checks: 50,
                retry_attempts: 3,
                retry_delay_seconds: 5,
                headers: HashMap::new(),
                web: Some(WebConfig {
                    enabled: true,
                    port: 8080,
                    bind_address: "127.0.0.1".to_string(),
                    show_problems_only: false,
                    layout_type: "cards".to_string(),
                    refresh_interval_seconds: 5,
                }),
            },
            services: vec![create_test_service()],
        };

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_web_config_validation_invalid_port() {
        let config = Config {
            global: GlobalConfig {
                default_feishu_webhook_url: None,
                message_template: None,
                check_interval_seconds: 60,
                log_level: "info".to_string(),
                request_timeout_seconds: 10,
                max_concurrent_checks: 50,
                retry_attempts: 3,
                retry_delay_seconds: 5,
                headers: HashMap::new(),
                web: Some(WebConfig {
                    enabled: true,
                    port: 0,
                    bind_address: "127.0.0.1".to_string(),
                    show_problems_only: false,
                    layout_type: "cards".to_string(),
                    refresh_interval_seconds: 5,
                }),
            },
            services: vec![create_test_service()],
        };

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("端口不能为0"));
    }

    #[test]
    fn test_web_config_validation_invalid_layout() {
        let config = Config {
            global: GlobalConfig {
                default_feishu_webhook_url: None,
                message_template: None,
                check_interval_seconds: 60,
                log_level: "info".to_string(),
                request_timeout_seconds: 10,
                max_concurrent_checks: 50,
                retry_attempts: 3,
                retry_delay_seconds: 5,
                headers: HashMap::new(),
                web: Some(WebConfig {
                    enabled: true,
                    port: 8080,
                    bind_address: "127.0.0.1".to_string(),
                    show_problems_only: false,
                    layout_type: "invalid".to_string(),
                    refresh_interval_seconds: 5,
                }),
            },
            services: vec![create_test_service()],
        };

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("无效的界面布局类型"));
    }

    #[test]
    fn test_web_config_validation_invalid_refresh_interval() {
        let config = Config {
            global: GlobalConfig {
                default_feishu_webhook_url: None,
                message_template: None,
                check_interval_seconds: 60,
                log_level: "info".to_string(),
                request_timeout_seconds: 10,
                max_concurrent_checks: 50,
                retry_attempts: 3,
                retry_delay_seconds: 5,
                headers: HashMap::new(),
                web: Some(WebConfig {
                    enabled: true,
                    port: 8080,
                    bind_address: "127.0.0.1".to_string(),
                    show_problems_only: false,
                    layout_type: "cards".to_string(),
                    refresh_interval_seconds: 0,
                }),
            },
            services: vec![create_test_service()],
        };

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("刷新间隔不能为0秒"));
    }

    #[test]
    fn test_web_config_validation_refresh_interval_too_long() {
        let config = Config {
            global: GlobalConfig {
                default_feishu_webhook_url: None,
                message_template: None,
                check_interval_seconds: 60,
                log_level: "info".to_string(),
                request_timeout_seconds: 10,
                max_concurrent_checks: 50,
                retry_attempts: 3,
                retry_delay_seconds: 5,
                headers: HashMap::new(),
                web: Some(WebConfig {
                    enabled: true,
                    port: 8080,
                    bind_address: "127.0.0.1".to_string(),
                    show_problems_only: false,
                    layout_type: "cards".to_string(),
                    refresh_interval_seconds: 400,
                }),
            },
            services: vec![create_test_service()],
        };

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("刷新间隔不能超过300秒"));
    }
}

/// Web 服务器配置结构
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebConfig {
    /// 是否启用 Web 功能
    #[serde(default = "default_web_enabled")]
    pub enabled: bool,
    /// 监听端口
    #[serde(default = "default_web_port")]
    pub port: u16,
    /// 绑定地址
    #[serde(default = "default_web_bind_address")]
    pub bind_address: String,
    /// 是否只显示问题服务（离线/不可用）
    #[serde(default = "default_show_problems_only")]
    pub show_problems_only: bool,
    /// 界面布局类型
    #[serde(default = "default_layout_type")]
    pub layout_type: String,
    /// 自动刷新间隔（秒）
    #[serde(default = "default_refresh_interval")]
    pub refresh_interval_seconds: u32,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            enabled: default_web_enabled(),
            port: default_web_port(),
            bind_address: default_web_bind_address(),
            show_problems_only: default_show_problems_only(),
            layout_type: default_layout_type(),
            refresh_interval_seconds: default_refresh_interval(),
        }
    }
}

/// 默认 Web 功能启用状态
fn default_web_enabled() -> bool {
    false
}

/// 默认 Web 服务器端口
fn default_web_port() -> u16 {
    8080
}

/// 默认 Web 服务器绑定地址
fn default_web_bind_address() -> String {
    "0.0.0.0".to_string()
}

/// 默认是否只显示问题服务
fn default_show_problems_only() -> bool {
    false
}

/// 默认界面布局类型
fn default_layout_type() -> String {
    "cards".to_string()
}

/// 默认自动刷新间隔（秒）
fn default_refresh_interval() -> u32 {
    3
}
