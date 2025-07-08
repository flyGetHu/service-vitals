//! 配置管理模块
//!
//! 提供配置文件解析、验证和热重载功能

pub mod types;
pub mod loader;
pub mod manager;
pub mod watcher;

// 重新导出主要类型
pub use types::{Config, GlobalConfig, ServiceConfig, validate_config};
pub use loader::{ConfigLoader, TomlConfigLoader};
pub use manager::{ConfigManager, ConfigDiff, ConfigUpdateNotification};
pub use watcher::{ConfigChangeEvent, ConfigWatcher};
