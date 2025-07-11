//! 通知处理基准测试
//!
//! 测试通知模板渲染和消息处理的性能

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use service_vitals::health::{HealthResult, HealthStatus};
use service_vitals::notification::template::{HandlebarsTemplate, MessageTemplate, TemplateContext};
use std::time::Duration;

/// 通知处理基准测试
fn notification_benchmark(c: &mut Criterion) {
    c.bench_function("template_rendering", |b| {
        let template = HandlebarsTemplate::new(
            "服务 {{service_name}} 状态异常\nURL: {{service_url}}\n响应时间: {{response_time}}ms".to_string()
        ).unwrap();
        
        let context = TemplateContext {
            service_name: "test-service".to_string(),
            service_url: "https://httpbin.org/status/500".to_string(),
            status_code: Some(500),
            response_time: 250,
            timestamp: "2023-01-01T00:00:00Z".to_string(),
            error_message: Some("Connection timeout".to_string()),
            custom_fields: std::collections::HashMap::new(),
        };
        
        b.iter(|| {
            let message = template.render(&context).unwrap();
            black_box(message)
        });
    });

    c.bench_function("template_rendering_complex", |b| {
        let template = HandlebarsTemplate::new(
            r#"🚨 服务告警

**服务名称**: {{service_name}}
**服务地址**: {{service_url}}
**HTTP状态码**: {{status_code}}
**响应时间**: {{response_time}}ms
**检测时间**: {{timestamp}}

{{#if error_message}}
**错误信息**: {{error_message}}
{{/if}}

请及时检查服务状态！"#.to_string()
        ).unwrap();
        
        let context = TemplateContext {
            service_name: "production-api".to_string(),
            service_url: "https://api.example.com/health".to_string(),
            status_code: Some(503),
            response_time: 5000,
            timestamp: "2023-01-01T00:00:00Z".to_string(),
            error_message: Some("Service unavailable".to_string()),
            custom_fields: std::collections::HashMap::new(),
        };
        
        b.iter(|| {
            let message = template.render(&context).unwrap();
            black_box(message)
        });
    });

    c.bench_function("template_creation", |b| {
        b.iter(|| {
            let template = HandlebarsTemplate::new(
                "服务 {{service_name}} 状态: {{status}}".to_string()
            ).unwrap();
            black_box(template)
        });
    });

    c.bench_function("health_result_to_notification_data", |b| {
        let result = HealthResult::new(
            "test-service".to_string(),
            "https://httpbin.org/status/500".to_string(),
            HealthStatus::Down,
            "GET".to_string(),
        )
        .with_status_code(500)
        .with_response_time(Duration::from_millis(250))
        .with_error("Connection timeout".to_string())
        .with_consecutive_failures(2);
        
        b.iter(|| {
            let data = serde_json::to_value(&result).unwrap();
            black_box(data)
        });
    });
}

criterion_group!(benches, notification_benchmark);
criterion_main!(benches);