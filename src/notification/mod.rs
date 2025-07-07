//! 通知模块
//!
//! 提供飞书通知和消息模板功能

pub mod sender;
pub mod feishu;
pub mod template;

// 重新导出主要类型
pub use sender::NotificationSender;
pub use feishu::FeishuSender;
pub use template::{MessageTemplate, HandlebarsTemplate};
