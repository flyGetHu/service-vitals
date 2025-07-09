//! 服务状态管理模块
//!
//! 提供服务运行状态的查询和管理功能

use crate::health::{HealthResult, HealthStatus};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 服务运行状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    /// 服务名称
    pub name: String,
    /// 服务URL
    pub url: String,
    /// 当前状态
    pub status: HealthStatus,
    /// 最后检测时间
    pub last_check: Option<DateTime<Utc>>,
    /// 状态码
    pub status_code: Option<u16>,
    /// 响应时间（毫秒）
    pub response_time_ms: Option<u64>,
    /// 连续失败次数
    pub consecutive_failures: u32,
    /// 错误信息
    pub error_message: Option<String>,
    /// 是否启用
    pub enabled: bool,
}

/// 整体服务状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverallStatus {
    /// 服务启动时间
    pub start_time: DateTime<Utc>,
    /// 配置文件路径
    pub config_path: PathBuf,
    /// 总服务数
    pub total_services: usize,
    /// 健康服务数
    pub healthy_services: usize,
    /// 不健康服务数
    pub unhealthy_services: usize,
    /// 禁用服务数
    pub disabled_services: usize,
    /// 最后配置重载时间
    pub last_config_reload: Option<DateTime<Utc>>,
    /// 服务详细状态
    pub services: Vec<ServiceStatus>,
}

/// 状态管理器
#[derive(Debug)]
pub struct StatusManager {
    /// 服务状态映射
    service_status: Arc<RwLock<HashMap<String, ServiceStatus>>>,
    /// 服务启动时间
    start_time: DateTime<Utc>,
    /// 配置文件路径
    config_path: PathBuf,
    /// 最后配置重载时间
    last_config_reload: Arc<RwLock<Option<DateTime<Utc>>>>,
}

impl StatusManager {
    /// 创建新的状态管理器
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            service_status: Arc::new(RwLock::new(HashMap::new())),
            start_time: Utc::now(),
            config_path,
            last_config_reload: Arc::new(RwLock::new(None)),
        }
    }

    /// 更新服务状态
    pub async fn update_service_status(&self, result: &HealthResult) {
        let mut status_map = self.service_status.write().await;

        let service_status = ServiceStatus {
            name: result.service_name.clone(),
            url: result.service_url.clone(),
            status: result.status,
            last_check: Some(result.timestamp),
            status_code: result.status_code,
            response_time_ms: Some(result.response_time.as_millis() as u64),
            consecutive_failures: result.consecutive_failures,
            error_message: result.error_message.clone(),
            enabled: true, // 假设运行中的服务都是启用的
        };

        status_map.insert(result.service_name.clone(), service_status);
    }

    /// 添加服务（初始状态）
    pub async fn add_service(&self, name: String, url: String, enabled: bool) {
        let mut status_map = self.service_status.write().await;

        let service_status = ServiceStatus {
            name: name.clone(),
            url,
            status: HealthStatus::Unknown,
            last_check: None,
            status_code: None,
            response_time_ms: None,
            consecutive_failures: 0,
            error_message: None,
            enabled,
        };

        status_map.insert(name, service_status);
    }

    /// 移除服务
    pub async fn remove_service(&self, name: &str) {
        let mut status_map = self.service_status.write().await;
        status_map.remove(name);
    }

    /// 标记配置重载
    pub async fn mark_config_reload(&self) {
        let mut last_reload = self.last_config_reload.write().await;
        *last_reload = Some(Utc::now());
    }

    /// 获取整体状态
    pub async fn get_overall_status(&self) -> OverallStatus {
        let status_map = self.service_status.read().await;
        let last_reload = self.last_config_reload.read().await;

        let services: Vec<ServiceStatus> = status_map.values().cloned().collect();

        let total_services = services.len();
        let healthy_services = services
            .iter()
            .filter(|s| s.enabled && s.status.is_healthy())
            .count();
        let unhealthy_services = services
            .iter()
            .filter(|s| s.enabled && !s.status.is_healthy() && s.status != HealthStatus::Unknown)
            .count();
        let disabled_services = services.iter().filter(|s| !s.enabled).count();

        OverallStatus {
            start_time: self.start_time,
            config_path: self.config_path.clone(),
            total_services,
            healthy_services,
            unhealthy_services,
            disabled_services,
            last_config_reload: *last_reload,
            services,
        }
    }

    /// 获取特定服务状态
    pub async fn get_service_status(&self, name: &str) -> Option<ServiceStatus> {
        let status_map = self.service_status.read().await;
        status_map.get(name).cloned()
    }

    /// 获取所有服务状态
    pub async fn get_all_services(&self) -> Vec<ServiceStatus> {
        let status_map = self.service_status.read().await;
        status_map.values().cloned().collect()
    }

    /// 保存状态到文件
    pub async fn save_to_file(&self, path: &PathBuf) -> Result<()> {
        let status = self.get_overall_status().await;
        let json_data = serde_json::to_string_pretty(&status).context("序列化状态数据失败")?;

        // 确保目录存在
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("创建状态文件目录失败")?;
        }

        fs::write(path, json_data).context("写入状态文件失败")?;

        Ok(())
    }

    /// 从文件加载状态
    pub async fn load_from_file(path: &PathBuf) -> Result<OverallStatus> {
        let json_data = fs::read_to_string(path).context("读取状态文件失败")?;

        let status: OverallStatus = serde_json::from_str(&json_data).context("解析状态文件失败")?;

        Ok(status)
    }

    /// 获取默认状态文件路径
    pub fn get_default_status_file_path() -> PathBuf {
        PathBuf::from("/tmp/service-vitals-status.json")
    }
}

impl Default for StatusManager {
    fn default() -> Self {
        Self::new(PathBuf::from("config.toml"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_status_manager_creation() {
        let manager = StatusManager::new(PathBuf::from("test.toml"));
        let status = manager.get_overall_status().await;

        assert_eq!(status.total_services, 0);
        assert_eq!(status.healthy_services, 0);
        assert_eq!(status.unhealthy_services, 0);
    }

    #[tokio::test]
    async fn test_add_and_update_service() {
        let manager = StatusManager::new(PathBuf::from("test.toml"));

        // 添加服务
        manager
            .add_service(
                "test-service".to_string(),
                "http://example.com".to_string(),
                true,
            )
            .await;

        let status = manager.get_overall_status().await;
        assert_eq!(status.total_services, 1);

        // 更新服务状态
        let health_result = HealthResult::new(
            "test-service".to_string(),
            "http://example.com".to_string(),
            HealthStatus::Up,
            "GET".to_string(),
        )
        .with_status_code(200)
        .with_response_time(Duration::from_millis(100));

        manager.update_service_status(&health_result).await;

        let updated_status = manager.get_overall_status().await;
        assert_eq!(updated_status.healthy_services, 1);
    }

    #[tokio::test]
    async fn test_config_reload_tracking() {
        let manager = StatusManager::new(PathBuf::from("test.toml"));

        let status_before = manager.get_overall_status().await;
        assert!(status_before.last_config_reload.is_none());

        manager.mark_config_reload().await;

        let status_after = manager.get_overall_status().await;
        assert!(status_after.last_config_reload.is_some());
    }
}
