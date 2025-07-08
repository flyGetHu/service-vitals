//! Service Vitals - 跨平台服务健康监控工具
//!
//! 这是一个用Rust编写的跨平台服务健康监控工具，支持：
//! - HTTP/HTTPS健康检测
//! - 飞书通知集成
//! - 配置热重载
//! - 多平台守护进程支持
//! - 结构化日志记录

pub mod config;
pub mod health;
pub mod cli;
pub mod notification;
pub mod error;
pub mod logging;
pub mod status;
pub mod daemon;
pub mod web;

// 重新导出主要类型
pub use config::{Config, GlobalConfig, ServiceConfig};
pub use health::{HealthChecker, HealthResult, HealthStatus};
pub use error::ServiceVitalsError;

/// 应用程序版本信息
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 应用程序名称
pub const APP_NAME: &str = env!("CARGO_PKG_NAME");

/// 应用程序描述
pub const APP_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");
