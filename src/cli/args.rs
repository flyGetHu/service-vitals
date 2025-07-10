//! 命令行参数定义
//!
//! 使用clap定义应用程序的命令行接口

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use tracing::error;

/// Service Vitals - 跨平台服务健康监控工具
#[derive(Parser, Debug, Clone)]
#[command(
    name = "service-vitals",
    version = crate::VERSION,
    about = crate::APP_DESCRIPTION,
    long_about = None
)]
pub struct Args {
    /// 配置文件路径
    #[arg(
        short,
        long,
        value_name = "FILE",
        help = "配置文件路径",
        env = "SERVICE_VITALS_CONFIG"
    )]
    pub config: Option<PathBuf>,

    /// 日志级别
    #[arg(
        short,
        long,
        value_enum,
        default_value = "info",
        help = "日志级别",
        env = "SERVICE_VITALS_LOG_LEVEL"
    )]
    pub log_level: LogLevel,

    /// 是否启用详细输出
    #[arg(short, long, help = "启用详细输出")]
    pub verbose: bool,

    /// 是否以守护进程模式运行
    #[arg(short, long, help = "以守护进程模式运行")]
    pub daemon: bool,

    /// PID文件路径（守护进程模式）
    #[arg(
        long,
        value_name = "FILE",
        help = "PID文件路径（守护进程模式）",
        requires = "daemon"
    )]
    pub pid_file: Option<PathBuf>,

    /// 工作目录
    #[arg(
        long,
        value_name = "DIR",
        help = "工作目录",
        env = "SERVICE_VITALS_WORKDIR"
    )]
    pub workdir: Option<PathBuf>,

    /// 子命令
    #[command(subcommand)]
    pub command: Commands,
}

/// 日志级别枚举
#[derive(ValueEnum, Clone, Debug, PartialEq)]
pub enum LogLevel {
    /// 调试级别
    Debug,
    /// 信息级别
    Info,
    /// 警告级别
    Warn,
    /// 错误级别
    Error,
}

impl From<LogLevel> for log::LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Error => log::LevelFilter::Error,
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "error"),
        }
    }
}

/// 子命令定义
#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// 启动健康检测服务
    Start {
        /// 是否在前台运行
        #[arg(short, long, help = "在前台运行")]
        foreground: bool,

        /// 检测间隔（秒）
        #[arg(
            short,
            long,
            value_name = "SECONDS",
            help = "检测间隔（秒）",
            env = "SERVICE_VITALS_INTERVAL"
        )]
        interval: Option<u64>,

        /// 最大并发检测数
        #[arg(
            long,
            value_name = "COUNT",
            help = "最大并发检测数",
            env = "SERVICE_VITALS_MAX_CONCURRENT"
        )]
        max_concurrent: Option<usize>,
    },

    /// 停止健康检测服务
    Stop {
        /// 强制停止
        #[arg(short, long, help = "强制停止")]
        force: bool,

        /// 等待超时时间（秒）
        #[arg(
            short,
            long,
            value_name = "SECONDS",
            default_value = "30",
            help = "等待超时时间（秒）"
        )]
        timeout: u64,
    },

    /// 重启健康检测服务
    Restart {
        /// 是否在前台运行
        #[arg(short, long, help = "在前台运行")]
        foreground: bool,

        /// 等待超时时间（秒）
        #[arg(
            short,
            long,
            value_name = "SECONDS",
            default_value = "30",
            help = "等待超时时间（秒）"
        )]
        timeout: u64,
    },

    /// 查看服务状态
    Status {
        /// 输出格式
        #[arg(short, long, value_enum, default_value = "text", help = "输出格式")]
        format: OutputFormat,

        /// 是否显示详细信息
        #[arg(short, long, help = "显示详细信息")]
        verbose: bool,
    },

    /// 执行一次性健康检测
    Check {
        /// 服务名称（可选，不指定则检测所有服务）
        #[arg(value_name = "SERVICE", help = "服务名称")]
        service: Option<String>,

        /// 输出格式
        #[arg(short, long, value_enum, default_value = "text", help = "输出格式")]
        format: OutputFormat,

        /// 超时时间（秒）
        #[arg(
            short,
            long,
            value_name = "SECONDS",
            default_value = "10",
            help = "超时时间（秒）"
        )]
        timeout: u64,
    },

    /// 初始化配置文件
    Init {
        /// 配置文件路径
        #[arg(
            value_name = "FILE",
            help = "配置文件路径",
            default_value = "config.toml"
        )]
        config_path: PathBuf,

        /// 是否覆盖现有文件
        #[arg(short, long, help = "覆盖现有文件")]
        force: bool,

        /// 配置模板类型
        #[arg(
            short,
            long,
            value_enum,
            default_value = "basic",
            help = "配置模板类型"
        )]
        template: ConfigTemplate,
    },

    /// 验证配置文件
    Validate {
        /// 配置文件路径
        #[arg(value_name = "FILE", help = "配置文件路径")]
        config_path: Option<PathBuf>,

        /// 是否显示详细信息
        #[arg(short, long, help = "显示详细信息")]
        verbose: bool,
    },

    /// 显示版本信息
    Version {
        /// 输出格式
        #[arg(short, long, value_enum, default_value = "text", help = "输出格式")]
        format: OutputFormat,
    },

    /// 测试通知功能
    TestNotification {
        /// 通知类型
        #[arg(short, long, value_enum, default_value = "feishu", help = "通知类型")]
        notification_type: NotificationType,

        /// 测试消息内容
        #[arg(short, long, default_value = "这是一条测试消息", help = "测试消息内容")]
        message: String,
    },

    /// 安装系统服务
    Install {
        /// 服务名称
        #[arg(long, default_value = "service-vitals", help = "服务名称")]
        service_name: String,
        /// 服务显示名称
        #[arg(long, default_value = "Service Vitals Monitor", help = "服务显示名称")]
        display_name: String,
        /// 服务描述
        #[arg(
            long,
            default_value = "Service health monitoring and alerting system",
            help = "服务描述"
        )]
        description: String,
        /// 运行用户（仅Unix系统）
        #[arg(long, help = "运行用户（仅Unix系统）")]
        user: Option<String>,
        /// 运行组（仅Unix系统）
        #[arg(long, help = "运行组（仅Unix系统）")]
        group: Option<String>,
    },

    /// 卸载系统服务
    Uninstall {
        /// 服务名称
        #[arg(long, default_value = "service-vitals", help = "服务名称")]
        service_name: String,
    },

    /// 启动系统服务
    StartService {
        /// 服务名称
        #[arg(long, default_value = "service-vitals", help = "服务名称")]
        service_name: String,
    },

    /// 停止系统服务
    StopService {
        /// 服务名称
        #[arg(long, default_value = "service-vitals", help = "服务名称")]
        service_name: String,
    },

    /// 重启系统服务
    RestartService {
        /// 服务名称
        #[arg(long, default_value = "service-vitals", help = "服务名称")]
        service_name: String,
    },

    /// 查看系统服务状态
    ServiceStatus {
        /// 服务名称
        #[arg(long, default_value = "service-vitals", help = "服务名称")]
        service_name: String,
        /// 输出格式
        #[arg(long, value_enum, default_value = "text", help = "输出格式")]
        format: OutputFormat,
    },
}

/// 输出格式枚举
#[derive(ValueEnum, Clone, Debug, PartialEq)]
pub enum OutputFormat {
    /// 文本格式
    Text,
    /// JSON格式
    Json,
    /// YAML格式
    Yaml,
    /// 表格格式
    Table,
}

/// 配置模板类型
#[derive(ValueEnum, Clone, Debug, PartialEq)]
pub enum ConfigTemplate {
    /// 基础模板
    Basic,
    /// 完整模板
    Full,
    /// 最小模板
    Minimal,
}

/// 通知类型枚举
#[derive(ValueEnum, Clone, Debug, PartialEq)]
pub enum NotificationType {
    /// 飞书通知
    Feishu,
    /// 邮件通知（未实现）
    Email,
    /// Webhook通知（未实现）
    Webhook,
}

impl Args {
    /// 解析命令行参数
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// 获取配置文件路径
    pub fn get_config_path(&self) -> PathBuf {
        if let Some(config) = self.config.clone() {
            config
        } else {
            match crate::config::loader::get_default_config_path() {
                Ok(path) => path,
                Err(e) => {
                    error!("获取默认配置文件路径失败: {e}");
                    panic!("获取默认配置文件路径失败: {e}");
                }
            }
        }
    }

    /// 是否启用详细输出
    pub fn is_verbose(&self) -> bool {
        self.verbose || matches!(self.log_level, LogLevel::Debug)
    }
}
