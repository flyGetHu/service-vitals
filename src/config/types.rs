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
    /// Web界面配置
    #[serde(default)]
    pub web: WebConfig,
}

/// Web配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    /// 是否启用Web服务器
    #[serde(default)]
    pub enabled: bool,
    /// 绑定地址
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    /// 绑定端口
    #[serde(default = "default_port")]
    pub port: u16,
    /// API密钥认证
    pub api_key: Option<String>,
    /// 是否禁用认证（内网环境）
    #[serde(default)]
    pub disable_auth: bool,
    /// 静态文件目录
    pub static_dir: Option<String>,
    /// CORS设置
    #[serde(default = "default_cors_enabled")]
    pub cors_enabled: bool,
    /// 允许的CORS源
    #[serde(default)]
    pub cors_origins: Vec<String>,
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

fn default_bind_address() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_cors_enabled() -> bool {
    true
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bind_address: default_bind_address(),
            port: default_port(),
            api_key: None,
            disable_auth: false,
            static_dir: None,
            cors_enabled: default_cors_enabled(),
            cors_origins: vec!["*".to_string()],
        }
    }
}

impl WebConfig {
    /// 获取完整的绑定地址
    pub fn socket_addr(&self) -> crate::error::Result<std::net::SocketAddr> {
        let addr = format!("{}:{}", self.bind_address, self.port);
        addr.parse()
            .map_err(|e| crate::error::ServiceVitalsError::Other(anyhow::anyhow!("无效的绑定地址: {}", e)))
    }

    /// 验证配置
    pub fn validate(&self) -> crate::error::Result<Vec<String>> {
        let mut warnings = Vec::new();

        // 检查端口范围
        if self.port < 1024 && self.bind_address != "127.0.0.1" && self.bind_address != "localhost" {
            warnings.push("使用特权端口(<1024)需要管理员权限".to_string());
        }

        // 检查认证配置
        if !self.disable_auth && self.api_key.is_none() {
            warnings.push("启用认证但未设置API密钥，建议设置api_key或启用disable_auth".to_string());
        }

        // 检查CORS配置
        if self.cors_enabled && self.cors_origins.contains(&"*".to_string()) && !self.disable_auth {
            warnings.push("启用了通配符CORS但未禁用认证，可能存在安全风险".to_string());
        }

        Ok(warnings)
    }
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
            }],
            web: WebConfig::default(),
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
        };

        assert_eq!(global_config.check_interval_seconds, 60);
        assert_eq!(global_config.log_level, "info");
        assert_eq!(global_config.request_timeout_seconds, 10);
        assert_eq!(global_config.max_concurrent_checks, 50);
        assert_eq!(global_config.retry_attempts, 3);
        assert_eq!(global_config.retry_delay_seconds, 5);
    }
}
