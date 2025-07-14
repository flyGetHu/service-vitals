//! 核心模块
//!
//! 包含应用程序的核心逻辑和生命周期管理

pub mod app;
pub mod daemon_service;
pub mod foreground_service;
pub mod service;

// 重新导出主要类型
pub use app::execute_command;
pub use daemon_service::DaemonService;
pub use foreground_service::ForegroundService;
pub use service::{ServiceComponents, ServiceLauncher, ServiceManager};
