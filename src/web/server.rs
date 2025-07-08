//! Web服务器实现
//!
//! 提供HTTP服务器和路由管理

use super::{
    api, auth, dashboard, metrics, WebConfig, WebServerState,
};
use crate::error::Result;
use crate::status::StatusManager;
use log::{info, warn};
use std::sync::Arc;
use tokio::sync::broadcast;
use warp::{Filter, Reply};

/// Web服务器
pub struct WebServer {
    /// 配置
    config: WebConfig,
    /// 状态管理器
    status_manager: Arc<StatusManager>,
    /// 指标收集器
    metrics_collector: Arc<metrics::MetricsCollector>,
    /// 关闭信号接收器
    shutdown_rx: Option<broadcast::Receiver<()>>,
}

impl WebServer {
    /// 创建新的Web服务器
    pub fn new(
        config: WebConfig,
        status_manager: Arc<StatusManager>,
        shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<Self> {
        let metrics_collector = Arc::new(metrics::MetricsCollector::new()
            .map_err(|e| crate::error::ServiceVitalsError::Other(anyhow::anyhow!("创建指标收集器失败: {}", e)))?);

        Ok(Self {
            config,
            status_manager,
            metrics_collector,
            shutdown_rx: Some(shutdown_rx),
        })
    }

    /// 启动Web服务器
    pub async fn start(&mut self) -> Result<()> {
        if !self.config.enabled {
            info!("Web服务器已禁用");
            return Ok(());
        }

        // 验证配置
        let warnings = self.config.validate()?;
        for warning in warnings {
            warn!("Web配置警告: {}", warning);
        }

        let addr = self.config.socket_addr()?;
        info!("启动Web服务器，监听地址: {}", addr);

        // 创建服务器状态
        let state = Arc::new(WebServerState::new(
            self.status_manager.clone(),
            self.config.clone(),
        ));

        // 创建路由
        let routes = self.create_routes(state.clone());

        // 启动指标更新任务
        self.start_metrics_update_task().await;

        // 获取关闭信号接收器
        let mut shutdown_rx = self.shutdown_rx.take()
            .ok_or_else(|| crate::error::ServiceVitalsError::Other(anyhow::anyhow!("关闭信号接收器已被使用")))?;

        // 启动服务器
        let (_, server) = warp::serve(routes)
            .bind_with_graceful_shutdown(addr, async move {
                let _ = shutdown_rx.recv().await;
                info!("接收到关闭信号，正在关闭Web服务器...");
            });

        info!("Web服务器已启动: http://{}", addr);
        info!("仪表板地址: http://{}/dashboard", addr);
        info!("API文档: http://{}/api/v1/status", addr);
        info!("Prometheus指标: http://{}/metrics", addr);

        server.await;
        info!("Web服务器已关闭");

        Ok(())
    }

    /// 创建路由
    fn create_routes(
        &self,
        state: Arc<WebServerState>,
    ) -> impl Filter<Extract = impl Reply, Error = std::convert::Infallible> + Clone {
        // API路由（内置认证）
        let api_routes = api::create_api_routes(state.clone());

        // 指标路由
        let metrics_routes = metrics::create_metrics_route(self.metrics_collector.clone());

        // 仪表板路由（不需要认证）
        let dashboard_routes = dashboard::create_dashboard_routes(state);

        // CORS处理
        let cors = if self.config.cors_enabled {
            warp::cors()
                .allow_origins(self.config.cors_origins.iter().map(|s| s.as_str()).collect::<Vec<_>>())
                .allow_headers(vec!["content-type", "x-api-key", "authorization"])
                .allow_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"])
                .build()
        } else {
            warp::cors().allow_any_origin().build()
        };

        // 组合所有路由
        let all_routes = dashboard_routes
            .or(api_routes)
            .or(metrics_routes)
            .with(cors)
            .recover(auth::handle_auth_error);

        all_routes
    }

    /// 启动指标更新任务
    async fn start_metrics_update_task(&self) {
        let status_manager = self.status_manager.clone();
        let metrics_collector = self.metrics_collector.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                // 获取当前状态并更新指标
                let overall_status = status_manager.get_overall_status().await;
                metrics_collector.update_all_metrics(&overall_status);
            }
        });
    }

    /// 获取服务器信息
    pub fn get_server_info(&self) -> ServerInfo {
        ServerInfo {
            enabled: self.config.enabled,
            bind_address: self.config.bind_address.clone(),
            port: self.config.port,
            auth_enabled: !self.config.disable_auth && self.config.api_key.is_some(),
            cors_enabled: self.config.cors_enabled,
        }
    }
}

/// 服务器信息
#[derive(Debug, serde::Serialize)]
pub struct ServerInfo {
    /// 是否启用
    pub enabled: bool,
    /// 绑定地址
    pub bind_address: String,
    /// 端口
    pub port: u16,
    /// 是否启用认证
    pub auth_enabled: bool,
    /// 是否启用CORS
    pub cors_enabled: bool,
}

/// 创建Web服务器配置的辅助函数
pub fn create_web_config() -> WebConfig {
    WebConfig::default()
}

/// 创建开发环境的Web配置
pub fn create_dev_web_config() -> WebConfig {
    WebConfig {
        enabled: true,
        bind_address: "127.0.0.1".to_string(),
        port: 8080,
        api_key: Some("dev-api-key-12345678".to_string()),
        disable_auth: false,
        static_dir: None,
        cors_enabled: true,
        cors_origins: vec!["*".to_string()],
    }
}

/// 创建生产环境的Web配置
pub fn create_prod_web_config(api_key: String) -> WebConfig {
    WebConfig {
        enabled: true,
        bind_address: "0.0.0.0".to_string(),
        port: 8080,
        api_key: Some(api_key),
        disable_auth: false,
        static_dir: None,
        cors_enabled: true,
        cors_origins: vec![], // 生产环境应该指定具体的源
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::StatusManager;
    use std::path::PathBuf;
    use tokio::sync::broadcast;

    #[tokio::test]
    async fn test_web_server_creation() {
        let config = create_dev_web_config();
        let status_manager = Arc::new(StatusManager::new(PathBuf::from("test.toml")));
        let (_, shutdown_rx) = broadcast::channel(1);

        let server = WebServer::new(config, status_manager, shutdown_rx);
        assert!(server.is_ok());
    }

    #[test]
    fn test_server_info() {
        let config = create_dev_web_config();
        let status_manager = Arc::new(StatusManager::new(PathBuf::from("test.toml")));
        let (_, shutdown_rx) = broadcast::channel(1);

        let server = WebServer::new(config, status_manager, shutdown_rx).unwrap();
        let info = server.get_server_info();

        assert!(info.enabled);
        assert_eq!(info.bind_address, "127.0.0.1");
        assert_eq!(info.port, 8080);
        assert!(info.auth_enabled);
    }

    #[test]
    fn test_create_web_configs() {
        let default_config = create_web_config();
        assert!(!default_config.enabled);

        let dev_config = create_dev_web_config();
        assert!(dev_config.enabled);
        assert!(dev_config.api_key.is_some());

        let prod_config = create_prod_web_config("prod-key".to_string());
        assert!(prod_config.enabled);
        assert_eq!(prod_config.bind_address, "0.0.0.0");
        assert!(prod_config.cors_origins.is_empty());
    }

    #[test]
    fn test_web_config_validation() {
        let mut config = create_dev_web_config();
        config.port = 80;
        config.bind_address = "0.0.0.0".to_string();

        let warnings = config.validate().unwrap();
        assert!(!warnings.is_empty());
        assert!(warnings.iter().any(|w| w.contains("特权端口")));
    }
}
