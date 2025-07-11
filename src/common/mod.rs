//! 通用模块
//!
//! 包含错误处理、日志系统等通用功能

pub mod error;
pub mod logging;
pub mod status;

// 重新导出主要类型
pub use error::{ServiceVitalsError, Result, ErrorSeverity, RecoveryStrategy, ErrorContext};
pub use logging::{LogConfig, LoggingSystem};
pub use status::{ServiceStatus, StatusManager};