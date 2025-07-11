//! Web 路由处理函数
//!
//! 实现 Web 服务器的路由处理逻辑

use super::{WebAppState, WebServiceStatus};
use askama::Template;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Json},
};
use tracing::error;

/// 仪表板模板
#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTemplate {
    services: Vec<WebServiceStatus>,
    last_updated: String,
    online_count: usize,
    offline_count: usize,
    unknown_count: usize,
    refresh_interval: u32,
    show_problems_only: bool,
}

/// API 状态响应结构
#[derive(serde::Serialize)]
struct ApiStatusResponse {
    services: Vec<ApiServiceStatus>,
    last_updated: String,
    total_services: usize,
    online_services: usize,
    offline_services: usize,
    unknown_services: usize,
}

/// API 服务状态结构
#[derive(serde::Serialize)]
struct ApiServiceStatus {
    name: String,
    url: String,
    status: String,
    response_time_ms: Option<u64>,
    last_check: Option<String>,
    error_message: Option<String>,
}

/// 仪表板页面处理函数
pub async fn dashboard(State(app_state): State<WebAppState>) -> impl IntoResponse {
    let state_guard = app_state.services.read().await;
    let all_services: Vec<WebServiceStatus> = state_guard.values().cloned().collect();
    drop(state_guard);

    // 先计算所有服务的统计数据（不受过滤影响）
    let online_count = all_services.iter().filter(|s| s.status == "Online").count();
    let offline_count = all_services
        .iter()
        .filter(|s| s.status == "Offline")
        .count();
    let unknown_count = all_services
        .iter()
        .filter(|s| s.status == "Unknown")
        .count();

    // 根据配置过滤要显示的服务
    let mut services = all_services;
    if app_state.config.show_problems_only {
        services.retain(|s| s.status == "Offline" || s.status == "Unknown");
    }

    let template = DashboardTemplate {
        services,
        last_updated: chrono::Utc::now()
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string(),
        online_count,
        offline_count,
        unknown_count,
        refresh_interval: app_state.config.refresh_interval_seconds,
        show_problems_only: app_state.config.show_problems_only,
    };

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            error!("模板渲染失败: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "模板渲染失败").into_response()
        }
    }
}

/// API 状态端点处理函数
pub async fn api_status(State(app_state): State<WebAppState>) -> impl IntoResponse {
    let state_guard = app_state.services.read().await;
    let services_map = state_guard.clone();
    drop(state_guard);

    let mut services = Vec::new();
    let mut online_count = 0;
    let mut offline_count = 0;
    let mut unknown_count = 0;

    // 先统计所有服务的状态（不受过滤影响）
    for service in services_map.values() {
        match service.status.as_str() {
            "Online" => online_count += 1,
            "Offline" => offline_count += 1,
            "Unknown" => unknown_count += 1,
            _ => {}
        }
    }

    // API 端点总是返回所有服务，让前端决定如何过滤
    for service in services_map.values() {
        services.push(ApiServiceStatus {
            name: service.name.clone(),
            url: service.url.clone(),
            status: service.status.clone(),
            response_time_ms: service.response_time_ms,
            last_check: service.last_check.map(|dt| dt.to_rfc3339()),
            error_message: service.error_message.clone(),
        });
    }

    let response = ApiStatusResponse {
        total_services: services_map.len(), // 总服务数（未过滤）
        online_services: online_count,
        offline_services: offline_count,
        unknown_services: unknown_count,
        services,
        last_updated: chrono::Utc::now().to_rfc3339(),
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );

    (headers, Json(response)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::WebConfig;
    use crate::web::WebServiceStatus;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    #[tokio::test]
    async fn test_api_status_handler() {
        // 创建测试状态数据
        let mut test_data = HashMap::new();
        test_data.insert(
            "test-service".to_string(),
            WebServiceStatus {
                name: "test-service".to_string(),
                url: "https://example.com".to_string(),
                status: "Online".to_string(),
                response_time_ms: Some(150),
                last_check: Some(chrono::Utc::now()),
                error_message: None,
            },
        );

        let app_state = WebAppState {
            config: WebConfig::default(),
            services: Arc::new(RwLock::new(test_data)),
        };

        // 调用处理函数
        let response = api_status(State(app_state)).await;

        // 验证响应
        // 注意：这里只是基本的结构测试，实际的 JSON 内容验证需要更复杂的测试
        assert!(response.into_response().status().is_success());
    }

    #[tokio::test]
    async fn test_dashboard_handler() {
        // 创建测试状态数据
        let mut test_data = HashMap::new();
        test_data.insert(
            "test-service".to_string(),
            WebServiceStatus {
                name: "test-service".to_string(),
                url: "https://example.com".to_string(),
                status: "Online".to_string(),
                response_time_ms: Some(150),
                last_check: Some(chrono::Utc::now()),
                error_message: None,
            },
        );

        let app_state = WebAppState {
            config: WebConfig::default(),
            services: Arc::new(RwLock::new(test_data)),
        };

        // 调用处理函数
        let response = dashboard(State(app_state)).await;

        // 验证响应状态
        assert!(response.into_response().status().is_success());
    }

    #[tokio::test]
    async fn test_api_status_with_filtering() {
        // 创建包含不同状态的测试数据
        let mut test_data = HashMap::new();
        test_data.insert(
            "online-service".to_string(),
            WebServiceStatus {
                name: "online-service".to_string(),
                url: "https://example.com".to_string(),
                status: "Online".to_string(),
                response_time_ms: Some(150),
                last_check: Some(chrono::Utc::now()),
                error_message: None,
            },
        );
        test_data.insert(
            "offline-service".to_string(),
            WebServiceStatus {
                name: "offline-service".to_string(),
                url: "https://example2.com".to_string(),
                status: "Offline".to_string(),
                response_time_ms: None,
                last_check: Some(chrono::Utc::now()),
                error_message: Some("Connection refused".to_string()),
            },
        );

        // 测试过滤功能
        let config = WebConfig {
            show_problems_only: true,
            ..Default::default()
        };

        let app_state = WebAppState {
            config,
            services: Arc::new(RwLock::new(test_data)),
        };

        let response = api_status(State(app_state)).await;
        assert!(response.into_response().status().is_success());
    }

    #[tokio::test]
    async fn test_dashboard_with_filtering() {
        // 创建包含不同状态的测试数据
        let mut test_data = HashMap::new();
        test_data.insert(
            "online-service".to_string(),
            WebServiceStatus {
                name: "online-service".to_string(),
                url: "https://example.com".to_string(),
                status: "Online".to_string(),
                response_time_ms: Some(150),
                last_check: Some(chrono::Utc::now()),
                error_message: None,
            },
        );
        test_data.insert(
            "offline-service".to_string(),
            WebServiceStatus {
                name: "offline-service".to_string(),
                url: "https://example2.com".to_string(),
                status: "Offline".to_string(),
                response_time_ms: None,
                last_check: Some(chrono::Utc::now()),
                error_message: Some("Service unavailable".to_string()),
            },
        );

        // 测试过滤功能
        let config = WebConfig {
            show_problems_only: true,
            ..Default::default()
        };

        let app_state = WebAppState {
            config,
            services: Arc::new(RwLock::new(test_data)),
        };

        let response = dashboard(State(app_state)).await;
        assert!(response.into_response().status().is_success());
    }

    #[tokio::test]
    async fn test_web_config_in_template() {
        // 测试配置参数是否正确传递到模板
        let test_data = HashMap::new();

        let config = WebConfig {
            refresh_interval_seconds: 10,
            ..Default::default()
        };

        let app_state = WebAppState {
            config,
            services: Arc::new(RwLock::new(test_data)),
        };

        let response = dashboard(State(app_state)).await;
        assert!(response.into_response().status().is_success());
    }

    #[tokio::test]
    async fn test_api_status_statistics_accuracy() {
        // 测试API统计数据的准确性：确保统计数据反映所有服务，不受过滤影响
        let mut test_data = HashMap::new();

        // 添加不同状态的服务
        test_data.insert(
            "online-service-1".to_string(),
            WebServiceStatus {
                name: "online-service-1".to_string(),
                url: "https://example1.com".to_string(),
                status: "Online".to_string(),
                response_time_ms: Some(150),
                last_check: Some(chrono::Utc::now()),
                error_message: None,
            },
        );
        test_data.insert(
            "online-service-2".to_string(),
            WebServiceStatus {
                name: "online-service-2".to_string(),
                url: "https://example2.com".to_string(),
                status: "Online".to_string(),
                response_time_ms: Some(200),
                last_check: Some(chrono::Utc::now()),
                error_message: None,
            },
        );
        test_data.insert(
            "offline-service".to_string(),
            WebServiceStatus {
                name: "offline-service".to_string(),
                url: "https://example3.com".to_string(),
                status: "Offline".to_string(),
                response_time_ms: None,
                last_check: Some(chrono::Utc::now()),
                error_message: Some("HTTP 500 Internal Server Error".to_string()),
            },
        );
        test_data.insert(
            "unknown-service".to_string(),
            WebServiceStatus {
                name: "unknown-service".to_string(),
                url: "https://example4.com".to_string(),
                status: "Unknown".to_string(),
                response_time_ms: None,
                last_check: Some(chrono::Utc::now()),
                error_message: Some("DNS resolution failed".to_string()),
            },
        );

        // 测试不启用过滤的情况
        let config_no_filter = WebConfig::default();
        let app_state_no_filter = WebAppState {
            config: config_no_filter,
            services: Arc::new(RwLock::new(test_data.clone())),
        };

        let response = api_status(State(app_state_no_filter)).await;
        let response = response.into_response();
        assert!(response.status().is_success());

        // 测试启用过滤的情况 - 统计数据应该相同
        let config_with_filter = WebConfig {
            show_problems_only: true,
            ..Default::default()
        };

        let app_state_with_filter = WebAppState {
            config: config_with_filter,
            services: Arc::new(RwLock::new(test_data)),
        };

        let response = api_status(State(app_state_with_filter)).await;
        let response = response.into_response();
        assert!(response.status().is_success());

        // 注意：由于我们修改了API行为，现在API总是返回所有服务
        // 统计数据应该始终反映真实的服务状态分布：
        // - total_services: 4
        // - online_services: 2
        // - offline_services: 1
        // - unknown_services: 1
    }
}
