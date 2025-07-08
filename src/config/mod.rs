//! 配置管理模块
//!
//! 提供配置文件解析、验证和热重载功能

pub mod loader;
pub mod manager;
pub mod types;
pub mod watcher;

// 重新导出主要类型
pub use loader::{ConfigLoader, TomlConfigLoader};
pub use manager::{ConfigDiff, ConfigManager, ConfigUpdateNotification};
pub use types::{validate_config, Config, GlobalConfig, ServiceConfig};
pub use watcher::{ConfigChangeEvent, ConfigWatcher};
