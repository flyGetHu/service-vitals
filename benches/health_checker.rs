//! 健康检测器基准测试
//!
//! 测试健康检测器的性能和并发处理能力

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use service_vitals::health::{HttpHealthChecker, HealthResult, HealthStatus};
use service_vitals::config::ServiceConfig;
use std::time::Duration;

/// 健康检测器基准测试
fn health_checker_benchmark(c: &mut Criterion) {
    c.bench_function("health_result_creation", |b| {
        b.iter(|| {
            let result = HealthResult::new(
                "test-service".to_string(),
                "https://httpbin.org/status/200".to_string(),
                HealthStatus::Up,
                "GET".to_string(),
            )
            .with_status_code(200)
            .with_response_time(Duration::from_millis(150));
            black_box(result)
        });
    });

    c.bench_function("service_config_creation", |b| {
        b.iter(|| {
            let config = ServiceConfig {
                name: "test-service".to_string(),
                url: "https://httpbin.org/status/200".to_string(),
                method: "GET".to_string(),
                expected_status_codes: vec![200],
                feishu_webhook_url: None,
                failure_threshold: 1,
                check_interval_seconds: None,
                enabled: true,
                description: None,
                headers: std::collections::HashMap::new(),
                body: None,
                alert_cooldown_secs: None,
            };
            black_box(config)
        });
    });
}

/// 健康结果处理基准测试
fn health_result_processing_benchmark(c: &mut Criterion) {
    c.bench_function("health_result_serialization", |b| {
        b.iter(|| {
            let result = HealthResult::new(
                "test-service".to_string(),
                "https://httpbin.org/status/200".to_string(),
                HealthStatus::Up,
                "GET".to_string(),
            )
            .with_status_code(200)
            .with_response_time(Duration::from_millis(150));
            
            let json = serde_json::to_string(&result).unwrap();
            black_box(json)
        });
    });

    c.bench_function("health_result_deserialization", |b| {
        let json = r#"{
            "id": "123e4567-e89b-12d3-a456-426614174000",
            "service_name": "test-service",
            "service_url": "https://httpbin.org/status/200",
            "timestamp": "2023-01-01T00:00:00Z",
            "status": "up",
            "status_code": 200,
            "response_time": 150,
            "error_message": null,
            "consecutive_failures": 0,
            "method": "GET",
            "response_size": null,
            "metadata": {}
        }"#;
        
        b.iter(|| {
            let result: HealthResult = serde_json::from_str(json).unwrap();
            black_box(result)
        });
    });
}

criterion_group!(
    benches,
    health_checker_benchmark,
    health_result_processing_benchmark
);
criterion_main!(benches);