//! 命令处理逻辑
//!
//! 实现各种CLI命令的处理逻辑

use crate::cli::args::{Args, Commands, ConfigTemplate, OutputFormat};
use crate::config::{ConfigLoader, TomlConfigLoader};
use crate::error::Result;
use crate::health::{HealthChecker, HttpHealthChecker};
use async_trait::async_trait;
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
        r#"[global]
# 最小配置只需要指定必要的全局设置

[[services]]
name = "示例服务"
url = "https://httpbin.org/status/200"
expected_status_codes = [200]
"#
    }

    /// 获取基础配置模板
    fn get_basic_config(&self) -> &'static str {
        r#"[global]
# 全局检测间隔，单位秒（默认60）
check_interval_seconds = 60

# 日志级别（可选，默认"info"）
log_level = "info"

# 请求超时时间，单位秒（默认10）
request_timeout_seconds = 10

# 最大并发检测数（默认50）
max_concurrent_checks = 50

[[services]]
name = "示例API服务"
url = "https://api.example.com/health"
method = "GET"
expected_status_codes = [200, 201]
failure_threshold = 2
enabled = true
description = "示例API健康检测"

[[services]]
name = "示例Web服务"
url = "https://www.example.com"
method = "GET"
expected_status_codes = [200]
failure_threshold = 1
enabled = true
description = "示例Web服务健康检测"
"#
    }

    /// 获取完整配置模板
    fn get_full_config(&self) -> &'static str {
        include_str!("../../examples/config.toml")
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
                .as_ref()
                .map(|p| p.clone())
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
        let config = loader.load_from_file(&args.get_config_path()).await?;

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
                eprintln!("未找到名为 '{}' 的启用服务", name);
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
                        println!("  错误: {}", error);
                    }
                }
                Err(e) => {
                    println!("✗ 检测失败: {}", e);
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
    async fn execute(&self, _args: &Args) -> Result<()> {
        println!("查看服务状态...");
        // TODO: 实现状态查看逻辑
        println!("服务状态: 运行中（占位符实现）");
        Ok(())
    }
}
