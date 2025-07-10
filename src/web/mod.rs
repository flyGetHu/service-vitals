//! Web 服务器模块
//!
//! 提供实时监控状态面板的 Web 服务器功能

pub mod handlers;

use crate::config::types::WebConfig;
use crate::error::{Result, ServiceVitalsError};
use crate::status::ServiceStatus;
use axum::{
    routing::get,
    Router,
};
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

/// 服务状态历史记录
#[derive(Debug, Clone)]
pub struct StatusHistory {
    /// 时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// 状态
    pub status: String,
    /// 响应时间（毫秒）
    pub response_time_ms: Option<u64>,
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
    /// 状态历史记录（最近10次）
    pub history: Vec<StatusHistory>,
}

/// 共享的 Web 状态数据
pub type SharedWebState = Arc<RwLock<HashMap<String, WebServiceStatus>>>;

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
        Router::new()
            .route("/dashboard", get(handlers::dashboard))
            .route("/api/v1/status", get(handlers::api_status))
            .layer(
                ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
                    .layer(CorsLayer::permissive()),
            )
            .with_state(Arc::clone(&self.state))
    }

    /// 更新服务状态
    async fn update_status(state: SharedWebState, status: ServiceStatus) {
        let mut state_guard = state.write().await;
        
        let web_status = state_guard
            .entry(status.name.clone())
            .or_insert_with(|| WebServiceStatus {
                name: status.name.clone(),
                url: status.url.clone(),
                status: "Unknown".to_string(),
                response_time_ms: None,
                last_check: None,
                history: Vec::new(),
            });

        // 更新当前状态
        let new_status = if status.status.is_healthy() {
            "Online"
        } else {
            "Offline"
        };

        // 如果状态发生变化，添加到历史记录
        if web_status.status != new_status {
            web_status.history.push(StatusHistory {
                timestamp: chrono::Utc::now(),
                status: new_status.to_string(),
                response_time_ms: status.response_time_ms,
            });

            // 保持最近10条记录
            if web_status.history.len() > 10 {
                web_status.history.remove(0);
            }
        }

        web_status.status = new_status.to_string();
        web_status.response_time_ms = status.response_time_ms;
        web_status.last_check = Some(chrono::Utc::now());

        if !status.url.is_empty() {
            web_status.url = status.url;
        }
    }
}
