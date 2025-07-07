//! 消息模板模块
//!
//! 提供消息模板渲染功能

use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

/// 模板上下文数据
#[derive(Debug, Clone)]
pub struct TemplateContext {
    /// 服务名称
    pub service_name: String,
    /// 服务URL
    pub service_url: String,
    /// HTTP状态码
    pub status_code: Option<u16>,
    /// 响应时间（毫秒）
    pub response_time: u64,
    /// 时间戳
    pub timestamp: String,
    /// 错误信息
    pub error_message: Option<String>,
    /// 自定义字段
    pub custom_fields: HashMap<String, Value>,
}

/// 消息模板trait
pub trait MessageTemplate: Send + Sync {
    /// 渲染模板
    ///
    /// # 参数
    /// * `context` - 模板上下文
    ///
    /// # 返回
    /// * `Result<String>` - 渲染后的消息
    fn render(&self, context: &TemplateContext) -> Result<String>;

    /// 验证模板语法
    ///
    /// # 返回
    /// * `Result<()>` - 验证结果
    fn validate(&self) -> Result<()>;
}

/// 简单的字符串替换模板
pub struct SimpleTemplate {
    /// 模板字符串
    template: String,
}

impl SimpleTemplate {
    /// 创建新的简单模板
    ///
    /// # 参数
    /// * `template` - 模板字符串
    ///
    /// # 返回
    /// * `Self` - 模板实例
    pub fn new(template: String) -> Self {
        Self { template }
    }

    /// 执行字符串替换
    fn replace_variables(&self, template: &str, context: &TemplateContext) -> String {
        let mut result = template.to_string();

        // 替换基础变量
        result = result.replace("{{service_name}}", &context.service_name);
        result = result.replace("{{service_url}}", &context.service_url);
        result = result.replace("{{response_time}}", &context.response_time.to_string());
        result = result.replace("{{timestamp}}", &context.timestamp);

        // 替换可选变量
        if let Some(status_code) = context.status_code {
            result = result.replace("{{status_code}}", &status_code.to_string());
        } else {
            result = result.replace("{{status_code}}", "N/A");
        }

        if let Some(ref error_message) = context.error_message {
            result = result.replace("{{error_message}}", error_message);
        } else {
            result = result.replace("{{error_message}}", "");
        }

        // 替换自定义字段
        for (key, value) in &context.custom_fields {
            let placeholder = format!("{{{{{}}}}}", key);
            let value_str = match value {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                _ => value.to_string(),
            };
            result = result.replace(&placeholder, &value_str);
        }

        result
    }
}

impl MessageTemplate for SimpleTemplate {
    fn render(&self, context: &TemplateContext) -> Result<String> {
        Ok(self.replace_variables(&self.template, context))
    }

    fn validate(&self) -> Result<()> {
        // 简单模板总是有效的
        Ok(())
    }
}

/// Handlebars模板（占位符实现）
pub struct HandlebarsTemplate {
    /// 模板字符串
    template: String,
}

impl HandlebarsTemplate {
    /// 创建新的Handlebars模板
    ///
    /// # 参数
    /// * `template` - 模板字符串
    ///
    /// # 返回
    /// * `Result<Self>` - 模板实例
    pub fn new(template: String) -> Result<Self> {
        // TODO: 在第二阶段实现真正的Handlebars支持
        Ok(Self { template })
    }
}

impl MessageTemplate for HandlebarsTemplate {
    fn render(&self, context: &TemplateContext) -> Result<String> {
        // 暂时使用简单的字符串替换
        let simple_template = SimpleTemplate::new(self.template.clone());
        simple_template.render(context)
    }

    fn validate(&self) -> Result<()> {
        // TODO: 在第二阶段实现真正的Handlebars语法验证
        Ok(())
    }
}

/// 默认的告警消息模板
pub fn default_alert_template() -> String {
    r#"🚨 **服务告警**
- **服务名称**: {{service_name}}
- **服务URL**: {{service_url}}
- **状态码**: {{status_code}}
- **响应时间**: {{response_time}}ms
- **检测时间**: {{timestamp}}
{{#if error_message}}
- **错误信息**: {{error_message}}
{{/if}}"#
        .to_string()
}

/// 默认的恢复消息模板
pub fn default_recovery_template() -> String {
    r#"✅ **服务恢复**
- **服务名称**: {{service_name}}
- **服务URL**: {{service_url}}
- **状态码**: {{status_code}}
- **响应时间**: {{response_time}}ms
- **恢复时间**: {{timestamp}}"#
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_template_render() {
        let template =
            SimpleTemplate::new("Service {{service_name}} is {{status_code}}".to_string());
        let context = TemplateContext {
            service_name: "test-service".to_string(),
            service_url: "http://example.com".to_string(),
            status_code: Some(200),
            response_time: 100,
            timestamp: "2023-01-01 12:00:00".to_string(),
            error_message: None,
            custom_fields: HashMap::new(),
        };

        let result = template.render(&context).unwrap();
        assert_eq!(result, "Service test-service is 200");
    }

    #[test]
    fn test_template_with_missing_status_code() {
        let template = SimpleTemplate::new("Status: {{status_code}}".to_string());
        let context = TemplateContext {
            service_name: "test".to_string(),
            service_url: "http://example.com".to_string(),
            status_code: None,
            response_time: 100,
            timestamp: "2023-01-01 12:00:00".to_string(),
            error_message: None,
            custom_fields: HashMap::new(),
        };

        let result = template.render(&context).unwrap();
        assert_eq!(result, "Status: N/A");
    }
}
