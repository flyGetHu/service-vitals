//! 健康检测模块
//!
//! 提供HTTP健康检测、结果处理和任务调度功能

pub mod checker;
pub mod result;
pub mod scheduler;

// 重新导出主要类型
pub use checker::{HealthChecker, HttpHealthChecker};
pub use result::{HealthResult, HealthStatus};
pub use scheduler::{Scheduler, TaskScheduler};
