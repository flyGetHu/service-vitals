//! 服务管理模块
//!
//! 负责服务的启动、组件初始化和生命周期管理

use anyhow::{Context, Result};
use service_vitals::cli::args::Args;
use service_vitals::config::{self, ConfigLoader, ConfigWatcher, TomlConfigLoader};
use service_vitals::daemon::{DaemonConfig, DaemonRuntime};
use service_vitals::health::{HttpHealthChecker, Scheduler, TaskScheduler};
use service_vitals::notification::sender::NoOpSender;
use service_vitals::notification::FeishuSender;
use service_vitals::status::{ServiceStatus, StatusManager};
use service_vitals::web::WebServer;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::broadcast;
use tracing::{error, info};

use crate::daemon_service::DaemonService;
use crate::foreground_service::ForegroundService;

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
            foreground_service.start(args, interval, max_concurrent).await
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
        let config_loader = TomlConfigLoader::new();
        let mut config = config_loader
            .load(&config_path)
            .await
            .context("加载配置文件失败")?;

        // 应用命令行参数覆盖
        if let Some(interval) = interval {
            config.global.check_interval = interval;
        }
        if let Some(max_concurrent) = max_concurrent {
            config.global.max_concurrent_checks = max_concurrent;
        }

        // 验证配置
        config::validate_config(&config).context("配置验证失败")?;

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
        let status_manager = Arc::new(StatusManager::new(config_path));

        // 创建通知发送器
        let notification_sender: Arc<dyn service_vitals::notification::NotificationSender> =
            if let Some(feishu_config) = &config.global.notifications.feishu {
                Arc::new(FeishuSender::new(feishu_config.clone()))
            } else {
                Arc::new(NoOpSender)
            };

        // 创建健康检测器
        let health_checker = Arc::new(HttpHealthChecker::new());

        // 创建任务调度器
        let scheduler = Arc::new(TaskScheduler::new(
            health_checker,
            notification_sender,
            config.global.max_concurrent_checks,
        ));

        // 注册服务到调度器
        for service_config in &config.services {
            if service_config.enabled {
                scheduler
                    .add_service(service_config.clone())
                    .await
                    .context("添加服务到调度器失败")?;
            }
        }

        Ok(ServiceComponents::new(status_manager, scheduler))
    }

    /// 启动Web服务器（如果启用）
    pub async fn start_web_server_if_enabled(
        config: &config::Config,
        scheduler: &Arc<TaskScheduler>,
    ) -> Result<Option<tokio::task::JoinHandle<()>>> {
        if let Some(web_config) = &config.global.web {
            if web_config.enabled {
                info!("启动Web服务器，监听地址: {}", web_config.bind_address);

                let web_server = WebServer::new(web_config.clone(), scheduler.clone());
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
        args: &Args,
        service_components: &ServiceComponents,
    ) -> Result<()> {
        if !args.disable_hot_reload {
            info!("启用配置热重载...");

            let config_path = args.get_config_path();
            let config_watcher = ConfigWatcher::new(config_path);
            let scheduler = service_components.scheduler.clone();

            tokio::spawn(async move {
                if let Err(e) = config_watcher.watch(move |config| {
                    let scheduler = scheduler.clone();
                    async move {
                        // 重新加载配置并更新调度器
                        info!("检测到配置变更，正在重新加载...");
                        // TODO: 实现配置热重载逻辑
                        let _ = scheduler;
                    }
                }).await {
                    error!("配置热重载失败: {}", e);
                }
            });
        }

        Ok(())
    }

    /// 启动后台任务
    pub async fn start_background_tasks(service_components: &ServiceComponents) {
        info!("启动后台任务...");

        // 启动调度器
        let scheduler = service_components.scheduler.clone();
        tokio::spawn(async move {
            if let Err(e) = scheduler.start().await {
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
        scheduler.stop().await;

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