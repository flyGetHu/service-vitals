//! Web 服务器模块
//!
//! 提供实时监控状态面板的 Web 服务器功能

pub mod handlers;

use crate::common::error::{Result, ServiceVitalsError};
use crate::common::status::ServiceStatus;
use crate::config::types::WebConfig;
use crate::health::result::HealthStatus;
use axum::{routing::get, Router};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{error, info};

/// Web 服务器错误类型
#[derive(Debug, thiserror::Error)]
pub enum WebError {
    #[error("端口冲突: 端口 {port} 已被占用")]
    PortConflict { port: u16 },
    #[error("模板渲染失败: {message}")]
    TemplateRenderError { message: String },
    #[error("服务器启动失败: {message}")]
    ServerStartError { message: String },
}

/// Web 服务器状态数据
#[derive(Debug, Clone)]
pub struct WebServiceStatus {
    /// 服务名称
    pub name: String,
    /// 服务 URL
    pub url: String,
    /// 当前状态
    pub status: String,
    /// 响应延迟（毫秒）
    pub response_time_ms: Option<u64>,
    /// 最后检查时间
    pub last_check: Option<chrono::DateTime<chrono::Utc>>,
    /// 错误信息（当状态为 Offline 或 Unknown 时）
    pub error_message: Option<String>,
}

/// 共享的 Web 状态数据
pub type SharedWebState = Arc<RwLock<HashMap<String, WebServiceStatus>>>;

/// Web 应用状态，包含配置和服务状态
#[derive(Clone)]
pub struct WebAppState {
    /// Web 配置
    pub config: WebConfig,
    /// 服务状态数据
    pub services: SharedWebState,
}

/// Web 服务器结构
pub struct WebServer {
    /// 配置
    config: WebConfig,
    /// 共享状态数据
    state: SharedWebState,
    /// 状态更新接收器
    status_receiver: Option<mpsc::Receiver<ServiceStatus>>,
}

impl WebServer {
    /// 创建新的 Web 服务器实例
    pub fn new(config: WebConfig) -> (Self, mpsc::Sender<ServiceStatus>) {
        let (tx, rx) = mpsc::channel(1000);
        let state = Arc::new(RwLock::new(HashMap::new()));

        (
            Self {
                config,
                state,
                status_receiver: Some(rx),
            },
            tx,
        )
    }

    /// 启动 Web 服务器
    pub async fn start(mut self) -> Result<()> {
        if !self.config.enabled {
            info!("Web 服务器功能已禁用");
            return Ok(());
        }

        info!(
            "启动 Web 服务器，监听地址: {}:{}",
            self.config.bind_address, self.config.port
        );

        // 启动状态更新任务
        let state_clone = Arc::clone(&self.state);
        if let Some(mut rx) = self.status_receiver.take() {
            tokio::spawn(async move {
                while let Some(status) = rx.recv().await {
                    Self::update_status(state_clone.clone(), status).await;
                }
            });
        }

        // 创建路由
        let app = self.create_router();

        // 解析监听地址
        let addr = format!("{}:{}", self.config.bind_address, self.config.port)
            .parse::<SocketAddr>()
            .map_err(|e| ServiceVitalsError::Other(anyhow::anyhow!("无效的监听地址: {}", e)))?;

        // 启动服务器
        let listener = tokio::net::TcpListener::bind(&addr).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                ServiceVitalsError::WebError(WebError::PortConflict {
                    port: self.config.port,
                })
            } else {
                ServiceVitalsError::WebError(WebError::ServerStartError {
                    message: e.to_string(),
                })
            }
        })?;

        info!("Web 服务器已启动，访问地址: http://{}", addr);

        axum::serve(listener, app).await.map_err(|e| {
            ServiceVitalsError::WebError(WebError::ServerStartError {
                message: e.to_string(),
            })
        })?;

        Ok(())
    }

    /// 创建路由
    fn create_router(&self) -> Router {
        let app_state = WebAppState {
            config: self.config.clone(),
            services: Arc::clone(&self.state),
        };

        Router::new()
            .route("/dashboard", get(handlers::dashboard))
            .route("/api/v1/status", get(handlers::api_status))
            .layer(
                ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
                    .layer(CorsLayer::permissive()),
            )
            .with_state(app_state)
    }

    /// 更新服务状态
    async fn update_status(state: SharedWebState, status: ServiceStatus) {
        let mut state_guard = state.write().await;

        let web_status =
            state_guard
                .entry(status.name.clone())
                .or_insert_with(|| WebServiceStatus {
                    name: status.name.clone(),
                    url: status.url.clone(),
                    status: "Unknown".to_string(),
                    response_time_ms: None,
                    last_check: None,
                    error_message: None,
                });

        // 更新当前状态
        let new_status = match status.status {
            HealthStatus::Up => "Online",
            HealthStatus::Down => "Offline",
            HealthStatus::Unknown => "Unknown",
            HealthStatus::Degraded => "Offline", // 降级状态视为离线
        };

        web_status.status = new_status.to_string();
        web_status.response_time_ms = status.response_time_ms;
        web_status.last_check = Some(chrono::Utc::now());

        // 更新错误信息：只有在服务离线或未知状态时才保留错误信息
        web_status.error_message = if new_status == "Offline" || new_status == "Unknown" {
            status.error_message.clone()
        } else {
            None
        };

        if !status.url.is_empty() {
            web_status.url = status.url;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::status::ServiceStatus;
    use crate::config::types::WebConfig;
    use crate::health::HealthStatus;

    #[tokio::test]
    async fn test_web_server_creation() {
        let config = WebConfig::default();
        let (web_server, _sender) = WebServer::new(config.clone());

        assert_eq!(web_server.config.port, config.port);
        assert_eq!(web_server.config.bind_address, config.bind_address);
    }

    #[tokio::test]
    async fn test_status_update() {
        let config = WebConfig::default();
        let (web_server, _sender) = WebServer::new(config);

        let status = ServiceStatus {
            name: "test-service".to_string(),
            url: "https://example.com".to_string(),
            status: HealthStatus::Up,
            last_check: Some(chrono::Utc::now()),
            status_code: Some(200),
            response_time_ms: Some(150),
            consecutive_failures: 0,
            error_message: None,
            enabled: true,
        };

        WebServer::update_status(web_server.state.clone(), status).await;

        let state_guard = web_server.state.read().await;
        assert!(state_guard.contains_key("test-service"));

        let web_status = state_guard.get("test-service").unwrap();
        assert_eq!(web_status.name, "test-service");
        assert_eq!(web_status.status, "Online");
        assert_eq!(web_status.response_time_ms, Some(150));
    }

    #[tokio::test]
    async fn test_status_history() {
        let config = WebConfig::default();
        let (web_server, _sender) = WebServer::new(config);

        // 第一次更新
        let status1 = ServiceStatus {
            name: "test-service".to_string(),
            url: "https://example.com".to_string(),
            status: HealthStatus::Up,
            last_check: Some(chrono::Utc::now()),
            status_code: Some(200),
            response_time_ms: Some(150),
            consecutive_failures: 0,
            error_message: None,
            enabled: true,
        };

        WebServer::update_status(web_server.state.clone(), status1).await;

        // 第二次更新（状态变化）
        let status2 = ServiceStatus {
            name: "test-service".to_string(),
            url: "https://example.com".to_string(),
            status: HealthStatus::Down,
            last_check: Some(chrono::Utc::now()),
            status_code: Some(500),
            response_time_ms: Some(5000),
            consecutive_failures: 1,
            error_message: Some("Internal Server Error".to_string()),
            enabled: true,
        };

        WebServer::update_status(web_server.state.clone(), status2).await;

        let state_guard = web_server.state.read().await;
        let web_status = state_guard.get("test-service").unwrap();

        assert_eq!(web_status.status, "Offline");
        assert!(web_status.error_message.is_some()); // 离线状态应该有错误信息
        assert_eq!(
            web_status.error_message.as_ref().unwrap(),
            "Internal Server Error"
        );
    }

    #[tokio::test]
    async fn test_error_message_propagation() {
        // 测试错误信息从 ServiceStatus 到 WebServiceStatus 的传递
        let config = WebConfig::default();
        let (web_server, _sender) = WebServer::new(config);

        // 创建一个带有错误信息的 ServiceStatus
        let service_status_with_error = ServiceStatus {
            name: "test-service".to_string(),
            url: "https://example.com".to_string(),
            status: HealthStatus::Down,
            last_check: Some(chrono::Utc::now()),
            status_code: Some(500),
            response_time_ms: Some(1000),
            consecutive_failures: 1,
            error_message: Some("HTTP 500 Internal Server Error".to_string()),
            enabled: true,
        };

        // 更新状态
        WebServer::update_status(web_server.state.clone(), service_status_with_error).await;

        // 验证错误信息是否正确传递
        let state_guard = web_server.state.read().await;
        let web_status = state_guard.get("test-service").unwrap();

        assert_eq!(web_status.status, "Offline");
        assert!(web_status.error_message.is_some());
        assert_eq!(
            web_status.error_message.as_ref().unwrap(),
            "HTTP 500 Internal Server Error"
        );

        // 测试 Unknown 状态也应该保留错误信息
        drop(state_guard);

        let service_status_unknown = ServiceStatus {
            name: "test-service-unknown".to_string(),
            url: "https://example.com".to_string(),
            status: HealthStatus::Unknown,
            last_check: Some(chrono::Utc::now()),
            status_code: None,
            response_time_ms: None,
            consecutive_failures: 0,
            error_message: Some("DNS resolution failed".to_string()),
            enabled: true,
        };

        WebServer::update_status(web_server.state.clone(), service_status_unknown).await;

        let state_guard = web_server.state.read().await;
        let web_status_unknown = state_guard.get("test-service-unknown").unwrap();

        assert_eq!(web_status_unknown.status, "Unknown");
        assert!(web_status_unknown.error_message.is_some());
        assert_eq!(
            web_status_unknown.error_message.as_ref().unwrap(),
            "DNS resolution failed"
        );

        // 测试 Online 状态应该清除错误信息
        drop(state_guard);

        let service_status_online = ServiceStatus {
            name: "test-service".to_string(),
            url: "https://example.com".to_string(),
            status: HealthStatus::Up,
            last_check: Some(chrono::Utc::now()),
            status_code: Some(200),
            response_time_ms: Some(150),
            consecutive_failures: 0,
            error_message: None,
            enabled: true,
        };

        WebServer::update_status(web_server.state.clone(), service_status_online).await;

        let state_guard = web_server.state.read().await;
        let web_status_online = state_guard.get("test-service").unwrap();

        assert_eq!(web_status_online.status, "Online");
        assert!(web_status_online.error_message.is_none()); // Online 状态应该没有错误信息
    }

    #[test]
    fn test_web_app_state_clone() {
        let config = WebConfig::default();
        let services = Arc::new(RwLock::new(HashMap::new()));

        let app_state = WebAppState {
            config: config.clone(),
            services: services.clone(),
        };

        let cloned_state = app_state.clone();
        assert_eq!(cloned_state.config.port, config.port);
    }
}
