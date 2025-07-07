//! 命令行接口模块
//!
//! 提供CLI参数解析和命令处理功能

pub mod args;
pub mod commands;

// 重新导出主要类型
pub use args::Args;
pub use commands::{Command, HelpCommand, InitCommand, StartCommand};
