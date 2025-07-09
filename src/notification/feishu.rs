//! 飞书通知发送器模块
//!
//! 实现飞书webhook通知功能，支持多种消息格式和重试机制

use crate::config::types::ServiceConfig;
use crate::health::HealthResult;
use crate::notification::sender::{MessageType, NotificationMessage, NotificationSender};
use crate::notification::template::{
    create_default_alert_template, create_default_recovery_template, MessageTemplate,
    TemplateContext,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// 飞书消息格式类型
#[derive(Debug, Clone)]
pub enum FeishuMessageFormat {
    /// 纯文本消息
    Text,
    /// 富文本消息（Markdown）
    RichText,
    /// 交互式卡片消息
    Card,
}

/// 飞书发送器配置
#[derive(Debug, Clone)]
pub struct FeishuConfig {
    /// 默认webhook URL
    pub webhook_url: Option<String>,
    /// 签名密钥（可选）
    pub secret: Option<String>,
    /// 是否@所有人
    pub mention_all: bool,
    /// @特定用户的user_id列表
    pub mention_users: Vec<String>,
    /// 重试次数
    pub max_retries: u32,
    /// 重试间隔（秒）
    pub retry_delay: u64,
    /// 请求超时时间（秒）
    pub timeout: u64,
    /// 默认消息格式
    pub default_format: FeishuMessageFormat,
}

impl Default for FeishuConfig {
    fn default() -> Self {
        Self {
            webhook_url: None,
            secret: None,
            mention_all: true,
            mention_users: Vec::new(),
            max_retries: 3,
            retry_delay: 5,
            timeout: 30,
            default_format: FeishuMessageFormat::Card,
        }
    }
}

/// 飞书通知发送器
pub struct FeishuSender {
    /// HTTP客户端
    client: Client,
    /// 发送器配置
    config: FeishuConfig,
    /// 消息模板
    alert_template: Box<dyn MessageTemplate>,
    /// 恢复消息模板
    recovery_template: Box<dyn MessageTemplate>,
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
        let config = FeishuConfig {
            webhook_url: default_webhook_url,
            ..Default::default()
        };
        Self::new_with_config(config)
    }

    /// 使用配置创建飞书发送器
    ///
    /// # 参数
    /// * `config` - 飞书配置
    ///
    /// # 返回
    /// * `Result<Self>` - 发送器实例
    pub fn new_with_config(config: FeishuConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout))
            .build()
            .context("创建HTTP客户端失败")?;

        // 创建默认模板
        let alert_template = create_default_alert_template().context("创建默认告警模板失败")?;
        let recovery_template =
            create_default_recovery_template().context("创建默认恢复模板失败")?;

        Ok(Self {
            client,
            config,
            alert_template,
            recovery_template,
        })
    }

    /// 设置消息模板
    ///
    /// # 参数
    /// * `alert_template` - 告警消息模板
    /// * `recovery_template` - 恢复消息模板
    pub fn set_templates(
        &mut self,
        alert_template: Box<dyn MessageTemplate>,
        recovery_template: Box<dyn MessageTemplate>,
    ) {
        self.alert_template = alert_template;
        self.recovery_template = recovery_template;
    }

    /// 构建纯文本消息体
    fn build_text_message(&self, content: &str) -> Value {
        let mut message = json!({
            "msg_type": "text",
            "content": {
                "text": content
            }
        });

        // 添加@功能
        if self.config.mention_all || !self.config.mention_users.is_empty() {
            let mut at = json!({});

            if self.config.mention_all {
                at["isAtAll"] = json!(true);
            }

            if !self.config.mention_users.is_empty() {
                at["atUserIds"] = json!(self.config.mention_users);
            }

            message["content"]["at"] = at;
        }

        message
    }

    /// 构建富文本消息体
    fn build_rich_text_message(&self, content: &str) -> Value {
        json!({
            "msg_type": "post",
            "content": {
                "post": {
                    "zh_cn": {
                        "title": "",
                        "content": [
                            [
                                {
                                    "tag": "text",
                                    "text": content
                                }
                            ]
                        ]
                    }
                }
            }
        })
    }

    /// 构建交互式卡片消息体
    fn build_card_message(&self, message: &NotificationMessage) -> Value {
        let color = match message.message_type {
            MessageType::Alert => "red",
            MessageType::Recovery => "green",
            MessageType::Info => "blue",
        };

        let mut elements = vec![json!({
            "tag": "div",
            "text": {
                "content": message.content,
                "tag": "lark_md"
            }
        })];

        // 添加@功能到卡片
        if self.config.mention_all || !self.config.mention_users.is_empty() {
            let mut at_text = String::new();

            if self.config.mention_all {
                at_text.push_str("<at id=all></at>");
            }

            for user_id in &self.config.mention_users {
                at_text.push_str(&format!("<at id={user_id}></at>"));
            }

            if !at_text.is_empty() {
                elements.push(json!({
                    "tag": "div",
                    "text": {
                        "content": at_text,
                        "tag": "lark_md"
                    }
                }));
            }
        }

        json!({
            "msg_type": "interactive",
            "card": {
                "elements": elements,
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

    /// 构建消息体
    fn build_message_body(
        &self,
        message: &NotificationMessage,
        format: &FeishuMessageFormat,
    ) -> Value {
        match format {
            FeishuMessageFormat::Text => {
                let content = format!("{}\n{}", message.title, message.content);
                self.build_text_message(&content)
            }
            FeishuMessageFormat::RichText => {
                let content = format!("{}\n{}", message.title, message.content);
                self.build_rich_text_message(&content)
            }
            FeishuMessageFormat::Card => self.build_card_message(message),
        }
    }

    /// 发送消息到飞书（带重试机制）
    async fn send_to_webhook(&self, webhook_url: &str, body: &Value) -> Result<()> {
        let mut last_error = None;

        for attempt in 0..=self.config.max_retries {
            debug!(
                "发送消息到飞书webhook: {} (尝试 {}/{})",
                webhook_url,
                attempt + 1,
                self.config.max_retries + 1
            );

            match self.send_single_request(webhook_url, body).await {
                Ok(()) => {
                    if attempt > 0 {
                        info!("飞书消息发送成功 (重试 {} 次后)", attempt);
                    } else {
                        info!("飞书消息发送成功");
                    }
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);

                    if attempt < self.config.max_retries {
                        warn!(
                            "飞书消息发送失败，{}秒后重试: {}",
                            self.config.retry_delay,
                            last_error.as_ref().unwrap()
                        );
                        sleep(Duration::from_secs(self.config.retry_delay)).await;
                    }
                }
            }
        }

        error!(
            "飞书消息发送失败，已重试 {} 次: {}",
            self.config.max_retries,
            last_error.as_ref().unwrap()
        );
        Err(last_error.unwrap())
    }

    /// 发送单次请求
    async fn send_single_request(&self, webhook_url: &str, body: &Value) -> Result<()> {
        let mut request = self.client.post(webhook_url).json(body);

        // 如果配置了签名密钥，添加签名
        if let Some(ref secret) = self.config.secret {
            let timestamp = chrono::Utc::now().timestamp();
            let sign = self.generate_sign(timestamp, secret)?;

            let mut signed_body = body.clone();
            signed_body["timestamp"] = json!(timestamp);
            signed_body["sign"] = json!(sign);

            request = self.client.post(webhook_url).json(&signed_body);
        }

        let response = request.send().await.context("发送飞书消息失败")?;

        if response.status().is_success() {
            // 检查飞书API响应
            let response_text = response.text().await.unwrap_or_default();
            if let Ok(response_json) = serde_json::from_str::<Value>(&response_text) {
                if let Some(code) = response_json.get("code").and_then(|c| c.as_i64()) {
                    if code != 0 {
                        let msg = response_json
                            .get("msg")
                            .and_then(|m| m.as_str())
                            .unwrap_or("未知错误");
                        return Err(anyhow::anyhow!("飞书API返回错误: {} - {}", code, msg));
                    }
                }
            }
            Ok(())
        } else {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!("HTTP请求失败: {} - {}", status, text))
        }
    }

    /// 生成飞书签名
    fn generate_sign(&self, timestamp: i64, secret: &str) -> Result<String> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        let string_to_sign = format!("{timestamp}\n{secret}");
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|e| anyhow::anyhow!("创建HMAC失败: {}", e))?;

        mac.update(string_to_sign.as_bytes());
        let result = mac.finalize();

        use base64::{engine::general_purpose, Engine as _};
        Ok(general_purpose::STANDARD.encode(result.into_bytes()))
    }

    /// 获取webhook URL
    fn get_webhook_url(&self, service: &ServiceConfig) -> Option<String> {
        service
            .feishu_webhook_url
            .clone()
            .or_else(|| self.config.webhook_url.clone())
    }

    /// 创建模板上下文
    fn create_template_context(
        &self,
        service: &ServiceConfig,
        result: &HealthResult,
    ) -> TemplateContext {
        let mut custom_fields = HashMap::new();

        // 添加健康状态
        let is_healthy = result.status.is_healthy();
        custom_fields.insert(
            "health_status".to_string(),
            serde_json::Value::Bool(is_healthy),
        );

        // 添加健康状态文本
        custom_fields.insert(
            "health_status_text".to_string(),
            serde_json::Value::String(if is_healthy { "正常" } else { "异常" }.to_string()),
        );

        // 添加服务描述
        if let Some(ref description) = service.description {
            custom_fields.insert(
                "service_description".to_string(),
                serde_json::Value::String(description.clone()),
            );
        }

        // 添加HTTP方法
        custom_fields.insert(
            "http_method".to_string(),
            serde_json::Value::String(service.method.clone()),
        );

        // 添加失败阈值
        custom_fields.insert(
            "failure_threshold".to_string(),
            serde_json::Value::Number(service.failure_threshold.into()),
        );

        TemplateContext {
            service_name: service.name.clone(),
            service_url: service.url.clone(),
            status_code: result.status_code,
            response_time: result.response_time_ms(),
            timestamp: result.timestamp.format("%Y-%m-%d %H:%M:%S").to_string(),
            error_message: result.error_message.clone(),
            custom_fields,
        }
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

        // 创建模板上下文
        let context = self.create_template_context(service, result);

        // 选择合适的模板和消息类型
        let (template, message_type, title_prefix) = if result.status.is_healthy() {
            (
                &self.recovery_template,
                MessageType::Recovery,
                "✅ 服务恢复",
            )
        } else {
            (&self.alert_template, MessageType::Alert, "🚨 服务告警")
        };

        // 渲染模板
        let content = template.render(&context).context("渲染消息模板失败")?;

        let message = NotificationMessage {
            title: format!("{} - {}", title_prefix, service.name),
            content,
            service_name: service.name.clone(),
            service_url: service.url.clone(),
            message_type,
        };

        let body = self.build_message_body(&message, &self.config.default_format);
        self.send_to_webhook(&webhook_url, &body).await
    }

    async fn send_message(&self, message: &NotificationMessage) -> Result<()> {
        // 对于自定义消息，需要有默认的webhook URL
        let webhook_url = match &self.config.webhook_url {
            Some(url) => url,
            None => {
                return Err(anyhow::anyhow!("未配置默认飞书webhook URL"));
            }
        };

        let body = self.build_message_body(message, &self.config.default_format);
        self.send_to_webhook(webhook_url, &body).await
    }

    async fn test_connection(&self) -> Result<()> {
        let webhook_url = match &self.config.webhook_url {
            Some(url) => url,
            None => {
                return Err(anyhow::anyhow!("未配置飞书webhook URL"));
            }
        };

        let test_message = NotificationMessage {
            title: "🔔 连接测试".to_string(),
            content: "这是一条测试消息，用于验证飞书webhook连接是否正常。\n\n如果您收到此消息，说明配置正确！".to_string(),
            service_name: "test".to_string(),
            service_url: "test".to_string(),
            message_type: MessageType::Info,
        };

        let body = self.build_message_body(&test_message, &self.config.default_format);
        self.send_to_webhook(webhook_url, &body).await
    }
}
