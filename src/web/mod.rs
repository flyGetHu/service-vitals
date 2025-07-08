//! Web界面和API模块
//!
//! 提供HTTP API和Web监控界面

use crate::config::WebConfig;
use crate::status::StatusManager;
use serde::Serialize;
use std::sync::Arc;

pub mod api;
pub mod auth;
pub mod dashboard;
pub mod metrics;
pub mod server;





/// Web服务器状态
#[derive(Debug, Clone)]
pub struct WebServerState {
    /// 状态管理器
    pub status_manager: Arc<StatusManager>,
    /// Web配置
    pub config: WebConfig,
    /// 启动时间
    pub start_time: chrono::DateTime<chrono::Utc>,
}

impl WebServerState {
    /// 创建新的Web服务器状态
    pub fn new(status_manager: Arc<StatusManager>, config: WebConfig) -> Self {
        Self {
            status_manager,
            config,
            start_time: chrono::Utc::now(),
        }
    }
}

/// API响应包装器
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    /// 是否成功
    pub success: bool,
    /// 响应数据
    pub data: Option<T>,
    /// 错误信息
    pub error: Option<String>,
    /// 时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl<T> ApiResponse<T> {
    /// 创建成功响应
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: chrono::Utc::now(),
        }
    }

    /// 创建错误响应
    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            timestamp: chrono::Utc::now(),
        }
    }
}

/// API错误类型
#[derive(Debug, Serialize)]
pub struct ApiError {
    /// 错误代码
    pub code: u16,
    /// 错误消息
    pub message: String,
    /// 详细信息
    pub details: Option<String>,
}

impl ApiError {
    /// 创建新的API错误
    pub fn new(code: u16, message: String) -> Self {
        Self {
            code,
            message,
            details: None,
        }
    }

    /// 添加详细信息
    pub fn with_details(mut self, details: String) -> Self {
        self.details = Some(details);
        self
    }
}

impl warp::reject::Reject for ApiError {}

/// 健康检查响应
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// 服务状态
    pub status: String,
    /// 版本信息
    pub version: String,
    /// 运行时间
    pub uptime_seconds: u64,
    /// 内存使用情况
    pub memory_usage: Option<MemoryUsage>,
}

/// 内存使用情况
#[derive(Debug, Serialize)]
pub struct MemoryUsage {
    /// 已使用内存（字节）
    pub used_bytes: u64,
    /// 总内存（字节）
    pub total_bytes: u64,
    /// 使用百分比
    pub usage_percent: f64,
}

/// 获取系统内存使用情况
pub fn get_memory_usage() -> Option<MemoryUsage> {
    // 简单的内存使用情况获取
    // 在实际实现中，可以使用更精确的系统调用
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
            let mut total = 0u64;
            let mut available = 0u64;
            
            for line in meminfo.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(value) = line.split_whitespace().nth(1) {
                        total = value.parse::<u64>().unwrap_or(0) * 1024; // kB to bytes
                    }
                } else if line.starts_with("MemAvailable:") {
                    if let Some(value) = line.split_whitespace().nth(1) {
                        available = value.parse::<u64>().unwrap_or(0) * 1024; // kB to bytes
                    }
                }
            }
            
            if total > 0 {
                let used = total - available;
                let usage_percent = (used as f64 / total as f64) * 100.0;
                return Some(MemoryUsage {
                    used_bytes: used,
                    total_bytes: total,
                    usage_percent,
                });
            }
        }
    }
    
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_config_default() {
        let config = WebConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.bind_address, "127.0.0.1");
        assert_eq!(config.port, 8080);
    }

    #[test]
    fn test_web_config_socket_addr() {
        let config = WebConfig {
            bind_address: "0.0.0.0".to_string(),
            port: 3000,
            ..Default::default()
        };
        
        let addr = config.socket_addr().unwrap();
        assert_eq!(addr.to_string(), "0.0.0.0:3000");
    }

    #[test]
    fn test_web_config_validation() {
        let mut config = WebConfig::default();
        config.enabled = true;
        config.disable_auth = false;
        config.api_key = None;
        
        let warnings = config.validate().unwrap();
        assert!(!warnings.is_empty());
        assert!(warnings.iter().any(|w| w.contains("API密钥")));
    }

    #[test]
    fn test_api_response_success() {
        let response = ApiResponse::success("test data");
        assert!(response.success);
        assert_eq!(response.data, Some("test data"));
        assert!(response.error.is_none());
    }

    #[test]
    fn test_api_response_error() {
        let response: ApiResponse<()> = ApiResponse::error("test error".to_string());
        assert!(!response.success);
        assert!(response.data.is_none());
        assert_eq!(response.error, Some("test error".to_string()));
    }

    #[test]
    fn test_api_error_creation() {
        let error = ApiError::new(404, "Not Found".to_string())
            .with_details("Resource not found".to_string());
        
        assert_eq!(error.code, 404);
        assert_eq!(error.message, "Not Found");
        assert_eq!(error.details, Some("Resource not found".to_string()));
    }
}
