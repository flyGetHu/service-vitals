//! 服务管理模块
//!
//! 负责服务的启动、组件初始化和生命周期管理

use crate::cli::args::Args;
use crate::common::status::StatusManager;
use crate::config::{self, ConfigLoader, TomlConfigLoader};
use crate::health::{HttpHealthChecker, Scheduler, TaskScheduler};
use crate::notification::sender::NoOpSender;
use crate::notification::FeishuSender;
use crate::web::WebServer;
use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{error, info};

use crate::core::daemon_service::DaemonService;
use crate::core::foreground_service::ForegroundService;

/// 服务管理器
pub struct ServiceManager;

impl ServiceManager {
    /// 创建新的服务管理器
    pub fn new() -> Self {
        Self
    }

    /// 启动服务
    pub async fn start(
        &self,
        args: &Args,
        foreground: bool,
        interval: Option<u64>,
        max_concurrent: Option<usize>,
    ) -> Result<()> {
        // 检查是否需要以守护进程模式运行
        let should_daemonize = args.daemon && !foreground;

        if should_daemonize {
            let daemon_service = DaemonService::new();
            daemon_service.start(args, interval, max_concurrent).await
        } else {
            let foreground_service = ForegroundService::new();
            foreground_service
                .start(args, interval, max_concurrent)
                .await
        }
    }
}

impl Default for ServiceManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 服务组件结构
pub struct ServiceComponents {
    /// 状态管理器，负责跟踪和持久化服务状态
    pub status_manager: Arc<StatusManager>,
    /// 任务调度器，负责执行健康检测任务
    pub scheduler: Arc<TaskScheduler>,
}

impl ServiceComponents {
    /// 创建新的服务组件
    pub fn new(status_manager: Arc<StatusManager>, scheduler: Arc<TaskScheduler>) -> Self {
        Self {
            status_manager,
            scheduler,
        }
    }
}

/// 服务启动器
pub struct ServiceLauncher;

impl ServiceLauncher {
    /// 加载和验证配置
    pub async fn load_and_validate_config(
        args: &Args,
        interval: Option<u64>,
        max_concurrent: Option<usize>,
    ) -> Result<config::Config> {
        let config_path = args.get_config_path();
        info!("加载配置文件: {:?}", config_path);

        // 创建配置加载器
        let config_loader = TomlConfigLoader::new(true);
        let mut config = config_loader
            .load_from_file(&config_path)
            .await
            .context("加载配置文件失败")?;

        // 应用命令行参数覆盖
        if let Some(interval) = interval {
            config.global.check_interval_seconds = interval;
        }
        if let Some(max_concurrent) = max_concurrent {
            config.global.max_concurrent_checks = max_concurrent;
        }

        // 验证配置
        config::validate_config(&config).map_err(|e| anyhow::anyhow!("配置验证失败: {}", e))?;

        info!("配置加载成功，共 {} 个服务", config.services.len());
        Ok(config)
    }

    /// 初始化服务组件
    pub async fn initialize_service_components(
        config: &config::Config,
        config_path: &std::path::Path,
    ) -> Result<ServiceComponents> {
        info!("初始化服务组件...");

        // 创建状态管理器
        let status_manager = Arc::new(StatusManager::new(config_path.to_path_buf()));

        // 创建通知发送器
        let notification_sender: Option<Arc<dyn crate::notification::NotificationSender>> =
            if let Some(feishu_url) = &config.global.default_feishu_webhook_url {
                Some(Arc::new(FeishuSender::new(Some(feishu_url.clone()))?))
            } else {
                Some(Arc::new(NoOpSender))
            };

        // 创建健康检测器
        let health_checker = Arc::new(HttpHealthChecker::new(
            std::time::Duration::from_secs(config.global.request_timeout_seconds),
            config.global.retry_attempts,
            std::time::Duration::from_secs(config.global.retry_delay_seconds),
        )?);

        // 创建任务调度器
        let scheduler = Arc::new(TaskScheduler::new(
            health_checker,
            notification_sender,
            config.global.clone(),
        ));

        // 注册服务到调度器
        scheduler
            .start(config.services.clone())
            .await
            .context("启动调度器失败")?;

        Ok(ServiceComponents::new(status_manager, scheduler))
    }

    /// 启动Web服务器（如果启用）
    pub async fn start_web_server_if_enabled(
        config: &config::Config,
        _scheduler: &Arc<TaskScheduler>,
    ) -> Result<Option<tokio::task::JoinHandle<()>>> {
        if let Some(web_config) = &config.global.web {
            if web_config.enabled {
                info!("启动Web服务器，监听地址: {}", web_config.bind_address);

                let (web_server, _status_tx) = WebServer::new(web_config.clone());
                let handle = tokio::spawn(async move {
                    if let Err(e) = web_server.start().await {
                        error!("Web服务器运行失败: {}", e);
                    }
                });

                return Ok(Some(handle));
            }
        }

        Ok(None)
    }

    /// 设置配置热重载
    pub async fn setup_config_hot_reload(
        _args: &Args,
        _service_components: &ServiceComponents,
    ) -> Result<()> {
        // 移除 disable_hot_reload 检查，直接返回 Ok(())
        Ok(())
    }

    /// 启动后台任务
    pub async fn start_background_tasks(
        service_components: &ServiceComponents,
        services: Vec<config::ServiceConfig>,
    ) {
        info!("启动后台任务...");

        // 启动调度器
        let scheduler = service_components.scheduler.clone();
        tokio::spawn(async move {
            if let Err(e) = scheduler.start(services.clone()).await {
                error!("调度器运行失败: {}", e);
            }
        });
    }

    /// 处理关闭和清理
    pub async fn handle_shutdown_and_cleanup(
        mut shutdown_rx: broadcast::Receiver<()>,
        web_server_handle: Option<tokio::task::JoinHandle<()>>,
        scheduler: &Arc<TaskScheduler>,
    ) -> Result<()> {
        info!("等待关闭信号...");

        // 等待关闭信号
        let _ = shutdown_rx.recv().await;

        info!("收到关闭信号，正在停止服务...");

        // 停止调度器
        let _ = scheduler.stop().await;

        // 等待Web服务器停止
        if let Some(handle) = web_server_handle {
            if let Err(e) = handle.await {
                error!("Web服务器停止时出错: {}", e);
            }
        }

        info!("服务已停止");
        Ok(())
    }
}
