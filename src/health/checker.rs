//! HTTP健康检测器实现
//!
//! 提供HTTP健康检测功能，支持多种HTTP方法和超时处理

use crate::common::error::{HealthCheckError, Result};
use crate::config::ServiceConfig;
use crate::health::result::{HealthResult, HealthStatus};
use async_trait::async_trait;
use reqwest::{Client, Method, Response};
use std::str::FromStr;
use std::time::{Duration, Instant};
use tokio::time::timeout;

/// 健康检测器trait，定义检测接口
#[async_trait]
pub trait HealthChecker: Send + Sync {
    /// 执行健康检测
    ///
    /// # 参数
    /// * `service` - 服务配置
    ///
    /// # 返回
    /// * `Result<HealthResult>` - 检测结果
    async fn check(&self, service: &ServiceConfig) -> Result<HealthResult>;

    /// 带超时的健康检测
    ///
    /// # 参数
    /// * `service` - 服务配置
    /// * `timeout_duration` - 超时时间
    ///
    /// # 返回
    /// * `Result<HealthResult>` - 检测结果
    async fn check_with_timeout(
        &self,
        service: &ServiceConfig,
        timeout_duration: Duration,
    ) -> Result<HealthResult>;

    /// 批量健康检测
    ///
    /// # 参数
    /// * `services` - 服务配置列表
    ///
    /// # 返回
    /// * `Vec<Result<HealthResult>>` - 检测结果列表
    async fn check_batch(&self, services: &[ServiceConfig]) -> Vec<Result<HealthResult>>;
}

/// HTTP健康检测器实现
pub struct HttpHealthChecker {
    /// HTTP客户端
    client: Client,
    /// 默认超时时间
    default_timeout: Duration,
    /// 重试次数
    retry_attempts: u32,
    /// 重试间隔
    retry_delay: Duration,
}

impl HttpHealthChecker {
    /// 创建新的HTTP健康检测器
    ///
    /// # 参数
    /// * `timeout` - 默认超时时间
    /// * `retry_attempts` - 重试次数
    /// * `retry_delay` - 重试间隔
    ///
    /// # 返回
    /// * `Result<Self>` - 检测器实例
    pub fn new(timeout: Duration, retry_attempts: u32, retry_delay: Duration) -> Result<Self> {
        let client = Client::builder()
            .timeout(timeout)
            .user_agent(format!("{}/{}", crate::APP_NAME, crate::VERSION))
            .build()
            .map_err(HealthCheckError::RequestError)?;

        Ok(Self {
            client,
            default_timeout: timeout,
            retry_attempts,
            retry_delay,
        })
    }

    /// 构建HTTP请求
    ///
    /// # 参数
    /// * `service` - 服务配置
    ///
    /// # 返回
    /// * `Result<reqwest::RequestBuilder>` - 请求构建器
    fn build_request(&self, service: &ServiceConfig) -> Result<reqwest::RequestBuilder> {
        // 解析HTTP方法
        let method = Method::from_str(&service.method.to_uppercase()).map_err(|_| {
            HealthCheckError::ConnectionError {
                url: format!("无效的HTTP方法: {}", service.method),
            }
        })?;

        let mut request = self.client.request(method, &service.url);

        // 添加请求头
        for (key, value) in &service.headers {
            request = request.header(key, value);
        }

        // 添加请求体（如果有）
        if let Some(body) = &service.body {
            request = request.json(body);
        }

        Ok(request)
    }

    /// 验证响应状态码
    ///
    /// # 参数
    /// * `status_code` - 实际状态码
    /// * `expected_codes` - 期望的状态码列表
    ///
    /// # 返回
    /// * `bool` - 是否匹配
    fn validate_status_code(&self, status_code: u16, expected_codes: &[u16]) -> bool {
        expected_codes.contains(&status_code)
    }

    /// 执行单次HTTP请求
    ///
    /// # 参数
    /// * `service` - 服务配置
    /// * `timeout_duration` - 超时时间
    ///
    /// # 返回
    /// * `Result<HealthResult>` - 检测结果
    async fn perform_request(
        &self,
        service: &ServiceConfig,
        timeout_duration: Duration,
    ) -> Result<HealthResult> {
        let start_time = Instant::now();

        // 构建请求
        let request = self.build_request(service)?;

        // 执行请求（带超时）
        let response_result = timeout(timeout_duration, request.send()).await;

        let response_time = start_time.elapsed();

        match response_result {
            Ok(Ok(response)) => {
                self.process_successful_response(service, response, response_time)
                    .await
            }
            Ok(Err(e)) => {
                Ok(self.create_error_result(service, response_time, self.format_request_error(&e)))
            }
            Err(_) => Ok(self.create_timeout_result(service, response_time)),
        }
    }

    /// 处理成功的HTTP响应
    ///
    /// # 参数
    /// * `service` - 服务配置
    /// * `response` - HTTP响应
    /// * `response_time` - 响应时间
    ///
    /// # 返回
    /// * `Result<HealthResult>` - 检测结果
    async fn process_successful_response(
        &self,
        service: &ServiceConfig,
        response: Response,
        response_time: Duration,
    ) -> Result<HealthResult> {
        let status_code = response.status().as_u16();
        let is_healthy = self.validate_status_code(status_code, &service.expected_status_codes);

        // 获取响应体大小
        let response_size = response.content_length().map(|len| len as usize);

        let mut result = HealthResult::new(
            service.name.clone(),
            service.url.clone(),
            if is_healthy {
                HealthStatus::Up
            } else {
                HealthStatus::Down
            },
            service.method.clone(),
        )
        .with_status_code(status_code)
        .with_response_time(response_time);

        if let Some(size) = response_size {
            result = result.with_response_size(size);
        }

        if !is_healthy {
            result = result.with_error(format!(
                "HTTP {} {}",
                status_code,
                reqwest::StatusCode::from_u16(status_code)
                    .map(|s| s.canonical_reason().unwrap_or("Unknown"))
                    .unwrap_or("Unknown")
            ));
        }

        // 添加响应头信息到元数据
        let server_header = response
            .headers()
            .get("server")
            .and_then(|v| v.to_str().ok())
            .map(|s| serde_json::Value::String(s.to_string()));

        if let Some(server) = server_header {
            result = result.with_metadata("server".to_string(), server);
        }

        Ok(result)
    }

    /// 创建错误结果
    fn create_error_result(
        &self,
        service: &ServiceConfig,
        response_time: Duration,
        error_message: String,
    ) -> HealthResult {
        HealthResult::new(
            service.name.clone(),
            service.url.clone(),
            HealthStatus::Down,
            service.method.clone(),
        )
        .with_response_time(response_time)
        .with_error(error_message)
    }

    /// 创建超时结果
    fn create_timeout_result(
        &self,
        service: &ServiceConfig,
        response_time: Duration,
    ) -> HealthResult {
        HealthResult::new(
            service.name.clone(),
            service.url.clone(),
            HealthStatus::Down,
            service.method.clone(),
        )
        .with_response_time(response_time)
        .with_error("Request timeout".to_string())
    }

    /// 格式化请求错误信息，使其更加清晰易读
    fn format_request_error(&self, error: &reqwest::Error) -> String {
        if error.is_timeout() {
            "Request timeout".to_string()
        } else if error.is_connect() {
            "Connection refused".to_string()
        } else if error.is_request() {
            "Invalid request".to_string()
        } else if let Some(status) = error.status() {
            format!(
                "HTTP {} {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("Unknown")
            )
        } else if error.is_decode() {
            "Response decode error".to_string()
        } else {
            // 对于其他类型的错误，提供更友好的描述
            let error_str = error.to_string();
            if error_str.contains("dns") || error_str.contains("DNS") {
                "DNS resolution failed".to_string()
            } else if error_str.contains("certificate")
                || error_str.contains("tls")
                || error_str.contains("ssl")
            {
                "SSL/TLS certificate error".to_string()
            } else if error_str.contains("network") {
                "Network error".to_string()
            } else {
                format!("Request failed: {}", error_str)
            }
        }
    }
}

#[async_trait]
impl HealthChecker for HttpHealthChecker {
    async fn check(&self, service: &ServiceConfig) -> Result<HealthResult> {
        self.check_with_timeout(service, self.default_timeout).await
    }

    async fn check_with_timeout(
        &self,
        service: &ServiceConfig,
        timeout_duration: Duration,
    ) -> Result<HealthResult> {
        let mut last_error = None;

        // 重试逻辑
        for attempt in 0..=self.retry_attempts {
            match self.perform_request(service, timeout_duration).await {
                Ok(result) => {
                    if result.status.is_healthy() || attempt == self.retry_attempts {
                        return Ok(result);
                    }
                    last_error = result.error_message;
                }
                Err(e) => {
                    if attempt == self.retry_attempts {
                        return Err(e);
                    }
                    last_error = Some(e.to_string());
                }
            }

            // 等待重试间隔
            if attempt < self.retry_attempts {
                tokio::time::sleep(self.retry_delay).await;
            }
        }

        // 如果所有重试都失败，返回最后一个错误
        Ok(self.create_error_result(
            service,
            Duration::from_millis(0),
            last_error.unwrap_or_else(|| "所有重试都失败".to_string()),
        ))
    }

    async fn check_batch(&self, services: &[ServiceConfig]) -> Vec<Result<HealthResult>> {
        let futures = services.iter().map(|service| self.check(service));
        futures::future::join_all(futures).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ServiceConfig;
    use std::collections::HashMap;
    use std::time::Duration;

    fn create_test_service(url: &str, expected_codes: Vec<u16>) -> ServiceConfig {
        ServiceConfig {
            name: "Test Service".to_string(),
            url: url.to_string(),
            method: "GET".to_string(),
            expected_status_codes: expected_codes,
            feishu_webhook_url: None,
            failure_threshold: 1,
            check_interval_seconds: None,
            enabled: true,
            description: Some("Test service".to_string()),
            headers: HashMap::new(),
            body: None,
            alert_cooldown_secs: Some(60),
        }
    }

    #[tokio::test]
    async fn test_http_health_checker_creation() {
        let checker = HttpHealthChecker::new(Duration::from_secs(10), 3, Duration::from_secs(1));
        assert!(checker.is_ok());
    }

    #[tokio::test]
    async fn test_http_get_request() {
        let checker = HttpHealthChecker::new(
            Duration::from_secs(10),
            0, // 不重试
            Duration::from_secs(1),
        )
        .unwrap();

        // 使用httpbin.org进行测试
        let service = create_test_service("https://httpbin.org/status/200", vec![200]);
        let result = checker.check(&service).await;

        assert!(result.is_ok());
        let health_result = result.unwrap();
        assert_eq!(health_result.status, HealthStatus::Up);
        assert_eq!(health_result.status_code, Some(200));
        assert!(health_result.response_time.as_millis() > 0);
    }

    #[tokio::test]
    async fn test_status_code_validation() {
        let checker =
            HttpHealthChecker::new(Duration::from_secs(10), 0, Duration::from_secs(1)).unwrap();

        // 测试状态码不匹配的情况
        let service = create_test_service("https://httpbin.org/status/404", vec![200]);
        let result = checker.check(&service).await;

        assert!(result.is_ok());
        let health_result = result.unwrap();
        assert_eq!(health_result.status, HealthStatus::Down);
        assert_eq!(health_result.status_code, Some(404));
        assert!(health_result.error_message.is_some());
        assert!(health_result.error_message.unwrap().contains("HTTP 404"));
    }

    #[tokio::test]
    async fn test_response_time_measurement() {
        let checker =
            HttpHealthChecker::new(Duration::from_secs(10), 0, Duration::from_secs(1)).unwrap();

        let service = create_test_service("https://httpbin.org/delay/1", vec![200]);
        let result = checker.check(&service).await;

        assert!(result.is_ok());
        let health_result = result.unwrap();
        // 响应时间应该大于1秒（1000毫秒）
        assert!(health_result.response_time_ms() >= 1000);
    }

    #[tokio::test]
    async fn test_timeout_handling() {
        let checker = HttpHealthChecker::new(
            Duration::from_millis(100), // 很短的超时时间
            0,
            Duration::from_secs(1),
        )
        .unwrap();

        // 使用一个会延迟的端点
        let service = create_test_service("https://httpbin.org/delay/2", vec![200]);
        let result = checker.check(&service).await;

        assert!(result.is_ok());
        let health_result = result.unwrap();
        assert_eq!(health_result.status, HealthStatus::Down);
        assert!(health_result.error_message.is_some());
        // 可能是超时或者其他网络错误
    }

    #[test]
    fn test_validate_status_code() {
        let checker =
            HttpHealthChecker::new(Duration::from_secs(10), 0, Duration::from_secs(1)).unwrap();

        assert!(checker.validate_status_code(200, &[200, 201, 202]));
        assert!(checker.validate_status_code(201, &[200, 201, 202]));
        assert!(!checker.validate_status_code(404, &[200, 201, 202]));
        assert!(!checker.validate_status_code(500, &[200]));
    }

    #[tokio::test]
    async fn test_post_request_with_body() {
        let checker =
            HttpHealthChecker::new(Duration::from_secs(10), 0, Duration::from_secs(1)).unwrap();

        let mut service = create_test_service("https://httpbin.org/post", vec![200]);
        service.method = "POST".to_string();
        service.body = Some(serde_json::json!({"test": "data"}));
        service
            .headers
            .insert("Content-Type".to_string(), "application/json".to_string());

        let result = checker.check(&service).await;

        assert!(result.is_ok());
        let health_result = result.unwrap();
        assert_eq!(health_result.status, HealthStatus::Up);
        assert_eq!(health_result.status_code, Some(200));
    }

    #[tokio::test]
    async fn test_batch_check() {
        let checker =
            HttpHealthChecker::new(Duration::from_secs(10), 0, Duration::from_secs(1)).unwrap();

        let services = vec![
            create_test_service("https://httpbin.org/status/200", vec![200]),
            create_test_service("https://httpbin.org/status/404", vec![200]),
            create_test_service("https://httpbin.org/status/500", vec![200]),
        ];

        let results = checker.check_batch(&services).await;

        assert_eq!(results.len(), 3);

        // 第一个应该成功
        assert!(results[0].is_ok());
        assert_eq!(results[0].as_ref().unwrap().status, HealthStatus::Up);

        // 第二个和第三个应该失败（状态码不匹配）
        assert!(results[1].is_ok());
        assert_eq!(results[1].as_ref().unwrap().status, HealthStatus::Down);

        assert!(results[2].is_ok());
        assert_eq!(results[2].as_ref().unwrap().status, HealthStatus::Down);
    }

    #[tokio::test]
    async fn test_error_message_formatting() {
        let checker =
            HttpHealthChecker::new(Duration::from_secs(1), 0, Duration::from_millis(100)).unwrap();

        // 测试连接拒绝错误
        let service_connection_refused = create_test_service("http://localhost:99999", vec![200]);

        let result = checker.check(&service_connection_refused).await.unwrap();
        assert_eq!(result.status, HealthStatus::Down);
        assert!(result.error_message.is_some());
        let error_msg = result.error_message.unwrap();
        assert!(error_msg.contains("Connection refused") || error_msg.contains("Request failed"));

        // 测试HTTP错误状态码
        let service_http_error = create_test_service("https://httpbin.org/status/500", vec![200]);

        let result = checker.check(&service_http_error).await.unwrap();
        assert_eq!(result.status, HealthStatus::Down);
        assert!(result.error_message.is_some());
        let error_msg = result.error_message.unwrap();
        // 检查错误信息是否包含HTTP 500或者是网络相关错误
        assert!(
            error_msg.contains("HTTP 500")
                || error_msg.contains("Request failed")
                || error_msg.contains("Connection")
                || error_msg.contains("timeout")
        );
    }
}
