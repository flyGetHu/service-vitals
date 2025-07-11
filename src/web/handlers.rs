//! Web 路由处理函数
//!
//! 实现 Web 服务器的路由处理逻辑

use super::{SharedWebState, WebServiceStatus};
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
}

/// API 状态响应结构
#[derive(serde::Serialize)]
struct ApiStatusResponse {
    services: Vec<ApiServiceStatus>,
    last_updated: String,
    total_services: usize,
    online_services: usize,
    offline_services: usize,
}

/// API 服务状态结构
#[derive(serde::Serialize)]
struct ApiServiceStatus {
    name: String,
    url: String,
    status: String,
    response_time_ms: Option<u64>,
    last_check: Option<String>,
    history: Vec<ApiStatusHistory>,
}

/// API 状态历史结构
#[derive(serde::Serialize)]
struct ApiStatusHistory {
    timestamp: String,
    status: String,
    response_time_ms: Option<u64>,
}

/// 仪表板页面处理函数
pub async fn dashboard(State(state): State<SharedWebState>) -> impl IntoResponse {
    let state_guard = state.read().await;
    let services: Vec<WebServiceStatus> = state_guard.values().cloned().collect();
    drop(state_guard);

    // 计算统计数据
    let online_count = services.iter().filter(|s| s.status == "Online").count();
    let offline_count = services.iter().filter(|s| s.status == "Offline").count();
    let unknown_count = services.iter().filter(|s| s.status == "Unknown").count();

    let template = DashboardTemplate {
        services,
        last_updated: chrono::Utc::now()
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string(),
        online_count,
        offline_count,
        unknown_count,
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
pub async fn api_status(State(state): State<SharedWebState>) -> impl IntoResponse {
    let state_guard = state.read().await;
    let services_map = state_guard.clone();
    drop(state_guard);

    let mut services = Vec::new();
    let mut online_count = 0;
    let mut offline_count = 0;

    for service in services_map.values() {
        match service.status.as_str() {
            "Online" => online_count += 1,
            "Offline" => offline_count += 1,
            _ => {}
        }

        services.push(ApiServiceStatus {
            name: service.name.clone(),
            url: service.url.clone(),
            status: service.status.clone(),
            response_time_ms: service.response_time_ms,
            last_check: service.last_check.map(|dt| dt.to_rfc3339()),
            history: service
                .history
                .iter()
                .map(|h| ApiStatusHistory {
                    timestamp: h.timestamp.to_rfc3339(),
                    status: h.status.clone(),
                    response_time_ms: h.response_time_ms,
                })
                .collect(),
        });
    }

    let response = ApiStatusResponse {
        total_services: services.len(),
        online_services: online_count,
        offline_services: offline_count,
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
    use crate::web::{StatusHistory, WebServiceStatus};
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
                history: vec![StatusHistory {
                    timestamp: chrono::Utc::now(),
                    status: "Online".to_string(),
                    response_time_ms: Some(150),
                }],
            },
        );

        let state = Arc::new(RwLock::new(test_data));

        // 调用处理函数
        let response = api_status(State(state)).await;

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
                history: vec![],
            },
        );

        let state = Arc::new(RwLock::new(test_data));

        // 调用处理函数
        let response = dashboard(State(state)).await;

        // 验证响应状态
        assert!(response.into_response().status().is_success());
    }
}
