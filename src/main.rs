//! Service Vitals 主程序入口
//!
//! 跨平台服务健康监控工具

use anyhow::{Context, Result};
use clap::Parser;
use service_vitals::cli::args::{Args, Commands};
use service_vitals::cli::commands::{
    CheckCommand, Command, InitCommand, StatusCommand, StopCommand, TestNotificationCommand,
    ValidateCommand, VersionCommand, InstallCommand, UninstallCommand, StartServiceCommand,
    StopServiceCommand, RestartServiceCommand, ServiceStatusCommand,
};
use service_vitals::config::{ConfigLoader, TomlConfigLoader, ConfigWatcher};
use service_vitals::health::{HttpHealthChecker, Scheduler, TaskScheduler};
use service_vitals::logging::{LogConfig, LoggingSystem};
use service_vitals::notification::sender::NoOpSender;
use service_vitals::notification::FeishuSender;
use service_vitals::status::StatusManager;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    // 解析命令行参数
    let args = Args::parse();

    // 初始化日志系统
    let mut log_config = LogConfig::default();
    log_config.level = args.log_level.clone().into();
    log_config.console = true;
    log_config.json_format = false;

    let _logging_system = LoggingSystem::setup_logging(log_config).context("初始化日志系统失败")?;

    info!("Service Vitals v{} 启动", service_vitals::VERSION);

    // 执行命令
    if let Err(e) = execute_command(&args).await {
        error!("命令执行失败: {}", e);
        std::process::exit(1);
    }

    Ok(())
}

/// 执行CLI命令
async fn execute_command(args: &Args) -> Result<()> {
    match &args.command {
        Commands::Start {
            foreground,
            interval,
            max_concurrent,
        } => execute_start_command(args, *foreground, *interval, *max_concurrent).await,
        Commands::Stop {
            force: _,
            timeout: _,
        } => {
            let command = StopCommand;
            command.execute(args).await.map_err(|e| anyhow::anyhow!(e))
        }
        Commands::Restart {
            foreground: _,
            timeout: _,
        } => {
            // 重启命令：先停止再启动
            let stop_command = StopCommand;
            stop_command.execute(args).await?;

            // 等待一段时间确保服务完全停止
            tokio::time::sleep(Duration::from_secs(2)).await;

            execute_start_command(args, false, None, None).await
        }
        Commands::Status {
            format: _,
            verbose: _,
        } => {
            let command = StatusCommand;
            command.execute(args).await.map_err(|e| anyhow::anyhow!(e))
        }
        Commands::Check {
            service: _,
            format: _,
            timeout: _,
        } => {
            let command = CheckCommand;
            command.execute(args).await.map_err(|e| anyhow::anyhow!(e))
        }
        Commands::Init {
            config_path: _,
            force: _,
            template: _,
        } => {
            let command = InitCommand;
            command.execute(args).await.map_err(|e| anyhow::anyhow!(e))
        }
        Commands::Validate {
            config_path: _,
            verbose: _,
        } => {
            let command = ValidateCommand;
            command.execute(args).await.map_err(|e| anyhow::anyhow!(e))
        }
        Commands::Version { format: _ } => {
            let command = VersionCommand;
            command.execute(args).await.map_err(|e| anyhow::anyhow!(e))
        }
        Commands::TestNotification {
            notification_type: _,
            message: _,
        } => {
            let command = TestNotificationCommand;
            command.execute(args).await.map_err(|e| anyhow::anyhow!(e))
        }
        Commands::Install { .. } => {
            let command = InstallCommand;
            command.execute(args).await.map_err(|e| anyhow::anyhow!(e))
        }
        Commands::Uninstall { .. } => {
            let command = UninstallCommand;
            command.execute(args).await.map_err(|e| anyhow::anyhow!(e))
        }
        Commands::StartService { .. } => {
            let command = StartServiceCommand;
            command.execute(args).await.map_err(|e| anyhow::anyhow!(e))
        }
        Commands::StopService { .. } => {
            let command = StopServiceCommand;
            command.execute(args).await.map_err(|e| anyhow::anyhow!(e))
        }
        Commands::RestartService { .. } => {
            let command = RestartServiceCommand;
            command.execute(args).await.map_err(|e| anyhow::anyhow!(e))
        }
        Commands::ServiceStatus { .. } => {
            let command = ServiceStatusCommand;
            command.execute(args).await.map_err(|e| anyhow::anyhow!(e))
        }
    }
}

/// 执行启动命令
async fn execute_start_command(
    args: &Args,
    foreground: bool,
    interval: Option<u64>,
    max_concurrent: Option<usize>,
) -> Result<()> {
    info!("启动健康检测服务...");

    // 加载配置
    let config_path = args.get_config_path();
    let loader = TomlConfigLoader::new(true);
    let mut config = loader
        .load_from_file(&config_path)
        .await
        .with_context(|| format!("加载配置文件失败: {}", config_path.display()))?;

    // 应用命令行参数覆盖
    if let Some(interval_secs) = interval {
        config.global.check_interval_seconds = interval_secs;
    }
    if let Some(max_concurrent_checks) = max_concurrent {
        config.global.max_concurrent_checks = max_concurrent_checks;
    }

    info!("配置加载完成，服务数量: {}", config.services.len());

    // 创建状态管理器
    let status_manager = Arc::new(StatusManager::new(config_path.clone()));

    // 初始化服务状态
    for service in &config.services {
        status_manager.add_service(
            service.name.clone(),
            service.url.clone(),
            service.enabled,
        ).await;
    }

    // 创建HTTP健康检测器
    let checker = Arc::new(HttpHealthChecker::new(
        Duration::from_secs(config.global.request_timeout_seconds),
        config.global.retry_attempts,
        Duration::from_secs(config.global.retry_delay_seconds),
    )?);

    // 创建通知发送器
    let notifier: Option<Arc<dyn service_vitals::notification::NotificationSender>> =
        if let Some(ref webhook_url) = config.global.default_feishu_webhook_url {
            Some(Arc::new(FeishuSender::new(Some(webhook_url.clone()))?))
        } else {
            Some(Arc::new(NoOpSender))
        };

    // 创建任务调度器
    let scheduler = Arc::new(TaskScheduler::new(checker, notifier, config.global.clone()));

    // 设置配置热重载
    let (mut config_watcher, config_receiver) = ConfigWatcher::new(
        &config_path,
        std::time::Duration::from_millis(500), // 500ms防抖动延迟
    ).context("创建配置监控器失败")?;

    // 启动配置文件监控
    config_watcher.start().context("启动配置监控失败")?;

    // 启动配置变更监听任务
    let scheduler_clone = scheduler.clone();
    let status_manager_clone = status_manager.clone();
    tokio::spawn(async move {
        let mut receiver = config_receiver;
        while let Ok(change_event) = receiver.recv().await {
            info!("检测到配置变更，版本: {}", change_event.version);

            // 更新状态管理器中的服务列表
            for service in &change_event.new_config.services {
                status_manager_clone.add_service(
                    service.name.clone(),
                    service.url.clone(),
                    service.enabled,
                ).await;
            }

            // 标记配置重载
            status_manager_clone.mark_config_reload().await;

            // 重新加载调度器配置
            if let Err(e) = scheduler_clone.reload_config(change_event.new_config.services).await {
                error!("配置热重载失败: {}", e);
            } else {
                info!("配置热重载成功");
            }
        }
    });

    // 启动状态保存任务
    let status_manager_save = status_manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30)); // 每30秒保存一次状态
        loop {
            interval.tick().await;
            let status_file = StatusManager::get_default_status_file_path();
            if let Err(e) = status_manager_save.save_to_file(&status_file).await {
                warn!("保存状态文件失败: {}", e);
            }
        }
    });

    // 启动调度器
    scheduler
        .start(config.services)
        .await
        .context("启动任务调度器失败")?;

    info!("健康检测服务已启动");

    if foreground {
        info!("在前台模式运行，按 Ctrl+C 停止服务");

        // 等待中断信号
        match signal::ctrl_c().await {
            Ok(()) => {
                info!("收到中断信号，正在停止服务...");
            }
            Err(err) => {
                error!("监听中断信号失败: {}", err);
            }
        }

        // 停止调度器
        scheduler.stop().await.context("停止任务调度器失败")?;

        info!("服务已停止");
    } else {
        info!("在后台模式运行");
        // TODO: 在第三阶段实现守护进程模式
        warn!("守护进程模式尚未实现，将在前台运行");

        // 等待中断信号
        match signal::ctrl_c().await {
            Ok(()) => {
                info!("收到中断信号，正在停止服务...");
            }
            Err(err) => {
                error!("监听中断信号失败: {}", err);
            }
        }

        // 停止调度器
        scheduler.stop().await.context("停止任务调度器失败")?;

        info!("服务已停止");
    }

    Ok(())
}
