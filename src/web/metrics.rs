//! Prometheus指标模块
//!
//! 提供Prometheus格式的指标导出

use crate::status::{OverallStatus, ServiceStatus};
use prometheus::{
    CounterVec, Encoder, Gauge, GaugeVec, HistogramOpts, HistogramVec, Opts,
    Registry, TextEncoder,
};
use std::sync::Arc;
use warp::Filter;

/// Prometheus指标收集器
pub struct MetricsCollector {
    /// 注册表
    registry: Registry,
    /// 健康检查总数计数器
    health_check_total: CounterVec,
    /// 响应时间直方图
    response_time_histogram: HistogramVec,
    /// 服务状态指标
    service_up: GaugeVec,
    /// 最后检查时间戳
    last_check_timestamp: GaugeVec,
    /// 连续失败次数
    consecutive_failures: GaugeVec,
    /// 系统指标
    system_info: Gauge,
    /// 启动时间
    start_time: Gauge,
}

impl MetricsCollector {
    /// 创建新的指标收集器
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();

        // 健康检查总数计数器
        let health_check_total = CounterVec::new(
            Opts::new(
                "service_vitals_health_check_total",
                "Total number of health checks performed",
            ),
            &["service", "status"],
        )?;

        // 响应时间直方图
        let response_time_histogram = HistogramVec::new(
            HistogramOpts::new(
                "service_vitals_response_time_seconds",
                "Response time of health checks in seconds",
            )
            .buckets(vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
            ]),
            &["service"],
        )?;

        // 服务状态指标（1=up, 0=down）
        let service_up = GaugeVec::new(
            Opts::new("service_vitals_up", "Whether the service is up (1) or down (0)"),
            &["service", "url"],
        )?;

        // 最后检查时间戳
        let last_check_timestamp = GaugeVec::new(
            Opts::new(
                "service_vitals_last_check_timestamp",
                "Timestamp of the last health check",
            ),
            &["service"],
        )?;

        // 连续失败次数
        let consecutive_failures = GaugeVec::new(
            Opts::new(
                "service_vitals_consecutive_failures",
                "Number of consecutive failures for the service",
            ),
            &["service"],
        )?;

        // 系统信息
        let system_info = Gauge::new(
            "service_vitals_build_info",
            "Build information about service-vitals",
        )?;

        // 启动时间
        let start_time = Gauge::new(
            "service_vitals_start_time_seconds",
            "Start time of the service-vitals process since unix epoch in seconds",
        )?;

        // 注册所有指标
        registry.register(Box::new(health_check_total.clone()))?;
        registry.register(Box::new(response_time_histogram.clone()))?;
        registry.register(Box::new(service_up.clone()))?;
        registry.register(Box::new(last_check_timestamp.clone()))?;
        registry.register(Box::new(consecutive_failures.clone()))?;
        registry.register(Box::new(system_info.clone()))?;
        registry.register(Box::new(start_time.clone()))?;

        // 设置系统信息
        system_info.set(1.0);

        Ok(Self {
            registry,
            health_check_total,
            response_time_histogram,
            service_up,
            last_check_timestamp,
            consecutive_failures,
            system_info,
            start_time,
        })
    }

    /// 更新服务指标
    pub fn update_service_metrics(&self, service: &ServiceStatus) {
        let service_name = &service.name;
        let service_url = &service.url;

        // 更新服务状态
        let up_value = if service.status.is_healthy() { 1.0 } else { 0.0 };
        self.service_up
            .with_label_values(&[service_name, service_url])
            .set(up_value);

        // 更新连续失败次数
        self.consecutive_failures
            .with_label_values(&[service_name])
            .set(service.consecutive_failures as f64);

        // 更新最后检查时间戳
        if let Some(last_check) = service.last_check {
            self.last_check_timestamp
                .with_label_values(&[service_name])
                .set(last_check.timestamp() as f64);
        }

        // 更新响应时间
        if let Some(response_time_ms) = service.response_time_ms {
            let response_time_seconds = response_time_ms as f64 / 1000.0;
            self.response_time_histogram
                .with_label_values(&[service_name])
                .observe(response_time_seconds);
        }

        // 增加健康检查计数
        let status_label = if service.status.is_healthy() { "up" } else { "down" };
        self.health_check_total
            .with_label_values(&[service_name, status_label])
            .inc();
    }

    /// 设置启动时间
    pub fn set_start_time(&self, start_time: chrono::DateTime<chrono::Utc>) {
        self.start_time.set(start_time.timestamp() as f64);
    }

    /// 更新所有服务指标
    pub fn update_all_metrics(&self, status: &OverallStatus) {
        // 设置启动时间
        self.set_start_time(status.start_time);

        // 更新每个服务的指标
        for service in &status.services {
            self.update_service_metrics(service);
        }
    }

    /// 获取Prometheus格式的指标
    pub fn gather_metrics(&self) -> Result<String, prometheus::Error> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8_lossy(&buffer).to_string())
    }

    /// 获取指标摘要
    pub fn get_metrics_summary(&self) -> MetricsSummary {
        let metric_families = self.registry.gather();
        let mut summary = MetricsSummary::default();

        for family in metric_families {
            match family.get_name() {
                "service_vitals_health_check_total" => {
                    summary.total_checks = family
                        .get_metric()
                        .iter()
                        .map(|m| m.get_counter().get_value() as u64)
                        .sum();
                }
                "service_vitals_up" => {
                    summary.services_up = family
                        .get_metric()
                        .iter()
                        .filter(|m| m.get_gauge().get_value() > 0.0)
                        .count();
                    summary.services_total = family.get_metric().len();
                }
                _ => {}
            }
        }

        summary
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new().expect("Failed to create metrics collector")
    }
}

/// 指标摘要
#[derive(Debug, Default, serde::Serialize)]
pub struct MetricsSummary {
    /// 总检查次数
    pub total_checks: u64,
    /// 正常服务数
    pub services_up: usize,
    /// 总服务数
    pub services_total: usize,
    /// 健康度百分比
    pub health_percentage: f64,
}

impl MetricsSummary {
    /// 计算健康度百分比
    pub fn calculate_health_percentage(&mut self) {
        if self.services_total > 0 {
            self.health_percentage = (self.services_up as f64 / self.services_total as f64) * 100.0;
        } else {
            self.health_percentage = 0.0;
        }
    }
}

/// 创建指标端点
pub fn create_metrics_route(
    collector: Arc<MetricsCollector>,
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("metrics")
        .and(warp::path::end())
        .and(warp::get())
        .and(with_collector(collector))
        .and_then(metrics_handler)
}

/// 指标收集器注入过滤器
fn with_collector(
    collector: Arc<MetricsCollector>,
) -> impl warp::Filter<Extract = (Arc<MetricsCollector>,), Error = std::convert::Infallible> + Clone
{
    warp::any().map(move || collector.clone())
}

/// 指标处理器
async fn metrics_handler(
    collector: Arc<MetricsCollector>,
) -> Result<impl warp::Reply, warp::Rejection> {
    match collector.gather_metrics() {
        Ok(metrics) => Ok(warp::reply::with_header(
            metrics,
            "content-type",
            "text/plain; version=0.0.4; charset=utf-8",
        )),
        Err(_) => Err(warp::reject::custom(super::ApiError::new(
            500,
            "Failed to gather metrics".to_string(),
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::health::HealthStatus;

    #[test]
    fn test_metrics_collector_creation() {
        let collector = MetricsCollector::new();
        assert!(collector.is_ok());
    }

    #[test]
    fn test_update_service_metrics() {
        let collector = MetricsCollector::new().unwrap();
        
        let service = ServiceStatus {
            name: "test-service".to_string(),
            url: "http://example.com".to_string(),
            status: HealthStatus::Up,
            last_check: Some(chrono::Utc::now()),
            status_code: Some(200),
            response_time_ms: Some(150),
            consecutive_failures: 0,
            error_message: None,
            enabled: true,
        };

        collector.update_service_metrics(&service);
        
        // 验证指标是否正确更新
        let metrics = collector.gather_metrics().unwrap();
        assert!(metrics.contains("service_vitals_up"));
        assert!(metrics.contains("test-service"));
    }

    #[test]
    fn test_gather_metrics() {
        let collector = MetricsCollector::new().unwrap();
        let metrics = collector.gather_metrics().unwrap();
        
        assert!(metrics.contains("service_vitals_build_info"));
        assert!(metrics.contains("service_vitals_start_time_seconds"));
    }

    #[test]
    fn test_metrics_summary() {
        let mut summary = MetricsSummary {
            services_up: 3,
            services_total: 4,
            ..Default::default()
        };
        
        summary.calculate_health_percentage();
        assert_eq!(summary.health_percentage, 75.0);
    }

    #[test]
    fn test_metrics_summary_zero_services() {
        let mut summary = MetricsSummary::default();
        summary.calculate_health_percentage();
        assert_eq!(summary.health_percentage, 0.0);
    }
}
