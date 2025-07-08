//! API端点实现
//!
//! 提供RESTful API接口

use super::{ApiError, ApiResponse, WebServerState};
use crate::config::GlobalConfig;
use crate::status::{OverallStatus, ServiceStatus};
use serde::Serialize;
use std::convert::Infallible;
use std::sync::Arc;
use warp::{filters::BoxedFilter, http::StatusCode, Filter, Rejection, Reply};

/// 服务状态API响应
#[derive(Debug, Serialize)]
pub struct ServiceStatusResponse {
    /// 服务名称
    pub name: String,
    /// 状态
    pub status: String,
    /// 是否健康
    pub healthy: bool,
    /// 最后检查时间
    pub last_check: Option<chrono::DateTime<chrono::Utc>>,
    /// 状态码
    pub status_code: Option<u16>,
    /// 响应时间（毫秒）
    pub response_time_ms: Option<u64>,
    /// 连续失败次数
    pub consecutive_failures: u32,
    /// 错误信息
    pub error_message: Option<String>,
    /// 是否启用
    pub enabled: bool,
}

impl From<ServiceStatus> for ServiceStatusResponse {
    fn from(status: ServiceStatus) -> Self {
        Self {
            name: status.name,
            status: format!("{:?}", status.status),
            healthy: status.status.is_healthy(),
            last_check: status.last_check,
            status_code: status.status_code,
            response_time_ms: status.response_time_ms,
            consecutive_failures: status.consecutive_failures,
            error_message: status.error_message,
            enabled: status.enabled,
        }
    }
}

/// 整体状态API响应
#[derive(Debug, Serialize)]
pub struct OverallStatusResponse {
    /// 启动时间
    pub start_time: chrono::DateTime<chrono::Utc>,
    /// 总服务数
    pub total_services: usize,
    /// 健康服务数
    pub healthy_services: usize,
    /// 不健康服务数
    pub unhealthy_services: usize,
    /// 禁用服务数
    pub disabled_services: usize,
    /// 最后配置重载时间
    pub last_config_reload: Option<chrono::DateTime<chrono::Utc>>,
    /// 服务列表
    pub services: Vec<ServiceStatusResponse>,
}

impl From<OverallStatus> for OverallStatusResponse {
    fn from(status: OverallStatus) -> Self {
        Self {
            start_time: status.start_time,
            total_services: status.total_services,
            healthy_services: status.healthy_services,
            unhealthy_services: status.unhealthy_services,
            disabled_services: status.disabled_services,
            last_config_reload: status.last_config_reload,
            services: status.services.into_iter().map(ServiceStatusResponse::from).collect(),
        }
    }
}

/// 配置API响应（敏感信息已清理）
#[derive(Debug, Serialize)]
pub struct ConfigResponse {
    /// 全局配置
    pub global: SanitizedGlobalConfig,
    /// 服务数量
    pub service_count: usize,
    /// 配置文件路径
    pub config_path: String,
}

/// 清理敏感信息的全局配置
#[derive(Debug, Serialize)]
pub struct SanitizedGlobalConfig {
    /// 检查间隔
    pub check_interval_seconds: u64,
    /// 请求超时
    pub request_timeout_seconds: u64,
    /// 最大并发检查数
    pub max_concurrent_checks: usize,
    /// 重试次数
    pub retry_attempts: u32,
    /// 重试延迟
    pub retry_delay_seconds: u64,
    /// 日志级别
    pub log_level: String,
    /// 是否配置了飞书Webhook
    pub has_feishu_webhook: bool,
    /// 是否配置了消息模板
    pub has_message_template: bool,
}

impl From<&GlobalConfig> for SanitizedGlobalConfig {
    fn from(config: &GlobalConfig) -> Self {
        Self {
            check_interval_seconds: config.check_interval_seconds,
            request_timeout_seconds: config.request_timeout_seconds,
            max_concurrent_checks: config.max_concurrent_checks,
            retry_attempts: config.retry_attempts,
            retry_delay_seconds: config.retry_delay_seconds,
            log_level: config.log_level.clone(),
            has_feishu_webhook: config.default_feishu_webhook_url.is_some(),
            has_message_template: config.message_template.is_some(),
        }
    }
}

/// 创建API路由
pub fn create_api_routes(
    state: Arc<WebServerState>,
) -> BoxedFilter<(impl Reply,)> {
    let status_routes = create_status_routes(state.clone());
    let config_routes = create_config_routes(state.clone());
    let health_routes = create_health_routes(state.clone());

    let api_routes = warp::path("api")
        .and(warp::path("v1"))
        .and(status_routes.or(config_routes).or(health_routes));

    // 如果启用了认证，添加认证过滤器
    if state.config.disable_auth || state.config.api_key.is_none() {
        api_routes.boxed()
    } else {
        api_routes
            .and(super::auth::auth_filter(state))
            .boxed()
    }
}

/// 创建状态相关路由
fn create_status_routes(
    state: Arc<WebServerState>,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let get_all_status = warp::path("status")
        .and(warp::path::end())
        .and(warp::get())
        .and(with_state(state.clone()))
        .and_then(get_all_status_handler);

    let get_service_status = warp::path("status")
        .and(warp::path::param::<String>())
        .and(warp::path::end())
        .and(warp::get())
        .and(with_state(state))
        .and_then(get_service_status_handler);

    get_all_status.or(get_service_status)
}

/// 创建配置相关路由
fn create_config_routes(
    state: Arc<WebServerState>,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path("config")
        .and(warp::path::end())
        .and(warp::get())
        .and(with_state(state))
        .and_then(get_config_handler)
}

/// 创建健康检查路由
fn create_health_routes(
    state: Arc<WebServerState>,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path("health")
        .and(warp::path::end())
        .and(warp::get())
        .and(with_state(state))
        .and_then(health_check_handler)
}

/// 状态注入过滤器
fn with_state(
    state: Arc<WebServerState>,
) -> impl Filter<Extract = (Arc<WebServerState>,), Error = Infallible> + Clone {
    warp::any().map(move || state.clone())
}

/// 获取所有服务状态处理器
async fn get_all_status_handler(
    state: Arc<WebServerState>,
) -> Result<impl Reply, Rejection> {
    let overall_status = state.status_manager.get_overall_status().await;
    let response = OverallStatusResponse::from(overall_status);
    let api_response = ApiResponse::success(response);
    
    Ok(warp::reply::with_status(
        warp::reply::json(&api_response),
        StatusCode::OK,
    ))
}

/// 获取特定服务状态处理器
async fn get_service_status_handler(
    service_name: String,
    state: Arc<WebServerState>,
) -> Result<impl Reply, Rejection> {
    match state.status_manager.get_service_status(&service_name).await {
        Some(service_status) => {
            let response = ServiceStatusResponse::from(service_status);
            let api_response = ApiResponse::success(response);
            
            Ok(warp::reply::with_status(
                warp::reply::json(&api_response),
                StatusCode::OK,
            ))
        }
        None => {
            let error = ApiError::new(404, format!("服务 '{}' 未找到", service_name));
            Err(warp::reject::custom(error))
        }
    }
}

/// 获取配置处理器
async fn get_config_handler(
    state: Arc<WebServerState>,
) -> Result<impl Reply, Rejection> {
    // 这里需要从某个地方获取配置信息
    // 为了演示，我们创建一个模拟的配置响应
    let config_response = ConfigResponse {
        global: SanitizedGlobalConfig {
            check_interval_seconds: 60,
            request_timeout_seconds: 10,
            max_concurrent_checks: 50,
            retry_attempts: 3,
            retry_delay_seconds: 5,
            log_level: "info".to_string(),
            has_feishu_webhook: false,
            has_message_template: false,
        },
        service_count: state.status_manager.get_all_services().await.len(),
        config_path: "config.toml".to_string(),
    };
    
    let api_response = ApiResponse::success(config_response);
    
    Ok(warp::reply::with_status(
        warp::reply::json(&api_response),
        StatusCode::OK,
    ))
}

/// 健康检查处理器
async fn health_check_handler(
    state: Arc<WebServerState>,
) -> Result<impl Reply, Rejection> {
    let uptime = chrono::Utc::now()
        .signed_duration_since(state.start_time)
        .num_seconds() as u64;
    
    let health_response = super::HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: uptime,
        memory_usage: super::get_memory_usage(),
    };
    
    let api_response = ApiResponse::success(health_response);
    
    Ok(warp::reply::with_status(
        warp::reply::json(&api_response),
        StatusCode::OK,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::HealthStatus;
    use crate::status::ServiceStatus;

    #[test]
    fn test_service_status_response_conversion() {
        let service_status = ServiceStatus {
            name: "test-service".to_string(),
            url: "http://example.com".to_string(),
            status: HealthStatus::Up,
            last_check: Some(chrono::Utc::now()),
            status_code: Some(200),
            response_time_ms: Some(150),
            consecutive_failures: 0,
            error_message: None,
            enabled: true,
        };
        
        let response = ServiceStatusResponse::from(service_status);
        assert_eq!(response.name, "test-service");
        assert!(response.healthy);
        assert_eq!(response.status_code, Some(200));
    }

    #[test]
    fn test_sanitized_global_config() {
        let global_config = GlobalConfig {
            check_interval_seconds: 30,
            request_timeout_seconds: 5,
            max_concurrent_checks: 100,
            retry_attempts: 2,
            retry_delay_seconds: 3,
            log_level: "debug".to_string(),
            default_feishu_webhook_url: Some("https://example.com/webhook".to_string()),
            message_template: Some("Test template".to_string()),
            headers: std::collections::HashMap::new(),
        };
        
        let sanitized = SanitizedGlobalConfig::from(&global_config);
        assert_eq!(sanitized.check_interval_seconds, 30);
        assert!(sanitized.has_feishu_webhook);
        assert!(sanitized.has_message_template);
    }
}
