//! Service Vitals - 跨平台服务健康监控工具
//!
//! 这是一个用Rust编写的跨平台服务健康监控工具，支持：
//! - HTTP/HTTPS健康检测
//! - 飞书通知集成
//! - 配置热重载
//! - 多平台守护进程支持
//! - 结构化日志记录

pub mod cli;
pub mod config;
pub mod daemon;
pub mod error;
pub mod health;
pub mod logging;
pub mod notification;
pub mod status;
pub mod web;

// 新增的模块
pub mod app;
pub mod service;
pub mod daemon_service;
pub mod foreground_service;

// 重新导出主要类型
pub use config::{Config, GlobalConfig, ServiceConfig};
pub use error::ServiceVitalsError;
pub use health::{HealthChecker, HealthResult, HealthStatus};

/// 应用程序版本信息
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 应用程序名称
pub const APP_NAME: &str = env!("CARGO_PKG_NAME");

/// 应用程序描述
pub const APP_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");
