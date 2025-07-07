//! 健康检测结果数据结构
//!
//! 定义健康检测的结果类型和状态枚举

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// 健康状态枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// 服务正常
    Up,
    /// 服务异常
    Down,
    /// 服务状态未知
    Unknown,
    /// 服务降级
    Degraded,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Up => write!(f, "正常"),
            HealthStatus::Down => write!(f, "异常"),
            HealthStatus::Unknown => write!(f, "未知"),
            HealthStatus::Degraded => write!(f, "降级"),
        }
    }
}

impl HealthStatus {
    /// 判断状态是否为健康
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Up)
    }

    /// 判断状态是否需要告警
    pub fn needs_alert(&self) -> bool {
        matches!(self, HealthStatus::Down | HealthStatus::Degraded)
    }
}

/// 健康检测结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResult {
    /// 检测ID
    pub id: Uuid,
    /// 服务名称
    pub service_name: String,
    /// 服务URL
    pub service_url: String,
    /// 检测时间戳
    pub timestamp: DateTime<Utc>,
    /// 健康状态
    pub status: HealthStatus,
    /// HTTP状态码（如果适用）
    pub status_code: Option<u16>,
    /// 响应时间
    #[serde(with = "duration_serde")]
    pub response_time: Duration,
    /// 错误信息（如果有）
    pub error_message: Option<String>,
    /// 连续失败次数
    pub consecutive_failures: u32,
    /// 检测方法
    pub method: String,
    /// 响应体大小（字节）
    pub response_size: Option<usize>,
    /// 额外的元数据
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

impl HealthResult {
    /// 创建新的健康检测结果
    ///
    /// # 参数
    /// * `service_name` - 服务名称
    /// * `service_url` - 服务URL
    /// * `status` - 健康状态
    /// * `method` - 检测方法
    ///
    /// # 返回
    /// * `Self` - 健康检测结果实例
    pub fn new(
        service_name: String,
        service_url: String,
        status: HealthStatus,
        method: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            service_name,
            service_url,
            timestamp: Utc::now(),
            status,
            status_code: None,
            response_time: Duration::from_millis(0),
            error_message: None,
            consecutive_failures: 0,
            method,
            response_size: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// 设置HTTP状态码
    pub fn with_status_code(mut self, status_code: u16) -> Self {
        self.status_code = Some(status_code);
        self
    }

    /// 设置响应时间
    pub fn with_response_time(mut self, response_time: Duration) -> Self {
        self.response_time = response_time;
        self
    }

    /// 设置错误信息
    pub fn with_error(mut self, error_message: String) -> Self {
        self.error_message = Some(error_message);
        self
    }

    /// 设置连续失败次数
    pub fn with_consecutive_failures(mut self, consecutive_failures: u32) -> Self {
        self.consecutive_failures = consecutive_failures;
        self
    }

    /// 设置响应体大小
    pub fn with_response_size(mut self, response_size: usize) -> Self {
        self.response_size = Some(response_size);
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: String, value: serde_json::Value) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// 获取响应时间（毫秒）
    pub fn response_time_ms(&self) -> u64 {
        self.response_time.as_millis() as u64
    }

    /// 判断是否需要发送通知
    pub fn should_notify(&self, failure_threshold: u32) -> bool {
        self.status.needs_alert() && self.consecutive_failures >= failure_threshold
    }

    /// 转换为JSON字符串
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// 从JSON字符串创建
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Duration序列化模块
mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_millis().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}

/// 健康检测统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStats {
    /// 总检测次数
    pub total_checks: u64,
    /// 成功次数
    pub successful_checks: u64,
    /// 失败次数
    pub failed_checks: u64,
    /// 平均响应时间（毫秒）
    pub average_response_time_ms: f64,
    /// 最大响应时间（毫秒）
    pub max_response_time_ms: u64,
    /// 最小响应时间（毫秒）
    pub min_response_time_ms: u64,
    /// 成功率（百分比）
    pub success_rate: f64,
    /// 最后检测时间
    pub last_check_time: Option<DateTime<Utc>>,
}

impl Default for HealthStats {
    fn default() -> Self {
        Self {
            total_checks: 0,
            successful_checks: 0,
            failed_checks: 0,
            average_response_time_ms: 0.0,
            max_response_time_ms: 0,
            min_response_time_ms: u64::MAX,
            success_rate: 0.0,
            last_check_time: None,
        }
    }
}

impl HealthStats {
    /// 更新统计信息
    pub fn update(&mut self, result: &HealthResult) {
        self.total_checks += 1;
        self.last_check_time = Some(result.timestamp);

        let response_time_ms = result.response_time_ms();

        if result.status.is_healthy() {
            self.successful_checks += 1;
        } else {
            self.failed_checks += 1;
        }

        // 更新响应时间统计
        if response_time_ms > 0 {
            self.max_response_time_ms = self.max_response_time_ms.max(response_time_ms);
            self.min_response_time_ms = self.min_response_time_ms.min(response_time_ms);

            // 计算平均响应时间
            let total_time = self.average_response_time_ms * (self.total_checks - 1) as f64
                + response_time_ms as f64;
            self.average_response_time_ms = total_time / self.total_checks as f64;
        }

        // 计算成功率
        self.success_rate = if self.total_checks > 0 {
            (self.successful_checks as f64 / self.total_checks as f64) * 100.0
        } else {
            0.0
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_health_status_display() {
        assert_eq!(HealthStatus::Up.to_string(), "正常");
        assert_eq!(HealthStatus::Down.to_string(), "异常");
        assert_eq!(HealthStatus::Unknown.to_string(), "未知");
        assert_eq!(HealthStatus::Degraded.to_string(), "降级");
    }

    #[test]
    fn test_health_status_is_healthy() {
        assert!(HealthStatus::Up.is_healthy());
        assert!(!HealthStatus::Down.is_healthy());
        assert!(!HealthStatus::Unknown.is_healthy());
        assert!(!HealthStatus::Degraded.is_healthy());
    }

    #[test]
    fn test_health_status_needs_alert() {
        assert!(!HealthStatus::Up.needs_alert());
        assert!(HealthStatus::Down.needs_alert());
        assert!(!HealthStatus::Unknown.needs_alert());
        assert!(HealthStatus::Degraded.needs_alert());
    }

    #[test]
    fn test_health_result_creation() {
        let result = HealthResult::new(
            "Test Service".to_string(),
            "https://example.com".to_string(),
            HealthStatus::Up,
            "GET".to_string(),
        );

        assert_eq!(result.service_name, "Test Service");
        assert_eq!(result.service_url, "https://example.com");
        assert_eq!(result.status, HealthStatus::Up);
        assert_eq!(result.method, "GET");
        assert_eq!(result.consecutive_failures, 0);
        assert!(result.error_message.is_none());
    }

    #[test]
    fn test_health_result_builder_pattern() {
        let result = HealthResult::new(
            "Test Service".to_string(),
            "https://example.com".to_string(),
            HealthStatus::Down,
            "GET".to_string(),
        )
        .with_status_code(500)
        .with_response_time(Duration::from_millis(1500))
        .with_error("Internal Server Error".to_string())
        .with_consecutive_failures(3)
        .with_response_size(1024)
        .with_metadata(
            "region".to_string(),
            serde_json::Value::String("us-east-1".to_string()),
        );

        assert_eq!(result.status_code, Some(500));
        assert_eq!(result.response_time_ms(), 1500);
        assert_eq!(
            result.error_message,
            Some("Internal Server Error".to_string())
        );
        assert_eq!(result.consecutive_failures, 3);
        assert_eq!(result.response_size, Some(1024));
        assert_eq!(
            result.metadata.get("region"),
            Some(&serde_json::Value::String("us-east-1".to_string()))
        );
    }

    #[test]
    fn test_health_result_serialization() {
        let result = HealthResult::new(
            "Test Service".to_string(),
            "https://example.com".to_string(),
            HealthStatus::Up,
            "GET".to_string(),
        )
        .with_status_code(200)
        .with_response_time(Duration::from_millis(500));

        // 测试JSON序列化
        let json = result.to_json().unwrap();
        assert!(!json.is_empty());
        assert!(json.contains("Test Service"));
        assert!(json.contains("up"));

        // 测试JSON反序列化
        let deserialized = HealthResult::from_json(&json).unwrap();
        assert_eq!(deserialized.service_name, result.service_name);
        assert_eq!(deserialized.status, result.status);
        assert_eq!(deserialized.status_code, result.status_code);
        assert_eq!(deserialized.response_time_ms(), result.response_time_ms());
    }

    #[test]
    fn test_health_result_should_notify() {
        let result_success = HealthResult::new(
            "Test".to_string(),
            "https://example.com".to_string(),
            HealthStatus::Up,
            "GET".to_string(),
        );
        assert!(!result_success.should_notify(1));

        let result_failure = HealthResult::new(
            "Test".to_string(),
            "https://example.com".to_string(),
            HealthStatus::Down,
            "GET".to_string(),
        )
        .with_consecutive_failures(3);

        assert!(!result_failure.should_notify(5)); // 阈值未达到
        assert!(result_failure.should_notify(3)); // 阈值达到
        assert!(result_failure.should_notify(1)); // 阈值超过
    }

    #[test]
    fn test_health_stats_update() {
        let mut stats = HealthStats::default();

        // 第一次成功检测
        let result1 = HealthResult::new(
            "Test".to_string(),
            "https://example.com".to_string(),
            HealthStatus::Up,
            "GET".to_string(),
        )
        .with_response_time(Duration::from_millis(100));

        stats.update(&result1);
        assert_eq!(stats.total_checks, 1);
        assert_eq!(stats.successful_checks, 1);
        assert_eq!(stats.failed_checks, 0);
        assert_eq!(stats.success_rate, 100.0);
        assert_eq!(stats.average_response_time_ms, 100.0);

        // 第二次失败检测
        let result2 = HealthResult::new(
            "Test".to_string(),
            "https://example.com".to_string(),
            HealthStatus::Down,
            "GET".to_string(),
        )
        .with_response_time(Duration::from_millis(200));

        stats.update(&result2);
        assert_eq!(stats.total_checks, 2);
        assert_eq!(stats.successful_checks, 1);
        assert_eq!(stats.failed_checks, 1);
        assert_eq!(stats.success_rate, 50.0);
        assert_eq!(stats.average_response_time_ms, 150.0);
        assert_eq!(stats.max_response_time_ms, 200);
        assert_eq!(stats.min_response_time_ms, 100);
    }
}
