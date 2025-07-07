//! 飞书通知发送器模块
//!
//! 实现飞书webhook通知功能

use crate::config::types::ServiceConfig;
use crate::health::HealthResult;
use crate::notification::sender::{MessageType, NotificationMessage, NotificationSender};
use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tracing::{debug, error, info};

/// 飞书通知发送器
pub struct FeishuSender {
    /// HTTP客户端
    client: Client,
    /// 默认webhook URL
    default_webhook_url: Option<String>,
}

impl FeishuSender {
    /// 创建新的飞书发送器
    ///
    /// # 参数
    /// * `default_webhook_url` - 默认webhook URL
    ///
    /// # 返回
    /// * `Result<Self>` - 发送器实例
    pub fn new(default_webhook_url: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("创建HTTP客户端失败")?;

        Ok(Self {
            client,
            default_webhook_url,
        })
    }

    /// 构建飞书消息体
    fn build_message_body(&self, message: &NotificationMessage) -> Value {
        let color = match message.message_type {
            MessageType::Alert => "red",
            MessageType::Recovery => "green",
            MessageType::Info => "blue",
        };

        json!({
            "msg_type": "interactive",
            "card": {
                "elements": [
                    {
                        "tag": "div",
                        "text": {
                            "content": message.content,
                            "tag": "lark_md"
                        }
                    }
                ],
                "header": {
                    "title": {
                        "content": message.title,
                        "tag": "plain_text"
                    },
                    "template": color
                }
            }
        })
    }

    /// 发送消息到飞书
    async fn send_to_webhook(&self, webhook_url: &str, body: &Value) -> Result<()> {
        debug!("发送消息到飞书webhook: {}", webhook_url);

        let response = self
            .client
            .post(webhook_url)
            .json(body)
            .send()
            .await
            .context("发送飞书消息失败")?;

        if response.status().is_success() {
            info!("飞书消息发送成功");
            Ok(())
        } else {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            error!("飞书消息发送失败: {} - {}", status, text);
            Err(anyhow::anyhow!("飞书消息发送失败: {}", status))
        }
    }

    /// 获取webhook URL
    fn get_webhook_url(&self, service: &ServiceConfig) -> Option<String> {
        service
            .feishu_webhook_url
            .clone()
            .or_else(|| self.default_webhook_url.clone())
    }
}

#[async_trait]
impl NotificationSender for FeishuSender {
    async fn send_health_alert(
        &self,
        service: &ServiceConfig,
        result: &HealthResult,
    ) -> Result<()> {
        let webhook_url = match self.get_webhook_url(service) {
            Some(url) => url,
            None => {
                debug!("服务 {} 未配置飞书webhook URL，跳过通知", service.name);
                return Ok(());
            }
        };

        let message = NotificationMessage {
            title: format!("🚨 服务告警 - {}", service.name),
            content: format!(
                "**服务名称**: {}\n**服务URL**: {}\n**状态**: {}\n**响应时间**: {}ms\n**检测时间**: {}",
                service.name,
                service.url,
                if result.status.is_healthy() { "正常" } else { "异常" },
                result.response_time_ms(),
                result.timestamp.format("%Y-%m-%d %H:%M:%S")
            ),
            service_name: service.name.clone(),
            service_url: service.url.clone(),
            message_type: MessageType::Alert,
        };

        let body = self.build_message_body(&message);
        self.send_to_webhook(&webhook_url, &body).await
    }

    async fn send_message(&self, message: &NotificationMessage) -> Result<()> {
        // 对于自定义消息，需要有默认的webhook URL
        let webhook_url = match &self.default_webhook_url {
            Some(url) => url,
            None => {
                return Err(anyhow::anyhow!("未配置默认飞书webhook URL"));
            }
        };

        let body = self.build_message_body(message);
        self.send_to_webhook(webhook_url, &body).await
    }

    async fn test_connection(&self) -> Result<()> {
        let webhook_url = match &self.default_webhook_url {
            Some(url) => url,
            None => {
                return Err(anyhow::anyhow!("未配置飞书webhook URL"));
            }
        };

        let test_message = NotificationMessage {
            title: "连接测试".to_string(),
            content: "这是一条测试消息，用于验证飞书webhook连接是否正常。".to_string(),
            service_name: "test".to_string(),
            service_url: "test".to_string(),
            message_type: MessageType::Info,
        };

        let body = self.build_message_body(&test_message);
        self.send_to_webhook(webhook_url, &body).await
    }
}
