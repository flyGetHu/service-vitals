//! 通知发送器模块
//!
//! 定义通知发送的trait和基础实现

use crate::config::types::ServiceConfig;
use crate::health::HealthResult;
use anyhow::Result;
use async_trait::async_trait;

/// 通知消息结构
#[derive(Debug, Clone)]
pub struct NotificationMessage {
    /// 消息标题
    pub title: String,
    /// 消息内容
    pub content: String,
    /// 服务名称
    pub service_name: String,
    /// 服务URL
    pub service_url: String,
    /// 消息类型
    pub message_type: MessageType,
}

/// 消息类型
#[derive(Debug, Clone)]
pub enum MessageType {
    /// 告警消息
    Alert,
    /// 恢复消息
    Recovery,
    /// 信息消息
    Info,
}

/// 通知发送器trait
#[async_trait]
pub trait NotificationSender: Send + Sync {
    /// 发送健康检测告警
    ///
    /// # 参数
    /// * `service` - 服务配置
    /// * `result` - 健康检测结果
    ///
    /// # 返回
    /// * `Result<()>` - 发送结果
    async fn send_health_alert(
        &self,
        service: &ServiceConfig,
        result: &HealthResult,
    ) -> Result<()>;

    /// 发送自定义消息
    ///
    /// # 参数
    /// * `message` - 通知消息
    ///
    /// # 返回
    /// * `Result<()>` - 发送结果
    async fn send_message(&self, message: &NotificationMessage) -> Result<()>;

    /// 测试连接
    ///
    /// # 返回
    /// * `Result<()>` - 测试结果
    async fn test_connection(&self) -> Result<()>;
}

/// 空的通知发送器实现（用于测试或禁用通知）
pub struct NoOpSender;

#[async_trait]
impl NotificationSender for NoOpSender {
    async fn send_health_alert(
        &self,
        _service: &ServiceConfig,
        _result: &HealthResult,
    ) -> Result<()> {
        // 不执行任何操作
        Ok(())
    }

    async fn send_message(&self, _message: &NotificationMessage) -> Result<()> {
        // 不执行任何操作
        Ok(())
    }

    async fn test_connection(&self) -> Result<()> {
        // 总是返回成功
        Ok(())
    }
}
