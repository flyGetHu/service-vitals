//! 前台服务模块
//!
//! 处理前台模式的启动和信号处理

use anyhow::Result;
use service_vitals::cli::args::Args;
use service_vitals::core::service::ServiceLauncher;
use tokio::signal;
use tokio::sync::broadcast;
use tracing::{error, info};

/// 前台服务
pub struct ForegroundService;

impl ForegroundService {
    /// 创建新的前台服务
    pub fn new() -> Self {
        Self
    }

    /// 启动前台模式
    pub async fn start(
        &self,
        args: &Args,
        interval: Option<u64>,
        max_concurrent: Option<usize>,
    ) -> Result<()> {
        info!("以前台模式启动服务...");

        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        // 设置Ctrl+C信号处理
        let shutdown_tx_clone = shutdown_tx.clone();
        tokio::spawn(async move {
            match signal::ctrl_c().await {
                Ok(()) => {
                    info!("收到中断信号，正在停止服务...");
                    let _ = shutdown_tx_clone.send(());
                }
                Err(err) => {
                    error!("监听中断信号失败: {}", err);
                }
            }
        });

        self.run_service_main(args, interval, max_concurrent, shutdown_rx).await
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
        let config = ServiceLauncher::load_and_validate_config(args, interval, max_concurrent).await?;

        // 2. 初始化核心组件
        let service_components = ServiceLauncher::initialize_service_components(&config, &config_path).await?;

        // 3. 启动Web服务器（如果启用）
        let web_server_handle =
            ServiceLauncher::start_web_server_if_enabled(&config, &service_components.scheduler).await?;

        // 4. 设置配置热重载
        ServiceLauncher::setup_config_hot_reload(args, &service_components).await?;

        // 5. 启动后台任务
        ServiceLauncher::start_background_tasks(&service_components).await;

        // 6. 等待关闭信号并清理
        ServiceLauncher::handle_shutdown_and_cleanup(
            shutdown_rx,
            web_server_handle,
            &service_components.scheduler,
        )
        .await
    }
}

impl Default for ForegroundService {
    fn default() -> Self {
        Self::new()
    }
}