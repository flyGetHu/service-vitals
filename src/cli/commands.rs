//! 命令处理逻辑
//!
//! 实现各种CLI命令的处理逻辑

use crate::cli::args::{Args, Commands, ConfigTemplate, NotificationType, OutputFormat};
use crate::common::error::Result;
use crate::common::status::{OverallStatus, StatusManager};
use crate::config::{ConfigLoader, TomlConfigLoader};
use crate::daemon::{
    service_manager::{ServiceInfo, ServiceManager},
    DaemonConfig,
};
use crate::health::{HealthChecker, HttpHealthChecker};
use crate::notification::sender::{MessageType, NotificationMessage};
use crate::notification::{FeishuSender, NotificationSender};
use async_trait::async_trait;
use chrono;
use serde::Serialize;
use std::path::Path;
use std::time::Duration;

/// 命令处理器trait
#[async_trait]
pub trait Command: Send + Sync {
    /// 执行命令
    async fn execute(&self, args: &Args) -> Result<()>;
}

/// 帮助命令
pub struct HelpCommand;

#[async_trait]
impl Command for HelpCommand {
    async fn execute(&self, _args: &Args) -> Result<()> {
        // clap会自动处理help命令
        Ok(())
    }
}

/// 版本命令
pub struct VersionCommand;

#[async_trait]
impl Command for VersionCommand {
    async fn execute(&self, args: &Args) -> Result<()> {
        if let Commands::Version { format } = &args.command {
            match format {
                OutputFormat::Json => {
                    let version_info = serde_json::json!({
                        "name": crate::APP_NAME,
                        "version": crate::VERSION,
                        "description": crate::APP_DESCRIPTION
                    });
                    println!("{}", serde_json::to_string_pretty(&version_info)?);
                }
                OutputFormat::Yaml => {
                    println!("name: {}", crate::APP_NAME);
                    println!("version: {}", crate::VERSION);
                    println!("description: {}", crate::APP_DESCRIPTION);
                }
                _ => {
                    println!("{} v{}", crate::APP_NAME, crate::VERSION);
                    println!("{}", crate::APP_DESCRIPTION);
                }
            }
        }
        Ok(())
    }
}

/// 初始化命令
pub struct InitCommand;

#[async_trait]
impl Command for InitCommand {
    async fn execute(&self, args: &Args) -> Result<()> {
        if let Commands::Init {
            config_path,
            force,
            template,
        } = &args.command
        {
            self.create_config_file(config_path, *force, template).await
        } else {
            Ok(())
        }
    }
}

impl InitCommand {
    /// 创建配置文件
    async fn create_config_file(
        &self,
        config_path: &Path,
        force: bool,
        template: &ConfigTemplate,
    ) -> Result<()> {
        // 检查文件是否已存在
        if config_path.exists() && !force {
            eprintln!("配置文件已存在: {}", config_path.display());
            eprintln!("使用 --force 参数覆盖现有文件");
            return Ok(());
        }

        // 创建目录（如果不存在）
        if let Some(parent) = config_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // 根据模板类型生成配置内容
        let config_content = match template {
            ConfigTemplate::Minimal => self.get_minimal_config(),
            ConfigTemplate::Basic => self.get_basic_config(),
            ConfigTemplate::Full => self.get_full_config(),
        };

        // 写入配置文件
        tokio::fs::write(config_path, config_content).await?;

        println!("配置文件已创建: {}", config_path.display());
        println!("请编辑配置文件以添加您的服务配置");

        Ok(())
    }

    /// 获取最小配置模板
    fn get_minimal_config(&self) -> &'static str {
        include_str!("../../examples/minimal_config.toml")
    }

    /// 获取基础配置模板
    fn get_basic_config(&self) -> &'static str {
        include_str!("../../examples/basic_config.toml")
    }

    /// 获取完整配置模板
    fn get_full_config(&self) -> &'static str {
        include_str!("../../examples/full_config.toml")
    }
}

/// 验证命令
pub struct ValidateCommand;

#[async_trait]
impl Command for ValidateCommand {
    async fn execute(&self, args: &Args) -> Result<()> {
        if let Commands::Validate {
            config_path,
            verbose,
        } = &args.command
        {
            let config_file = config_path
                .clone()
                .unwrap_or_else(|| args.get_config_path());

            self.validate_config_file(&config_file, *verbose).await
        } else {
            Ok(())
        }
    }
}

impl ValidateCommand {
    /// 验证配置文件
    async fn validate_config_file(&self, config_path: &Path, verbose: bool) -> Result<()> {
        println!("验证配置文件: {}", config_path.display());

        // 加载配置
        let loader = TomlConfigLoader::new(true);
        let config = loader.load_from_file(config_path).await?;

        if verbose {
            println!("配置验证通过！");
            println!("全局配置:");
            println!("  检测间隔: {}秒", config.global.check_interval_seconds);
            println!("  日志级别: {}", config.global.log_level);
            println!("  请求超时: {}秒", config.global.request_timeout_seconds);
            println!("  最大并发: {}", config.global.max_concurrent_checks);

            println!("服务配置:");
            for (i, service) in config.services.iter().enumerate() {
                println!("  {}. {} ({})", i + 1, service.name, service.url);
                println!("     方法: {}", service.method);
                println!("     期望状态码: {:?}", service.expected_status_codes);
                println!("     失败阈值: {}", service.failure_threshold);
                println!(
                    "     启用状态: {}",
                    if service.enabled { "是" } else { "否" }
                );
            }
        } else {
            println!("✓ 配置文件验证通过");
            println!("✓ 找到 {} 个服务配置", config.services.len());
        }

        Ok(())
    }
}

/// 检测命令
pub struct CheckCommand;

#[async_trait]
impl Command for CheckCommand {
    async fn execute(&self, args: &Args) -> Result<()> {
        if let Commands::Check {
            service,
            format,
            timeout,
        } = &args.command
        {
            self.perform_health_check(args, service.as_deref(), format, *timeout)
                .await
        } else {
            Ok(())
        }
    }
}

impl CheckCommand {
    /// 执行健康检测
    async fn perform_health_check(
        &self,
        args: &Args,
        service_name: Option<&str>,
        format: &OutputFormat,
        timeout: u64,
    ) -> Result<()> {
        // 加载配置
        let loader = TomlConfigLoader::new(true);
        let config = loader.load_from_file(args.get_config_path()).await?;

        // 创建健康检测器
        let checker = HttpHealthChecker::new(
            Duration::from_secs(timeout),
            config.global.retry_attempts,
            Duration::from_secs(config.global.retry_delay_seconds),
        )?;

        // 过滤要检测的服务
        let services_to_check: Vec<_> = if let Some(name) = service_name {
            config
                .services
                .into_iter()
                .filter(|s| s.name == name && s.enabled)
                .collect()
        } else {
            config.services.into_iter().filter(|s| s.enabled).collect()
        };

        if services_to_check.is_empty() {
            if let Some(name) = service_name {
                eprintln!("未找到名为 '{name}' 的启用服务");
            } else {
                eprintln!("未找到任何启用的服务");
            }
            return Ok(());
        }

        println!("开始健康检测...");

        // 执行检测
        let results = checker.check_batch(&services_to_check).await;

        // 输出结果
        match format {
            OutputFormat::Json => {
                let json_results: Vec<_> = results.into_iter().filter_map(|r| r.ok()).collect();
                println!("{}", serde_json::to_string_pretty(&json_results)?);
            }
            OutputFormat::Table => {
                self.print_table_results(&results);
            }
            _ => {
                self.print_text_results(&results);
            }
        }

        Ok(())
    }

    /// 打印文本格式结果
    fn print_text_results(&self, results: &[Result<crate::health::HealthResult>]) {
        for result in results {
            match result {
                Ok(health_result) => {
                    let status_icon = if health_result.status.is_healthy() {
                        "✓"
                    } else {
                        "✗"
                    };
                    println!(
                        "{} {} ({}) - {} - {}ms",
                        status_icon,
                        health_result.service_name,
                        health_result.service_url,
                        health_result.status,
                        health_result.response_time_ms()
                    );

                    if let Some(error) = &health_result.error_message {
                        println!("  错误: {error}");
                    }
                }
                Err(e) => {
                    println!("✗ 检测失败: {e}");
                }
            }
        }
    }

    /// 打印表格格式结果
    fn print_table_results(&self, results: &[Result<crate::health::HealthResult>]) {
        println!(
            "{:<20} {:<10} {:<15} {:<10} {:<30}",
            "服务名称", "状态", "状态码", "响应时间", "错误信息"
        );
        println!("{}", "-".repeat(85));

        for result in results {
            match result {
                Ok(health_result) => {
                    let status = if health_result.status.is_healthy() {
                        "正常"
                    } else {
                        "异常"
                    };
                    let status_code = health_result
                        .status_code
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "N/A".to_string());
                    let error_msg = health_result.error_message.as_deref().unwrap_or("");

                    println!(
                        "{:<20} {:<10} {:<15} {:<10} {:<30}",
                        health_result.service_name,
                        status,
                        status_code,
                        format!("{}ms", health_result.response_time_ms()),
                        error_msg
                    );
                }
                Err(e) => {
                    println!(
                        "{:<20} {:<10} {:<15} {:<10} {:<30}",
                        "未知",
                        "错误",
                        "N/A",
                        "N/A",
                        e.to_string()
                    );
                }
            }
        }
    }
}

/// 启动命令
pub struct StartCommand;

#[async_trait]
impl Command for StartCommand {
    async fn execute(&self, args: &Args) -> Result<()> {
        if let Commands::Start {
            foreground,
            interval: _,
            max_concurrent: _,
        } = &args.command
        {
            println!("启动健康检测服务...");

            if *foreground {
                println!("在前台模式运行");
                // TODO: 实现前台运行逻辑
            } else {
                println!("在后台模式运行");
                // TODO: 实现后台运行逻辑
            }

            // 这里暂时只是占位符，实际的服务启动逻辑将在任务调度器中实现
            println!("服务启动完成（占位符实现）");
        }
        Ok(())
    }
}

/// 停止命令
pub struct StopCommand;

#[async_trait]
impl Command for StopCommand {
    async fn execute(&self, _args: &Args) -> Result<()> {
        println!("停止健康检测服务...");
        // TODO: 实现服务停止逻辑
        println!("服务已停止（占位符实现）");
        Ok(())
    }
}

/// 状态命令
pub struct StatusCommand;

#[async_trait]
impl Command for StatusCommand {
    async fn execute(&self, args: &Args) -> Result<()> {
        if let Commands::Status { format, verbose } = &args.command {
            let status_file = StatusManager::get_default_status_file_path();

            // 尝试从状态文件加载状态
            match StatusManager::load_from_file(&status_file).await {
                Ok(status) => {
                    self.display_status(&status, format, *verbose).await?;
                }
                Err(_) => {
                    // 如果没有状态文件，显示服务未运行
                    match format {
                        OutputFormat::Json => {
                            let error_info = serde_json::json!({
                                "error": "服务未运行或状态文件不存在",
                                "status": "stopped"
                            });
                            println!("{}", serde_json::to_string_pretty(&error_info)?);
                        }
                        OutputFormat::Yaml => {
                            println!("error: 服务未运行或状态文件不存在");
                            println!("status: stopped");
                        }
                        OutputFormat::Text | OutputFormat::Table => {
                            println!("❌ 服务未运行或状态文件不存在");
                            println!("请使用 'service-vitals start' 启动服务");
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl StatusCommand {
    async fn display_status(
        &self,
        status: &OverallStatus,
        format: &OutputFormat,
        verbose: bool,
    ) -> Result<()> {
        match format {
            OutputFormat::Json => {
                if verbose {
                    println!("{}", serde_json::to_string_pretty(status)?);
                } else {
                    let summary = serde_json::json!({
                        "total_services": status.total_services,
                        "healthy_services": status.healthy_services,
                        "unhealthy_services": status.unhealthy_services,
                        "disabled_services": status.disabled_services,
                        "start_time": status.start_time,
                        "last_config_reload": status.last_config_reload
                    });
                    println!("{}", serde_json::to_string_pretty(&summary)?);
                }
            }
            OutputFormat::Yaml => {
                if verbose {
                    // 简单的YAML输出
                    println!("start_time: {}", status.start_time);
                    println!("config_path: {}", status.config_path.display());
                    println!("total_services: {}", status.total_services);
                    println!("healthy_services: {}", status.healthy_services);
                    println!("unhealthy_services: {}", status.unhealthy_services);
                    println!("disabled_services: {}", status.disabled_services);
                    if let Some(reload_time) = status.last_config_reload {
                        println!("last_config_reload: {reload_time}");
                    }
                    println!("services:");
                    for service in &status.services {
                        println!("  - name: {}", service.name);
                        println!("    url: {}", service.url);
                        println!("    status: {:?}", service.status);
                        println!("    enabled: {}", service.enabled);
                        if let Some(last_check) = service.last_check {
                            println!("    last_check: {last_check}");
                        }
                        if let Some(status_code) = service.status_code {
                            println!("    status_code: {status_code}");
                        }
                        if let Some(response_time) = service.response_time_ms {
                            println!("    response_time_ms: {response_time}");
                        }
                    }
                } else {
                    println!("total_services: {}", status.total_services);
                    println!("healthy_services: {}", status.healthy_services);
                    println!("unhealthy_services: {}", status.unhealthy_services);
                    println!("disabled_services: {}", status.disabled_services);
                }
            }
            OutputFormat::Text | OutputFormat::Table => {
                self.display_text_status(status, verbose).await?;
            }
        }
        Ok(())
    }

    async fn display_text_status(&self, status: &OverallStatus, verbose: bool) -> Result<()> {
        println!("🔍 Service Vitals 状态报告");
        println!(
            "生成时间: {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        );
        println!();

        // 总体状态
        println!("📊 总体状态:");
        println!(
            "  启动时间: {}",
            status.start_time.format("%Y-%m-%d %H:%M:%S UTC")
        );
        println!("  配置文件: {}", status.config_path.display());
        println!("  总服务数: {}", status.total_services);
        println!("  健康服务: {} ✅", status.healthy_services);
        println!("  异常服务: {} ❌", status.unhealthy_services);
        println!("  禁用服务: {} ⏸️", status.disabled_services);

        if let Some(reload_time) = status.last_config_reload {
            println!(
                "  最后配置重载: {}",
                reload_time.format("%Y-%m-%d %H:%M:%S UTC")
            );
        }

        println!();

        // 服务详情
        if verbose || !status.services.is_empty() {
            println!("📋 服务详情:");
            println!("┌─────────────────────────────────────────────────────────────────────────────────────┐");
            println!("│ 服务名称                │ 状态 │ 状态码 │ 响应时间 │ 最后检测时间              │");
            println!("├─────────────────────────────────────────────────────────────────────────────────────┤");

            for service in &status.services {
                let status_icon = match service.status {
                    crate::health::HealthStatus::Up => "✅",
                    crate::health::HealthStatus::Down => "❌",
                    crate::health::HealthStatus::Unknown => "❓",
                    crate::health::HealthStatus::Degraded => "⚠️",
                };

                let status_code_str = service
                    .status_code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "N/A".to_string());

                let response_time_str = service
                    .response_time_ms
                    .map(|t| format!("{t}ms"))
                    .unwrap_or_else(|| "N/A".to_string());

                let last_check_str = service
                    .last_check
                    .map(|t| t.format("%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "从未检测".to_string());

                println!(
                    "│ {:<23} │ {:<4} │ {:<6} │ {:<8} │ {:<25} │",
                    truncate_string(&service.name, 23),
                    status_icon,
                    status_code_str,
                    response_time_str,
                    last_check_str
                );

                if verbose && service.error_message.is_some() {
                    println!(
                        "│   错误: {:<71} │",
                        truncate_string(service.error_message.as_ref().unwrap(), 71)
                    );
                }
            }

            println!("└─────────────────────────────────────────────────────────────────────────────────────┘");
        }

        // 健康度总结
        let health_percentage = if status.total_services > 0 {
            (status.healthy_services as f64 / status.total_services as f64) * 100.0
        } else {
            0.0
        };

        println!();
        println!(
            "💡 健康度: {:.1}% ({}/{})",
            health_percentage, status.healthy_services, status.total_services
        );

        Ok(())
    }
}

/// 截断字符串到指定长度
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        format!("{s:<max_len$}")
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// 安装服务命令
pub struct InstallCommand;

#[async_trait]
impl Command for InstallCommand {
    async fn execute(&self, args: &Args) -> Result<()> {
        if let Commands::Install {
            service_name,
            display_name,
            description,
            user,
            group,
        } = &args.command
        {
            let service_manager = ServiceManager::new();

            // 创建守护进程配置
            let config = DaemonConfig {
                service_name: service_name.clone(),
                display_name: display_name.clone(),
                description: description.clone(),
                config_path: args.get_config_path(),
                user: user.clone(),
                group: group.clone(),
                ..Default::default()
            };

            // 验证配置
            let warnings = service_manager.validate_config(&config)?;
            if !warnings.is_empty() {
                println!("⚠️  配置警告:");
                for warning in &warnings {
                    println!("   - {warning}");
                }
                println!();
            }

            // 显示建议
            let suggestions = service_manager.suggest_config_improvements(&config);
            if !suggestions.is_empty() {
                println!("💡 配置建议:");
                for suggestion in &suggestions {
                    println!("   - {suggestion}");
                }
                println!();
            }

            // 安装服务
            println!("🔧 正在安装服务: {service_name}");
            service_manager.install_service(&config).await?;
            println!("✅ 服务安装成功!");

            // 显示下一步操作
            println!("\n📋 下一步操作:");
            println!("   启动服务: service-vitals start-service");
            println!("   查看状态: service-vitals service-status");
        }
        Ok(())
    }
}

/// 卸载服务命令
pub struct UninstallCommand;

#[async_trait]
impl Command for UninstallCommand {
    async fn execute(&self, args: &Args) -> Result<()> {
        if let Commands::Uninstall { service_name } = &args.command {
            let service_manager = ServiceManager::new();

            println!("🗑️  正在卸载服务: {service_name}");
            service_manager.uninstall_service(service_name).await?;
            println!("✅ 服务卸载成功!");
        }
        Ok(())
    }
}

/// 启动服务命令
pub struct StartServiceCommand;

#[async_trait]
impl Command for StartServiceCommand {
    async fn execute(&self, args: &Args) -> Result<()> {
        if let Commands::StartService { service_name } = &args.command {
            let service_manager = ServiceManager::new();

            println!("▶️  正在启动服务: {service_name}");
            service_manager.start_service(service_name).await?;
            println!("✅ 服务启动成功!");
        }
        Ok(())
    }
}

/// 停止服务命令
pub struct StopServiceCommand;

#[async_trait]
impl Command for StopServiceCommand {
    async fn execute(&self, args: &Args) -> Result<()> {
        if let Commands::StopService { service_name } = &args.command {
            let service_manager = ServiceManager::new();

            println!("⏹️  正在停止服务: {service_name}");
            service_manager.stop_service(service_name).await?;
            println!("✅ 服务停止成功!");
        }
        Ok(())
    }
}

/// 重启服务命令
pub struct RestartServiceCommand;

#[async_trait]
impl Command for RestartServiceCommand {
    async fn execute(&self, args: &Args) -> Result<()> {
        if let Commands::RestartService { service_name } = &args.command {
            let service_manager = ServiceManager::new();

            println!("🔄 正在重启服务: {service_name}");
            service_manager.restart_service(service_name).await?;
            println!("✅ 服务重启成功!");
        }
        Ok(())
    }
}

/// 服务状态命令
pub struct ServiceStatusCommand;

#[async_trait]
impl Command for ServiceStatusCommand {
    async fn execute(&self, args: &Args) -> Result<()> {
        if let Commands::ServiceStatus {
            service_name,
            format,
            verbose,
        } = &args.command
        {
            let service_manager = ServiceManager::new();

            // 获取系统服务状态
            let service_info = service_manager.get_service_status(service_name).await?;

            // 尝试从状态文件加载应用状态
            let status_file = StatusManager::get_default_status_file_path();
            let app_status = StatusManager::load_from_file(&status_file).await.ok();

            // 根据格式输出
            match format {
                OutputFormat::Json => {
                    self.display_json_status(&service_info, &app_status, *verbose)
                        .await?;
                }
                OutputFormat::Yaml => {
                    self.display_yaml_status(&service_info, &app_status, *verbose)
                        .await?;
                }
                OutputFormat::Text | OutputFormat::Table => {
                    self.display_text_status(&service_info, &app_status, *verbose)
                        .await?;
                }
            }
        }
        Ok(())
    }
}

impl ServiceStatusCommand {
    /// 显示JSON格式状态
    async fn display_json_status(
        &self,
        service_info: &ServiceInfo,
        app_status: &Option<OverallStatus>,
        verbose: bool,
    ) -> Result<()> {
        if verbose {
            let combined_status = serde_json::json!({
                "system_service": {
                    "name": service_info.name,
                    "status": service_info.status,
                    "is_installed": service_info.is_installed,
                    "platform": service_info.platform
                },
                "application_status": app_status,
                "metrics_update_check": self.check_metrics_updates(service_info, app_status).await
            });
            println!("{}", serde_json::to_string_pretty(&combined_status)?);
        } else {
            let combined_status = serde_json::json!({
                "system_service": {
                    "name": service_info.name,
                    "status": service_info.status,
                    "is_installed": service_info.is_installed,
                    "platform": service_info.platform
                }
            });
            println!("{}", serde_json::to_string_pretty(&combined_status)?);
        }
        Ok(())
    }

    /// 显示YAML格式状态
    async fn display_yaml_status(
        &self,
        service_info: &ServiceInfo,
        app_status: &Option<OverallStatus>,
        verbose: bool,
    ) -> Result<()> {
        println!("system_service:");
        println!("  name: {}", service_info.name);
        println!("  status: {:?}", service_info.status);
        println!("  is_installed: {}", service_info.is_installed);
        println!("  platform: {}", service_info.platform);

        if let Some(status) = app_status {
            println!("application_status:");
            println!("  start_time: {}", status.start_time);
            println!("  config_path: {}", status.config_path.display());
            println!("  total_services: {}", status.total_services);
            println!("  healthy_services: {}", status.healthy_services);
            println!("  unhealthy_services: {}", status.unhealthy_services);
            println!("  disabled_services: {}", status.disabled_services);
            if let Some(reload_time) = status.last_config_reload {
                println!("  last_config_reload: {reload_time}");
            }

            // 在 verbose 模式下显示服务详情
            if verbose && !status.services.is_empty() {
                println!("  services:");
                for service in &status.services {
                    println!("    - name: {}", service.name);
                    println!("      url: {}", service.url);
                    println!("      status: {:?}", service.status);
                    println!("      enabled: {}", service.enabled);
                    if let Some(last_check) = service.last_check {
                        println!("      last_check: {last_check}");
                    }
                    if let Some(status_code) = service.status_code {
                        println!("      status_code: {status_code}");
                    }
                    if let Some(response_time) = service.response_time_ms {
                        println!("      response_time_ms: {response_time}");
                    }
                    if let Some(ref error) = service.error_message {
                        println!("      error_message: {error}");
                    }
                }
            }
        } else {
            println!("application_status: null");
        }

        let metrics_check = self.check_metrics_updates(service_info, app_status).await;
        println!("metrics_update_check:");
        println!("  is_updating: {}", metrics_check.is_updating);
        println!(
            "  last_update_age_seconds: {}",
            metrics_check.last_update_age_seconds.unwrap_or(0)
        );
        println!("  status_summary: {}", metrics_check.status_summary);

        Ok(())
    }

    /// 显示文本格式状态
    async fn display_text_status(
        &self,
        service_info: &ServiceInfo,
        app_status: &Option<OverallStatus>,
        verbose: bool,
    ) -> Result<()> {
        println!("🔍 Service Vitals 系统服务状态报告");
        println!(
            "生成时间: {}",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        );
        println!();

        // 系统服务状态
        println!("📋 系统服务状态:");
        println!("  服务名称: {}", service_info.name);
        println!("  平台: {}", service_info.platform);
        println!(
            "  安装状态: {}",
            if service_info.is_installed {
                "✅ 已安装"
            } else {
                "❌ 未安装"
            }
        );

        let status_display = match service_info.status {
            crate::daemon::DaemonStatus::Running => "✅ 运行中",
            crate::daemon::DaemonStatus::Stopped => "⏹️ 已停止",
            crate::daemon::DaemonStatus::Starting => "🔄 启动中",
            crate::daemon::DaemonStatus::Stopping => "⏹️ 停止中",
            crate::daemon::DaemonStatus::Unknown => "❓ 未知",
        };
        println!("  运行状态: {status_display}");
        println!();

        // 应用监控状态
        if let Some(status) = app_status {
            println!("📊 应用监控状态:");
            println!(
                "  启动时间: {}",
                status.start_time.format("%Y-%m-%d %H:%M:%S UTC")
            );
            println!("  配置文件: {}", status.config_path.display());
            println!(
                "  运行时长: {}",
                self.format_duration(chrono::Utc::now() - status.start_time)
            );
            println!();

            // 服务统计
            println!("  📈 服务统计:");
            println!("    总服务数: {}", status.total_services);
            println!("    健康服务: {} ✅", status.healthy_services);
            println!("    异常服务: {} ❌", status.unhealthy_services);
            println!("    禁用服务: {} ⏸️", status.disabled_services);

            // 健康度计算
            let health_percentage = if status.total_services > 0 {
                (status.healthy_services as f64 / status.total_services as f64) * 100.0
            } else {
                0.0
            };
            println!("    健康度: {:.1}%", health_percentage);

            if let Some(reload_time) = status.last_config_reload {
                println!(
                    "    最后配置重载: {}",
                    reload_time.format("%Y-%m-%d %H:%M:%S UTC")
                );
            }
            println!();

            // 服务检测活动
            if !status.services.is_empty() {
                if verbose {
                    // verbose 模式：显示完整的服务表格
                    println!("  📋 服务详情:");
                    println!("┌─────────────────────────────────────────────────────────────────────────────────────┐");
                    println!("│ 服务名称                │ 状态 │ 状态码 │ 响应时间 │ 最后检测时间              │");
                    println!("├─────────────────────────────────────────────────────────────────────────────────────┤");

                    for service in &status.services {
                        let status_icon = match service.status {
                            crate::health::HealthStatus::Up => "✅",
                            crate::health::HealthStatus::Down => "❌",
                            crate::health::HealthStatus::Unknown => "❓",
                            crate::health::HealthStatus::Degraded => "⚠️",
                        };

                        let status_code_str = service
                            .status_code
                            .map(|c| c.to_string())
                            .unwrap_or_else(|| "N/A".to_string());

                        let response_time_str = service
                            .response_time_ms
                            .map(|t| format!("{t}ms"))
                            .unwrap_or_else(|| "N/A".to_string());

                        let last_check_str = service
                            .last_check
                            .map(|t| t.format("%m-%d %H:%M:%S").to_string())
                            .unwrap_or_else(|| "从未检测".to_string());

                        println!(
                            "│ {:<23} │ {:<4} │ {:<6} │ {:<8} │ {:<25} │",
                            truncate_string(&service.name, 23),
                            status_icon,
                            status_code_str,
                            response_time_str,
                            last_check_str
                        );

                        if let Some(ref error) = service.error_message {
                            println!("│   错误: {:<71} │", truncate_string(error, 71));
                        }
                    }

                    println!("└─────────────────────────────────────────────────────────────────────────────────────┘");
                } else {
                    // 简化模式：显示最近的检测活动
                    println!("  📋 最近检测活动:");
                    let mut recent_checks: Vec<_> = status
                        .services
                        .iter()
                        .filter(|s| s.last_check.is_some())
                        .collect();
                    recent_checks.sort_by(|a, b| {
                        b.last_check
                            .unwrap_or_default()
                            .cmp(&a.last_check.unwrap_or_default())
                    });

                    for (i, service) in recent_checks.iter().take(5).enumerate() {
                        let status_icon = match service.status {
                            crate::health::HealthStatus::Up => "✅",
                            crate::health::HealthStatus::Down => "❌",
                            crate::health::HealthStatus::Unknown => "❓",
                            crate::health::HealthStatus::Degraded => "⚠️",
                        };

                        let last_check_str = service
                            .last_check
                            .map(|t| self.format_relative_time(chrono::Utc::now() - t))
                            .unwrap_or_else(|| "从未".to_string());

                        println!(
                            "    {}. {} {} ({}前)",
                            i + 1,
                            status_icon,
                            service.name,
                            last_check_str
                        );
                    }
                    if recent_checks.len() > 5 {
                        println!(
                            "    ... 还有 {} 个服务，使用 --verbose 查看完整列表",
                            recent_checks.len() - 5
                        );
                    }
                }
                println!();
            }
        } else {
            println!("📊 应用监控状态: ❌ 未运行或状态文件不存在");
            println!("    请使用 'service-vitals start' 启动监控服务");
            println!();
        }

        // 指标更新检查
        let metrics_check = self.check_metrics_updates(service_info, app_status).await;
        println!("🔄 指标更新检查:");
        println!(
            "  更新状态: {}",
            if metrics_check.is_updating {
                "✅ 正常更新"
            } else {
                "❌ 更新异常"
            }
        );

        if let Some(age_seconds) = metrics_check.last_update_age_seconds {
            if age_seconds > 0 {
                println!(
                    "  最后更新: {}前",
                    self.format_duration_seconds(age_seconds)
                );
            } else {
                println!("  最后更新: 刚刚");
            }
        } else {
            println!("  最后更新: 无记录");
        }

        println!("  状态总结: {}", metrics_check.status_summary);

        // 建议操作
        if !metrics_check.is_updating {
            println!();
            println!("💡 建议操作:");
            if service_info.status != crate::daemon::DaemonStatus::Running {
                println!("  - 启动系统服务: service-vitals start-service");
            }
            if app_status.is_none() {
                println!("  - 检查服务配置和日志");
                println!("  - 手动启动测试: service-vitals start --foreground");
            } else {
                println!("  - 检查服务日志: journalctl -u service-vitals -f");
                println!("  - 重启服务: service-vitals restart-service");
            }
        }

        Ok(())
    }

    /// 检查指标更新状态
    async fn check_metrics_updates(
        &self,
        service_info: &ServiceInfo,
        app_status: &Option<OverallStatus>,
    ) -> MetricsUpdateCheck {
        let mut is_updating = false;
        let mut last_update_age_seconds = None;
        let status_summary;

        match (&service_info.status, app_status) {
            (crate::daemon::DaemonStatus::Running, Some(status)) => {
                // 系统服务运行中且有应用状态
                if let Some(most_recent_check) =
                    status.services.iter().filter_map(|s| s.last_check).max()
                {
                    let age = (chrono::Utc::now() - most_recent_check).num_seconds() as u64;
                    last_update_age_seconds = Some(age);

                    // 如果最近5分钟内有更新，认为是正常的
                    is_updating = age < 300;

                    status_summary = if is_updating {
                        "服务正常运行，指标持续更新".to_string()
                    } else {
                        format!(
                            "服务运行但指标更新滞后（{}前）",
                            self.format_duration_seconds(age)
                        )
                    };
                } else {
                    status_summary = "服务运行中，但尚无检测记录".to_string();
                }
            }
            (crate::daemon::DaemonStatus::Running, None) => {
                status_summary = "系统服务运行中，但应用状态不可用".to_string();
            }
            (status, _) => {
                status_summary = format!("系统服务状态: {:?}", status);
            }
        }

        MetricsUpdateCheck {
            is_updating,
            last_update_age_seconds,
            status_summary,
        }
    }

    /// 格式化持续时间
    fn format_duration(&self, duration: chrono::Duration) -> String {
        let total_seconds = duration.num_seconds();

        if total_seconds < 60 {
            format!("{}秒", total_seconds)
        } else if total_seconds < 3600 {
            format!("{}分钟", total_seconds / 60)
        } else if total_seconds < 86400 {
            let hours = total_seconds / 3600;
            let minutes = (total_seconds % 3600) / 60;
            format!("{}小时{}分钟", hours, minutes)
        } else {
            let days = total_seconds / 86400;
            let hours = (total_seconds % 86400) / 3600;
            format!("{}天{}小时", days, hours)
        }
    }

    /// 格式化相对时间
    fn format_relative_time(&self, duration: chrono::Duration) -> String {
        let total_seconds = duration.num_seconds();

        if total_seconds < 60 {
            format!("{}秒", total_seconds)
        } else if total_seconds < 3600 {
            format!("{}分钟", total_seconds / 60)
        } else if total_seconds < 86400 {
            format!("{}小时", total_seconds / 3600)
        } else {
            format!("{}天", total_seconds / 86400)
        }
    }

    /// 格式化秒数为可读格式
    fn format_duration_seconds(&self, seconds: u64) -> String {
        if seconds < 60 {
            format!("{}秒", seconds)
        } else if seconds < 3600 {
            format!("{}分钟", seconds / 60)
        } else if seconds < 86400 {
            let hours = seconds / 3600;
            let minutes = (seconds % 3600) / 60;
            if minutes > 0 {
                format!("{}小时{}分钟", hours, minutes)
            } else {
                format!("{}小时", hours)
            }
        } else {
            let days = seconds / 86400;
            let hours = (seconds % 86400) / 3600;
            if hours > 0 {
                format!("{}天{}小时", days, hours)
            } else {
                format!("{}天", days)
            }
        }
    }
}

/// 指标更新检查结果
#[derive(Debug, Clone, Serialize)]
struct MetricsUpdateCheck {
    /// 是否正在更新
    is_updating: bool,
    /// 最后更新距离现在的秒数
    last_update_age_seconds: Option<u64>,
    /// 状态总结
    status_summary: String,
}

/// 测试通知命令
pub struct TestNotificationCommand;

#[async_trait]
impl Command for TestNotificationCommand {
    async fn execute(&self, args: &Args) -> Result<()> {
        if let Commands::TestNotification {
            notification_type,
            message,
        } = &args.command
        {
            self.test_notification(args, notification_type, message)
                .await
        } else {
            Ok(())
        }
    }
}

impl TestNotificationCommand {
    /// 测试通知功能
    async fn test_notification(
        &self,
        args: &Args,
        notification_type: &NotificationType,
        message: &str,
    ) -> Result<()> {
        println!("测试通知功能...");

        match notification_type {
            NotificationType::Feishu => self.test_feishu_notification(args, message).await,
            NotificationType::Email => {
                println!("邮件通知功能尚未实现");
                Ok(())
            }
            NotificationType::Webhook => {
                println!("Webhook通知功能尚未实现");
                Ok(())
            }
        }
    }

    /// 测试飞书通知
    async fn test_feishu_notification(&self, args: &Args, message: &str) -> Result<()> {
        // 加载配置
        let loader = TomlConfigLoader::new(true);
        let config = loader.load_from_file(args.get_config_path()).await?;

        // 检查是否配置了飞书webhook
        let webhook_url = match config.global.default_feishu_webhook_url {
            Some(url) => url,
            None => {
                println!("❌ 未配置飞书webhook URL");
                println!("请在配置文件中设置 global.default_feishu_webhook_url");
                return Ok(());
            }
        };

        println!("🔗 使用webhook URL: {webhook_url}");

        // 创建飞书发送器
        let sender = FeishuSender::new(Some(webhook_url))?;

        // 创建测试消息
        let test_message = NotificationMessage {
            title: "🧪 Service Vitals 通知测试".to_string(),
            content: format!(
                "**测试时间**: {}\n**测试消息**: {}\n\n这是一条来自 Service Vitals 的测试通知，用于验证通知功能是否正常工作。",
                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
                message
            ),
            service_name: "test-service".to_string(),
            service_url: "https://example.com".to_string(),
            message_type: MessageType::Info,
        };

        // 发送测试消息
        println!("📤 发送测试消息...");
        match sender.send_message(&test_message).await {
            Ok(()) => {
                println!("✅ 测试消息发送成功！");
                println!("请检查您的飞书群组是否收到测试消息。");
            }
            Err(e) => {
                println!("❌ 测试消息发送失败: {e}");
                println!("请检查：");
                println!("  1. webhook URL是否正确");
                println!("  2. 网络连接是否正常");
                println!("  3. 飞书机器人是否已添加到群组");
            }
        }

        Ok(())
    }
}
