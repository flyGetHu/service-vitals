//! 应用程序核心逻辑
//!
//! 包含主函数、命令执行和应用程序生命周期管理

use crate::cli::args::{Args, Commands};
use crate::cli::commands::{
    CheckCommand, Command, InitCommand, InstallCommand, RestartServiceCommand,
    ServiceStatusCommand, StartServiceCommand, StatusCommand, StopCommand, StopServiceCommand,
    TestNotificationCommand, UninstallCommand, ValidateCommand, VersionCommand,
};
use crate::common::logging::{LogConfig, LoggingSystem};
use crate::core::service::ServiceManager;
use anyhow::{Context, Result};
use clap::Parser;
use std::time::Duration;
use tracing::{error, info};

/// 应用程序主函数
pub async fn main() -> Result<()> {
    // 解析命令行参数
    let args = Args::parse();

    // 初始化日志系统
    let log_config = LogConfig {
        level: args.log_level.clone().into(),
        console: true,
        json_format: false,
        ..Default::default()
    };

    let _logging_system = LoggingSystem::setup_logging(log_config).context("初始化日志系统失败")?;

    info!("Service Vitals v{} 启动", crate::VERSION);

    // 执行命令
    if let Err(e) = execute_command(&args).await {
        error!("命令执行失败: {}", e);
        std::process::exit(1);
    }

    Ok(())
}

/// 执行CLI命令
pub async fn execute_command(args: &Args) -> Result<()> {
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
            command
                .execute(args)
                .await
                .map_err(|e: crate::common::error::ServiceVitalsError| anyhow::anyhow!(e))
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
            command
                .execute(args)
                .await
                .map_err(|e: crate::common::error::ServiceVitalsError| anyhow::anyhow!(e))
        }
        Commands::Check {
            service: _,
            format: _,
            timeout: _,
        } => {
            let command = CheckCommand;
            command
                .execute(args)
                .await
                .map_err(|e: crate::common::error::ServiceVitalsError| anyhow::anyhow!(e))
        }
        Commands::Init {
            config_path: _,
            force: _,
            template: _,
        } => {
            let command = InitCommand;
            command
                .execute(args)
                .await
                .map_err(|e: crate::common::error::ServiceVitalsError| anyhow::anyhow!(e))
        }
        Commands::Validate {
            config_path: _,
            verbose: _,
        } => {
            let command = ValidateCommand;
            command
                .execute(args)
                .await
                .map_err(|e: crate::common::error::ServiceVitalsError| anyhow::anyhow!(e))
        }
        Commands::Version { format: _ } => {
            let command = VersionCommand;
            command
                .execute(args)
                .await
                .map_err(|e: crate::common::error::ServiceVitalsError| anyhow::anyhow!(e))
        }
        Commands::TestNotification {
            notification_type: _,
            message: _,
        } => {
            let command = TestNotificationCommand;
            command
                .execute(args)
                .await
                .map_err(|e: crate::common::error::ServiceVitalsError| anyhow::anyhow!(e))
        }
        Commands::Install { .. } => {
            let command = InstallCommand;
            command
                .execute(args)
                .await
                .map_err(|e: crate::common::error::ServiceVitalsError| anyhow::anyhow!(e))
        }
        Commands::Uninstall { .. } => {
            let command = UninstallCommand;
            command
                .execute(args)
                .await
                .map_err(|e: crate::common::error::ServiceVitalsError| anyhow::anyhow!(e))
        }
        Commands::StartService { .. } => {
            let command = StartServiceCommand;
            command
                .execute(args)
                .await
                .map_err(|e: crate::common::error::ServiceVitalsError| anyhow::anyhow!(e))
        }
        Commands::StopService { .. } => {
            let command = StopServiceCommand;
            command
                .execute(args)
                .await
                .map_err(|e: crate::common::error::ServiceVitalsError| anyhow::anyhow!(e))
        }
        Commands::RestartService { .. } => {
            let command = RestartServiceCommand;
            command
                .execute(args)
                .await
                .map_err(|e: crate::common::error::ServiceVitalsError| anyhow::anyhow!(e))
        }
        Commands::ServiceStatus { .. } => {
            let command = ServiceStatusCommand;
            command
                .execute(args)
                .await
                .map_err(|e: crate::common::error::ServiceVitalsError| anyhow::anyhow!(e))
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

    let service_manager = ServiceManager::new();
    service_manager
        .start(args, foreground, interval, max_concurrent)
        .await
}
