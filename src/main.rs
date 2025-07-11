//! Service Vitals 主程序入口
//!
//! 跨平台服务健康监控工具

use anyhow::{Context, Result};
use clap::Parser;
use service_vitals::cli::args::{Args, Commands};
use service_vitals::cli::commands::{
    CheckCommand, Command, InitCommand, InstallCommand, RestartServiceCommand,
    ServiceStatusCommand, StartServiceCommand, StatusCommand, StopCommand, StopServiceCommand,
    TestNotificationCommand, UninstallCommand, ValidateCommand, VersionCommand,
};
use service_vitals::config::{self, ConfigLoader, ConfigWatcher, TomlConfigLoader};
use service_vitals::daemon::{DaemonConfig, DaemonRuntime};
use service_vitals::health::{HttpHealthChecker, Scheduler, TaskScheduler};
use service_vitals::logging::{LogConfig, LoggingSystem};
use service_vitals::notification::sender::NoOpSender;
use service_vitals::notification::FeishuSender;
use service_vitals::status::{ServiceStatus, StatusManager};
use service_vitals::web::WebServer;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::broadcast;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
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

    // 检查是否需要以守护进程模式运行
    let should_daemonize = args.daemon && !foreground;

    if should_daemonize {
        return start_daemon_mode(args, interval, max_concurrent).await;
    } else {
        return start_foreground_mode(args, interval, max_concurrent).await;
    }
}

/// 守护进程模式启动
async fn start_daemon_mode(
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
            start_service_main((*args).clone(), interval, max_concurrent, shutdown_rx)
                .await
                .map_err(|e| service_vitals::error::ServiceVitalsError::DaemonError(e.to_string()))
        })
        .await
        .context("守护进程运行失败")
}

/// 前台模式启动
async fn start_foreground_mode(
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

    start_service_main(args.clone(), interval, max_concurrent, shutdown_rx).await
}

/// 服务主逻辑
async fn start_service_main(
    args: Args,
    interval: Option<u64>,
    max_concurrent: Option<usize>,
    shutdown_rx: broadcast::Receiver<()>,
) -> Result<()> {
    // 1. 加载和验证配置
    let config_path = args.get_config_path();
    let config = load_and_validate_config(&args, interval, max_concurrent).await?;

    // 2. 初始化核心组件
    let service_components = initialize_service_components(&config, &config_path).await?;

    // 3. 启动Web服务器（如果启用）
    let web_server_handle =
        start_web_server_if_enabled(&config, &service_components.scheduler).await?;

    // 4. 设置配置热重载
    setup_config_hot_reload(&args, &service_components).await?;

    // 5. 启动后台任务
    start_background_tasks(&service_components).await;

    // 6. 启动调度器
    service_components
        .scheduler
        .start(config.services)
        .await
        .context("启动任务调度器失败")?;

    info!("健康检测服务已启动");

    // 7. 等待关闭信号并清理资源
    handle_shutdown_and_cleanup(
        shutdown_rx,
        web_server_handle,
        &service_components.scheduler,
    )
    .await
}

/// 加载和验证配置文件
///
/// 从指定路径加载配置文件，并应用命令行参数覆盖。
///
/// # 参数
///
/// * `args` - 命令行参数，包含配置文件路径
/// * `interval` - 可选的检测间隔覆盖值（秒）
/// * `max_concurrent` - 可选的最大并发检测数覆盖值
///
/// # 返回值
///
/// 返回加载并验证后的配置对象，如果配置文件不存在或格式错误则返回错误。
///
/// # 错误
///
/// * 配置文件不存在
/// * 配置文件格式错误
/// * 配置验证失败
async fn load_and_validate_config(
    args: &Args,
    interval: Option<u64>,
    max_concurrent: Option<usize>,
) -> Result<config::Config> {
    let config_path = args.get_config_path();
    let loader = TomlConfigLoader::new(true);

    // 检查配置文件是否存在
    if !config_path.exists() {
        return Err(anyhow::anyhow!(
            "配置文件不存在: {}\n提示：请运行 'service-vitals init' 创建默认配置文件",
            config_path.display()
        ));
    }

    let mut config = loader
        .load_from_file(&config_path)
        .await
        .with_context(|| {
            format!(
                "加载配置文件失败: {}\n请检查配置文件格式是否正确。参考示例：examples/basic_config.toml",
                config_path.display()
            )
        })?;

    // 应用命令行参数覆盖
    if let Some(interval_secs) = interval {
        config.global.check_interval_seconds = interval_secs;
    }
    if let Some(max_concurrent_checks) = max_concurrent {
        config.global.max_concurrent_checks = max_concurrent_checks;
    }

    info!("配置加载完成，服务数量: {}", config.services.len());
    Ok(config)
}

/// 服务组件集合
///
/// 包含服务运行所需的核心组件，便于统一管理和传递。
struct ServiceComponents {
    /// 状态管理器，负责跟踪和持久化服务状态
    status_manager: Arc<StatusManager>,
    /// 任务调度器，负责执行健康检测任务
    scheduler: Arc<TaskScheduler>,
}

/// 初始化核心服务组件
///
/// 创建并配置服务运行所需的核心组件，包括状态管理器、健康检测器、
/// 通知发送器和任务调度器。
///
/// # 参数
///
/// * `config` - 服务配置对象
/// * `config_path` - 配置文件路径，用于状态管理器
///
/// # 返回值
///
/// 返回初始化完成的服务组件集合。
///
/// # 错误
///
/// * HTTP健康检测器创建失败
/// * 通知发送器创建失败
async fn initialize_service_components(
    config: &config::Config,
    config_path: &std::path::Path,
) -> Result<ServiceComponents> {
    // 创建状态管理器
    let status_manager = Arc::new(StatusManager::new(config_path.to_path_buf()));

    // 初始化服务状态
    for service in &config.services {
        status_manager
            .add_service(service.name.clone(), service.url.clone(), service.enabled)
            .await;
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
    let scheduler = TaskScheduler::new(checker, notifier, config.global.clone());

    // 设置回调来更新 StatusManager
    let status_manager_for_callback = status_manager.clone();
    scheduler
        .set_health_result_callback(Arc::new(move |result| {
            let status_manager = status_manager_for_callback.clone();
            let result = result.clone();

            tokio::spawn(async move {
                status_manager.update_service_status(&result).await;
            });
        }))
        .await;

    let scheduler = Arc::new(scheduler);

    Ok(ServiceComponents {
        status_manager,
        scheduler,
    })
}

/// 启动Web服务器（如果启用）
///
/// 根据配置决定是否启动Web监控面板。如果启用，会创建Web服务器实例，
/// 设置健康检测结果回调，并在后台启动服务器。
///
/// # 参数
///
/// * `config` - 服务配置对象，包含Web服务器配置
/// * `scheduler` - 任务调度器，用于设置健康检测结果回调
///
/// # 返回值
///
/// 如果Web服务器启动成功，返回服务器任务句柄；如果未启用或启动失败，返回None。
///
/// # 错误
///
/// * Web服务器创建失败
/// * 回调设置失败
async fn start_web_server_if_enabled(
    config: &config::Config,
    scheduler: &Arc<TaskScheduler>,
) -> Result<Option<tokio::task::JoinHandle<()>>> {
    let web_server_handle = if let Some(ref web_config) = config.global.web {
        if web_config.enabled {
            info!(
                "启动 Web 监控面板，地址: {}:{}",
                web_config.bind_address, web_config.port
            );

            let (web_server, status_sender) = WebServer::new(web_config.clone());

            // 设置健康检测结果回调，将结果发送到 Web 服务器
            let status_sender_for_callback = status_sender.clone();

            scheduler
                .set_health_result_callback(Arc::new(move |result| {
                    let status_sender = status_sender_for_callback.clone();
                    let result = result.clone();

                    tokio::spawn(async move {
                        // 发送到 Web 服务器
                        let service_status = ServiceStatus {
                            name: result.service_name.clone(),
                            url: result.service_url.clone(),
                            status: result.status,
                            last_check: Some(result.timestamp),
                            status_code: result.status_code,
                            response_time_ms: Some(result.response_time.as_millis() as u64),
                            consecutive_failures: result.consecutive_failures,
                            error_message: result.error_message.clone(),
                            enabled: true,
                        };

                        let _ = status_sender.send(service_status).await;
                    });
                }))
                .await;

            // 启动 Web 服务器
            Some(tokio::spawn(async move {
                if let Err(e) = web_server.start().await {
                    error!("Web 服务器启动失败: {}", e);
                }
            }))
        } else {
            info!("Web 监控面板已禁用");
            None
        }
    } else {
        info!("Web 监控面板未配置");
        None
    };

    Ok(web_server_handle)
}

/// 设置配置热重载
///
/// 创建配置文件监控器，监听配置文件变更并自动重新加载配置。
/// 当配置文件发生变更时，会更新状态管理器中的服务列表，并重新加载调度器配置。
///
/// # 参数
///
/// * `args` - 命令行参数，包含配置文件路径
/// * `service_components` - 服务组件集合，用于配置更新
///
/// # 返回值
///
/// 配置监控器启动成功返回Ok，否则返回错误。
///
/// # 错误
///
/// * 配置监控器创建失败
/// * 配置监控器启动失败
async fn setup_config_hot_reload(
    args: &Args,
    service_components: &ServiceComponents,
) -> Result<()> {
    let config_path = args.get_config_path();

    // 设置配置热重载
    let (mut config_watcher, config_receiver) = ConfigWatcher::new(
        &config_path,
        std::time::Duration::from_millis(500), // 500ms防抖动延迟
    )
    .context("创建配置监控器失败")?;

    // 启动配置文件监控
    config_watcher.start().context("启动配置监控失败")?;

    // 启动配置变更监听任务
    let scheduler_clone = service_components.scheduler.clone();
    let status_manager_clone = service_components.status_manager.clone();
    tokio::spawn(async move {
        let mut receiver = config_receiver;
        while let Ok(change_event) = receiver.recv().await {
            info!("检测到配置变更，版本: {}", change_event.version);

            // 更新状态管理器中的服务列表
            for service in &change_event.new_config.services {
                status_manager_clone
                    .add_service(service.name.clone(), service.url.clone(), service.enabled)
                    .await;
            }

            // 标记配置重载
            status_manager_clone.mark_config_reload().await;

            // 重新加载调度器配置
            if let Err(e) = scheduler_clone
                .reload_config(change_event.new_config.services)
                .await
            {
                error!("配置热重载失败: {}", e);
            } else {
                info!("配置热重载成功");
            }
        }
    });

    Ok(())
}

/// 启动后台任务
///
/// 启动各种后台维护任务，目前包括：
/// - 状态保存任务：定期将服务状态保存到文件
///
/// # 参数
///
/// * `service_components` - 服务组件集合，提供状态管理器等组件
async fn start_background_tasks(service_components: &ServiceComponents) {
    // 启动状态保存任务
    let status_manager_save = service_components.status_manager.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30)); // 每30秒保存一次状态
        loop {
            interval.tick().await;
            let status_file = StatusManager::get_default_status_file_path();
            if let Err(e) = status_manager_save.save_to_file(&status_file).await {
                tracing::warn!("保存状态文件失败: {}", e);
            }
        }
    });
}

/// 处理关闭信号并清理资源
///
/// 等待关闭信号（如Ctrl+C），然后按顺序清理各种资源：
/// 1. 停止任务调度器
/// 2. 停止Web服务器（如果启动了）
/// 3. 输出停止确认信息
///
/// # 参数
///
/// * `shutdown_rx` - 关闭信号接收器
/// * `web_server_handle` - Web服务器任务句柄（可选）
/// * `scheduler` - 任务调度器，需要停止
///
/// # 返回值
///
/// 资源清理完成返回Ok，否则返回错误。
///
/// # 错误
///
/// * 任务调度器停止失败
async fn handle_shutdown_and_cleanup(
    mut shutdown_rx: broadcast::Receiver<()>,
    web_server_handle: Option<tokio::task::JoinHandle<()>>,
    scheduler: &Arc<TaskScheduler>,
) -> Result<()> {
    // 等待关闭信号
    match shutdown_rx.recv().await {
        Ok(()) => {
            info!("收到关闭信号，正在停止服务...");
        }
        Err(err) => {
            error!("等待关闭信号失败: {}", err);
        }
    }

    // 停止调度器
    scheduler.stop().await.context("停止任务调度器失败")?;

    // 停止 Web 服务器（如果启动了）
    if let Some(handle) = web_server_handle {
        handle.abort();
        info!("Web 服务器已停止");
    }

    info!("服务已停止");
    Ok(())
}
