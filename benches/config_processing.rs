//! 配置处理基准测试
//!
//! 测试配置解析、验证和序列化的性能

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use service_vitals::config::types::WebConfig;
use service_vitals::config::{Config, GlobalConfig, ServiceConfig};
use std::collections::HashMap;

/// 配置处理基准测试
fn config_processing_benchmark(c: &mut Criterion) {
    c.bench_function("config_creation", |b| {
        b.iter(|| {
            let global_config = GlobalConfig {
                default_feishu_webhook_url: Some(
                    "https://open.feishu.cn/open-apis/bot/v2/hook/xxx".to_string(),
                ),
                message_template: Some("服务 {{service_name}} 状态异常".to_string()),
                check_interval_seconds: 60,
                log_level: "info".to_string(),
                request_timeout_seconds: 10,
                max_concurrent_checks: 50,
                retry_attempts: 3,
                retry_delay_seconds: 5,
                headers: HashMap::new(),
                web: Some(WebConfig {
                    enabled: true,
                    port: 8080,
                    bind_address: "127.0.0.1".to_string(),
                    show_problems_only: false,
                    layout_type: "cards".to_string(),
                    refresh_interval_seconds: 30,
                }),
            };

            let service_config = ServiceConfig {
                name: "test-service".to_string(),
                url: "https://httpbin.org/status/200".to_string(),
                method: "GET".to_string(),
                expected_status_codes: vec![200],
                feishu_webhook_url: None,
                failure_threshold: 1,
                check_interval_seconds: None,
                enabled: true,
                description: Some("测试服务".to_string()),
                headers: HashMap::new(),
                body: None,
                alert_cooldown_secs: None,
            };

            let config = Config {
                global: global_config,
                services: vec![service_config],
            };

            black_box(config)
        });
    });

    c.bench_function("config_serialization", |b| {
        let config = create_test_config();

        b.iter(|| {
            let toml = toml::to_string(&config).unwrap();
            black_box(toml)
        });
    });

    c.bench_function("config_deserialization", |b| {
        let toml_str = r#"
[global]
default_feishu_webhook_url = "https://open.feishu.cn/open-apis/bot/v2/hook/xxx"
message_template = "服务 {{service_name}} 状态异常"
check_interval_seconds = 60
log_level = "info"
request_timeout_seconds = 10
max_concurrent_checks = 50
retry_attempts = 3
retry_delay_seconds = 5

[global.web]
enabled = true
port = 8080
bind_address = "127.0.0.1"
show_problems_only = false
layout_type = "cards"
refresh_interval_seconds = 30

[[services]]
name = "test-service"
url = "https://httpbin.org/status/200"
method = "GET"
expected_status_codes = [200]
failure_threshold = 1
enabled = true
description = "测试服务"
"#;

        b.iter(|| {
            let config: Config = toml::from_str(toml_str).unwrap();
            black_box(config)
        });
    });

    c.bench_function("config_validation", |b| {
        let config = create_test_config();

        b.iter(|| {
            let result = service_vitals::config::validate_config(&config);
            black_box(result)
        });
    });
}

/// 创建测试配置
fn create_test_config() -> Config {
    let global_config = GlobalConfig {
        default_feishu_webhook_url: Some(
            "https://open.feishu.cn/open-apis/bot/v2/hook/xxx".to_string(),
        ),
        message_template: Some("服务 {{service_name}} 状态异常".to_string()),
        check_interval_seconds: 60,
        log_level: "info".to_string(),
        request_timeout_seconds: 10,
        max_concurrent_checks: 50,
        retry_attempts: 3,
        retry_delay_seconds: 5,
        headers: HashMap::new(),
        web: Some(WebConfig {
            enabled: true,
            port: 8080,
            bind_address: "127.0.0.1".to_string(),
            show_problems_only: false,
            layout_type: "cards".to_string(),
            refresh_interval_seconds: 30,
        }),
    };

    let service_config = ServiceConfig {
        name: "test-service".to_string(),
        url: "https://httpbin.org/status/200".to_string(),
        method: "GET".to_string(),
        expected_status_codes: vec![200],
        feishu_webhook_url: None,
        failure_threshold: 1,
        check_interval_seconds: None,
        enabled: true,
        description: Some("测试服务".to_string()),
        headers: HashMap::new(),
        body: None,
        alert_cooldown_secs: None,
    };

    Config {
        global: global_config,
        services: vec![service_config],
    }
}

criterion_group!(benches, config_processing_benchmark);
criterion_main!(benches);
