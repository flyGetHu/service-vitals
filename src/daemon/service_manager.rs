//! 统一服务管理接口
//!
//! 提供跨平台的服务管理功能统一接口

use crate::common::error::Result;
#[cfg(unix)]
use crate::daemon::PlatformDaemonManager;
use crate::daemon::{DaemonConfig, DaemonManager, DaemonStatus};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

/// 服务管理器
#[cfg(unix)]
pub struct ServiceManager {
    /// 平台特定的守护进程管理器
    daemon_manager: PlatformDaemonManager,
}

/// 服务管理器（非Unix系统）
#[cfg(not(unix))]
pub struct ServiceManager {
    // 占位符，非Unix系统暂不支持服务管理
}

#[cfg(unix)]
impl ServiceManager {
    /// 创建新的服务管理器
    pub fn new() -> Self {
        Self {
            daemon_manager: PlatformDaemonManager::new(),
        }
    }
}

#[cfg(not(unix))]
impl ServiceManager {
    /// 创建新的服务管理器（非Unix系统）
    pub fn new() -> Self {
        Self {}
    }

    /// 安装服务（非Unix系统）
    pub async fn install_service(&self, _config: &DaemonConfig) -> Result<()> {
        Err(crate::error::ServiceVitalsError::DaemonError(
            "非Unix系统不支持服务安装".to_string(),
        ))
    }

    /// 卸载服务（非Unix系统）
    pub async fn uninstall_service(&self, _service_name: &str) -> Result<()> {
        Err(crate::error::ServiceVitalsError::DaemonError(
            "非Unix系统不支持服务卸载".to_string(),
        ))
    }

    /// 启动服务（非Unix系统）
    pub async fn start_service(&self, _service_name: &str) -> Result<()> {
        Err(crate::error::ServiceVitalsError::DaemonError(
            "非Unix系统不支持服务启动".to_string(),
        ))
    }

    /// 停止服务（非Unix系统）
    pub async fn stop_service(&self, _service_name: &str) -> Result<()> {
        Err(crate::error::ServiceVitalsError::DaemonError(
            "非Unix系统不支持服务停止".to_string(),
        ))
    }

    /// 重启服务（非Unix系统）
    pub async fn restart_service(&self, _service_name: &str) -> Result<()> {
        Err(crate::error::ServiceVitalsError::DaemonError(
            "非Unix系统不支持服务重启".to_string(),
        ))
    }

    /// 获取服务状态（非Unix系统）
    pub async fn get_service_status(&self, _service_name: &str) -> Result<ServiceInfo> {
        Err(crate::error::ServiceVitalsError::DaemonError(
            "非Unix系统不支持服务状态查询".to_string(),
        ))
    }

    /// 验证配置（非Unix系统）
    pub fn validate_config(&self, _config: &DaemonConfig) -> Result<Vec<String>> {
        Ok(vec!["非Unix系统不支持服务配置验证".to_string()])
    }

    /// 生成服务配置建议（非Unix系统）
    pub fn suggest_config_improvements(&self, _config: &DaemonConfig) -> Vec<String> {
        vec!["非Unix系统不支持服务配置建议".to_string()]
    }
}

// Unix系统的ServiceManager实现
#[cfg(unix)]
impl ServiceManager {
    /// 安装服务
    pub async fn install_service(&self, config: &DaemonConfig) -> Result<()> {
        info!("开始安装服务: {}", config.service_name);

        // 检查服务是否已经安装
        if self
            .daemon_manager
            .is_installed(&config.service_name)
            .await?
        {
            info!("服务已经安装: {}", config.service_name);
            return Ok(());
        }

        // 执行安装
        self.daemon_manager.install(config).await?;
        info!("服务安装完成: {}", config.service_name);

        Ok(())
    }

    /// 卸载服务
    pub async fn uninstall_service(&self, service_name: &str) -> Result<()> {
        info!("开始卸载服务: {service_name}");

        // 检查服务是否已安装
        if !self.daemon_manager.is_installed(service_name).await? {
            info!("服务未安装: {service_name}");
            return Ok(());
        }

        // 执行卸载
        self.daemon_manager.uninstall(service_name).await?;
        info!("服务卸载完成: {service_name}");

        Ok(())
    }

    /// 启动服务
    pub async fn start_service(&self, service_name: &str) -> Result<()> {
        info!("启动服务: {service_name}");

        // 检查当前状态
        let current_status = self.daemon_manager.status(service_name).await?;
        if current_status == DaemonStatus::Running {
            info!("服务已经在运行: {service_name}");
            return Ok(());
        }

        // 启动服务
        self.daemon_manager.start(service_name).await?;
        info!("服务启动完成: {service_name}");

        Ok(())
    }

    /// 停止服务
    pub async fn stop_service(&self, service_name: &str) -> Result<()> {
        info!("停止服务: {service_name}");

        // 检查当前状态
        let current_status = self.daemon_manager.status(service_name).await?;
        if current_status == DaemonStatus::Stopped {
            info!("服务已经停止: {service_name}");
            return Ok(());
        }

        // 停止服务
        self.daemon_manager.stop(service_name).await?;
        info!("服务停止完成: {service_name}");

        Ok(())
    }

    /// 重启服务
    pub async fn restart_service(&self, service_name: &str) -> Result<()> {
        info!("重启服务: {service_name}");
        self.daemon_manager.restart(service_name).await?;
        info!("服务重启完成: {service_name}");
        Ok(())
    }

    /// 获取服务状态
    pub async fn get_service_status(&self, service_name: &str) -> Result<ServiceInfo> {
        let status = self.daemon_manager.status(service_name).await?;
        let is_installed = self.daemon_manager.is_installed(service_name).await?;

        Ok(ServiceInfo {
            name: service_name.to_string(),
            status,
            is_installed,
            platform: get_platform_name(),
        })
    }

    /// 列出所有相关服务状态
    pub async fn list_services(&self, service_names: &[String]) -> Result<Vec<ServiceInfo>> {
        let mut services = Vec::new();

        for service_name in service_names {
            match self.get_service_status(service_name).await {
                Ok(info) => services.push(info),
                Err(e) => {
                    error!("获取服务状态失败 {service_name}: {e}");
                    services.push(ServiceInfo {
                        name: service_name.clone(),
                        status: DaemonStatus::Unknown,
                        is_installed: false,
                        platform: get_platform_name(),
                    });
                }
            }
        }

        Ok(services)
    }

    /// 验证服务配置
    pub fn validate_config(&self, config: &DaemonConfig) -> Result<Vec<String>> {
        let mut warnings = Vec::new();

        // 检查可执行文件路径
        if !config.executable_path.exists() {
            warnings.push(format!(
                "可执行文件不存在: {}",
                config.executable_path.display()
            ));
        }

        // 检查配置文件路径
        if !config.config_path.exists() {
            warnings.push(format!("配置文件不存在: {}", config.config_path.display()));
        }

        // 检查工作目录
        if !config.working_directory.exists() {
            warnings.push(format!(
                "工作目录不存在: {}",
                config.working_directory.display()
            ));
        }

        // 平台特定检查
        #[cfg(unix)]
        {
            // 检查用户和组
            if let Some(ref user) = config.user {
                if user.is_empty() {
                    warnings.push("用户名不能为空".to_string());
                }
            }

            if let Some(ref group) = config.group {
                if group.is_empty() {
                    warnings.push("组名不能为空".to_string());
                }
            }

            // 检查PID文件目录权限
            if let Some(ref pid_file) = config.pid_file {
                if let Some(parent) = pid_file.parent() {
                    if !parent.exists() {
                        warnings.push(format!("PID文件目录不存在: {}", parent.display()));
                    }
                }
            }
        }

        Ok(warnings)
    }

    /// 生成服务配置建议
    pub fn suggest_config_improvements(&self, config: &DaemonConfig) -> Vec<String> {
        let mut suggestions = Vec::new();

        // 安全建议
        #[cfg(unix)]
        {
            if config.user.is_none() {
                suggestions.push("建议指定专用用户运行服务以提高安全性".to_string());
            }

            if config.user.as_ref().is_some_and(|u| u == "root") {
                suggestions.push("不建议使用root用户运行服务".to_string());
            }
        }

        // 日志建议
        if config.log_file.is_none() {
            suggestions.push("建议配置日志文件以便问题排查".to_string());
        }

        // 路径建议
        if config.working_directory.to_string_lossy().contains("tmp") {
            suggestions.push("不建议使用临时目录作为工作目录".to_string());
        }

        suggestions
    }
}

impl Default for ServiceManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 服务信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    /// 服务名称
    pub name: String,
    /// 服务状态
    pub status: DaemonStatus,
    /// 是否已安装
    pub is_installed: bool,
    /// 平台信息
    pub platform: String,
}

/// 获取平台名称
fn get_platform_name() -> String {
    #[cfg(target_os = "linux")]
    return "Linux".to_string();

    #[cfg(target_os = "macos")]
    return "macOS".to_string();

    #[cfg(target_os = "freebsd")]
    return "FreeBSD".to_string();

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "freebsd")))]
    return "Unknown".to_string();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_config_validation() {
        let manager = ServiceManager::new();
        let mut config = DaemonConfig::for_development();

        // 设置不存在的路径
        config.executable_path = PathBuf::from("/nonexistent/path");
        config.config_path = PathBuf::from("/nonexistent/config.toml");

        let warnings = manager.validate_config(&config).unwrap();
        assert!(!warnings.is_empty());
        assert!(warnings.iter().any(|w| w.contains("可执行文件不存在")));
        assert!(warnings.iter().any(|w| w.contains("配置文件不存在")));
    }

    #[test]
    fn test_config_suggestions() {
        let manager = ServiceManager::new();
        let config = DaemonConfig::for_development();

        let suggestions = manager.suggest_config_improvements(&config);
        // 应该有一些建议
        assert!(!suggestions.is_empty());
    }

    #[test]
    fn test_platform_name() {
        let platform = get_platform_name();
        assert!(!platform.is_empty());
        assert!(
            platform == "Linux"
                || platform == "macOS"
                || platform == "FreeBSD"
                || platform == "Unknown"
        );
    }

    #[test]
    fn test_service_info_serialization() {
        let info = ServiceInfo {
            name: "test-service".to_string(),
            status: DaemonStatus::Running,
            is_installed: true,
            platform: "Linux".to_string(),
        };

        let json = serde_json::to_string(&info).unwrap();
        let deserialized: ServiceInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(info.name, deserialized.name);
        assert_eq!(info.status, deserialized.status);
        assert_eq!(info.is_installed, deserialized.is_installed);
        assert_eq!(info.platform, deserialized.platform);
    }
}
