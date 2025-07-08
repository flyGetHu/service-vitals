//! Service Vitals 主程序入口
//!
//! 跨平台服务健康监控工具

use anyhow::{Context, Result};
use clap::Parser;
use service_vitals::cli::args::{Args, Commands};
use service_vitals::cli::commands::{
    CheckCommand, Command, InitCommand, StatusCommand, StopCommand, ValidateCommand, VersionCommand,
};
use service_vitals::config::{ConfigLoader, TomlConfigLoader};
use service_vitals::health::{HttpHealthChecker, Scheduler, TaskScheduler};
use service_vitals::logging::{LogConfig, LoggingSystem};
use service_vitals::notification::sender::NoOpSender;
use service_vitals::notification::FeishuSender;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    // 解析命令行参数
    let args = Args::parse();

    // 初始化日志系统
    let log_config = LogConfig {
        level: args.log_level.clone().into(),
        file_path: None,
        console: true,
        json_format: false,
    };

    LoggingSystem::setup_logging(&log_config).context("初始化日志系统失败")?;

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
