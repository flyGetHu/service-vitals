//! 守护进程服务模块
//!
//! 处理守护进程模式的启动和管理

use crate::cli::args::Args;
use crate::core::service::ServiceLauncher;
use crate::daemon::{DaemonConfig, DaemonRuntime};
use anyhow::{Context, Result};
use tokio::sync::broadcast;
use tracing::info;

/// 守护进程服务
pub struct DaemonService;

impl DaemonService {
    /// 创建新的守护进程服务
    pub fn new() -> Self {
        Self
    }

    /// 启动守护进程模式
    pub async fn start(
        &self,
        args: &Args,
        interval: Option<u64>,
        max_concurrent: Option<usize>,
    ) -> Result<()> {
        info!("以守护进程模式启动服务...");

        // 创建守护进程配置
        let mut daemon_config = if args.workdir.is_some() || args.pid_file.is_some() {
            DaemonConfig::for_development()
        } else {
            DaemonConfig::default()
        };

        // 应用命令行参数覆盖
        daemon_config.config_path = args.get_config_path();
        if let Some(ref workdir) = args.workdir {
            daemon_config.working_directory = workdir.clone();
        }
        if let Some(ref pid_file) = args.pid_file {
            daemon_config.pid_file = Some(pid_file.clone());
        }

        // 创建守护进程运行时
        let mut daemon_runtime = DaemonRuntime::new(daemon_config);

        // 启动守护进程
        daemon_runtime
            .run(|shutdown_rx| async move {
                self.run_service_main(args, interval, max_concurrent, shutdown_rx)
                    .await
                    .map_err(|e| {
                        crate::common::error::ServiceVitalsError::DaemonError(e.to_string())
                    })
            })
            .await
            .context("守护进程运行失败")
    }

    /// 运行服务主逻辑
    async fn run_service_main(
        &self,
        args: &Args,
        interval: Option<u64>,
        max_concurrent: Option<usize>,
        shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<()> {
        // 1. 加载和验证配置
        let config_path = args.get_config_path();
        let config =
            ServiceLauncher::load_and_validate_config(args, interval, max_concurrent).await?;

        // 2. 初始化核心组件
        let service_components =
            ServiceLauncher::initialize_service_components(&config, &config_path).await?;

        // 3. 启动Web服务器（如果启用）
        let web_server_handle =
            ServiceLauncher::start_web_server_if_enabled(&config, &service_components.scheduler)
                .await?;

        // 4. 设置配置热重载
        ServiceLauncher::setup_config_hot_reload(args, &service_components).await?;

        // 5. 启动后台任务
        ServiceLauncher::start_background_tasks(&service_components, config.services.clone()).await;

        // 6. 等待关闭信号并清理
        ServiceLauncher::handle_shutdown_and_cleanup(
            shutdown_rx,
            web_server_handle,
            &service_components.scheduler,
        )
        .await
    }
}

impl Default for DaemonService {
    fn default() -> Self {
        Self::new()
    }
}
