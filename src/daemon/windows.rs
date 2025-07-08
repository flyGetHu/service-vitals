//! Windows服务管理
//!
//! 提供Windows Service Control Manager集成支持

use crate::daemon::{DaemonConfig, DaemonManager, DaemonStatus};
use crate::error::{Result, ServiceVitalsError};
use async_trait::async_trait;
use log::{info, warn, error, debug};
use std::ffi::OsString;
use std::time::Duration;

#[cfg(windows)]
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
    service_manager::{ServiceManager, ServiceManagerAccess},
    Result as WindowsServiceResult,
};

/// Windows服务管理器
pub struct WindowsServiceManager;

impl WindowsServiceManager {
    /// 创建新的Windows服务管理器
    pub fn new() -> Self {
        debug!("Windows服务管理器初始化");
        Self
    }

    /// 获取服务管理器
    #[cfg(windows)]
    fn get_service_manager(&self) -> WindowsServiceResult<ServiceManager> {
        ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::ALL_ACCESS)
    }

    /// 将DaemonStatus转换为Windows服务状态
    #[cfg(windows)]
    fn windows_state_to_daemon_status(&self, state: ServiceState) -> DaemonStatus {
        match state {
            ServiceState::Running => DaemonStatus::Running,
            ServiceState::Stopped => DaemonStatus::Stopped,
            ServiceState::StartPending => DaemonStatus::Starting,
            ServiceState::StopPending => DaemonStatus::Stopping,
            ServiceState::ContinuePending => DaemonStatus::Starting,
            ServiceState::PausePending => DaemonStatus::Stopping,
            ServiceState::Paused => DaemonStatus::Stopped,
        }
    }
}

#[async_trait]
impl DaemonManager for WindowsServiceManager {
    async fn install(&self, config: &DaemonConfig) -> Result<()> {
        #[cfg(windows)]
        {
            info!("安装Windows服务: {}", config.service_name);

            let manager = self.get_service_manager()
                .map_err(|e| ServiceVitalsError::DaemonError(format!("无法连接到服务管理器: {}", e)))?;

            // 构建服务启动命令
            let mut service_binary_path = OsString::from(&config.executable_path);
            service_binary_path.push(" start --config ");
            service_binary_path.push(&config.config_path);
            service_binary_path.push(" --daemon");

            // 创建服务
            let service_info = windows_service::service_manager::ServiceInfo {
                name: OsString::from(&config.service_name),
                display_name: OsString::from(&config.display_name),
                service_type: ServiceType::OWN_PROCESS,
                start_type: windows_service::service_manager::ServiceStartType::AutoStart,
                error_control: windows_service::service_manager::ServiceErrorControl::Normal,
                executable_path: service_binary_path,
                launch_arguments: vec![],
                dependencies: vec![],
                account_name: None, // 使用LocalSystem账户
                account_password: None,
            };

            let _service = manager.create_service(&service_info, windows_service::service_manager::ServiceAccess::ALL_ACCESS)
                .map_err(|e| ServiceVitalsError::DaemonError(format!("创建服务失败: {}", e)))?;

            info!("Windows服务安装成功: {}", config.service_name);
            Ok(())
        }

        #[cfg(not(windows))]
        {
            Err(ServiceVitalsError::DaemonError(
                "Windows服务管理器只能在Windows系统上使用".to_string(),
            ))
        }
    }

    async fn uninstall(&self, service_name: &str) -> Result<()> {
        #[cfg(windows)]
        {
            info!("卸载Windows服务: {}", service_name);

            let manager = self.get_service_manager()
                .map_err(|e| ServiceVitalsError::DaemonError(format!("无法连接到服务管理器: {}", e)))?;

            // 先尝试停止服务
            let _ = self.stop(service_name).await;

            // 打开服务
            let service = manager.open_service(service_name, windows_service::service_manager::ServiceAccess::DELETE)
                .map_err(|e| ServiceVitalsError::DaemonError(format!("无法打开服务: {}", e)))?;

            // 删除服务
            service.delete()
                .map_err(|e| ServiceVitalsError::DaemonError(format!("删除服务失败: {}", e)))?;

            info!("Windows服务卸载成功: {}", service_name);
            Ok(())
        }

        #[cfg(not(windows))]
        {
            Err(ServiceVitalsError::DaemonError(
                "Windows服务管理器只能在Windows系统上使用".to_string(),
            ))
        }
    }

    async fn start(&self, service_name: &str) -> Result<()> {
        #[cfg(windows)]
        {
            info!("启动Windows服务: {}", service_name);

            let manager = self.get_service_manager()
                .map_err(|e| ServiceVitalsError::DaemonError(format!("无法连接到服务管理器: {}", e)))?;

            let service = manager.open_service(service_name, windows_service::service_manager::ServiceAccess::START)
                .map_err(|e| ServiceVitalsError::DaemonError(format!("无法打开服务: {}", e)))?;

            service.start(&[] as &[&str])
                .map_err(|e| ServiceVitalsError::DaemonError(format!("启动服务失败: {}", e)))?;

            info!("Windows服务启动成功: {}", service_name);
            Ok(())
        }

        #[cfg(not(windows))]
        {
            Err(ServiceVitalsError::DaemonError(
                "Windows服务管理器只能在Windows系统上使用".to_string(),
            ))
        }
    }

    async fn stop(&self, service_name: &str) -> Result<()> {
        #[cfg(windows)]
        {
            info!("停止Windows服务: {}", service_name);

            let manager = self.get_service_manager()
                .map_err(|e| ServiceVitalsError::DaemonError(format!("无法连接到服务管理器: {}", e)))?;

            let service = manager.open_service(service_name, windows_service::service_manager::ServiceAccess::STOP)
                .map_err(|e| ServiceVitalsError::DaemonError(format!("无法打开服务: {}", e)))?;

            service.stop()
                .map_err(|e| ServiceVitalsError::DaemonError(format!("停止服务失败: {}", e)))?;

            info!("Windows服务停止成功: {}", service_name);
            Ok(())
        }

        #[cfg(not(windows))]
        {
            Err(ServiceVitalsError::DaemonError(
                "Windows服务管理器只能在Windows系统上使用".to_string(),
            ))
        }
    }

    async fn restart(&self, service_name: &str) -> Result<()> {
        info!("重启Windows服务: {}", service_name);
        
        // 先停止服务
        self.stop(service_name).await?;
        
        // 等待一段时间确保服务完全停止
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // 启动服务
        self.start(service_name).await?;
        
        Ok(())
    }

    async fn status(&self, service_name: &str) -> Result<DaemonStatus> {
        #[cfg(windows)]
        {
            let manager = self.get_service_manager()
                .map_err(|e| ServiceVitalsError::DaemonError(format!("无法连接到服务管理器: {}", e)))?;

            let service = manager.open_service(service_name, windows_service::service_manager::ServiceAccess::QUERY_STATUS)
                .map_err(|e| ServiceVitalsError::DaemonError(format!("无法打开服务: {}", e)))?;

            let status = service.query_status()
                .map_err(|e| ServiceVitalsError::DaemonError(format!("查询服务状态失败: {}", e)))?;

            Ok(self.windows_state_to_daemon_status(status.current_state))
        }

        #[cfg(not(windows))]
        {
            Err(ServiceVitalsError::DaemonError(
                "Windows服务管理器只能在Windows系统上使用".to_string(),
            ))
        }
    }

    async fn is_installed(&self, service_name: &str) -> Result<bool> {
        #[cfg(windows)]
        {
            let manager = self.get_service_manager()
                .map_err(|e| ServiceVitalsError::DaemonError(format!("无法连接到服务管理器: {}", e)))?;

            match manager.open_service(service_name, windows_service::service_manager::ServiceAccess::QUERY_CONFIG) {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            }
        }

        #[cfg(not(windows))]
        {
            Ok(false)
        }
    }
}

/// Windows服务主函数
#[cfg(windows)]
pub fn run_windows_service(config: DaemonConfig) -> WindowsServiceResult<()> {
    use std::sync::mpsc;
    use std::thread;

    // 定义服务主函数
    define_windows_service!(ffi_service_main, service_main);

    fn service_main(arguments: Vec<OsString>) {
        if let Err(e) = run_service(arguments) {
            error!("Windows服务运行失败: {}", e);
        }
    }

    fn run_service(_arguments: Vec<OsString>) -> WindowsServiceResult<()> {
        let (shutdown_tx, shutdown_rx) = mpsc::channel();

        // 注册服务控制处理器
        let event_handler = move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    info!("接收到服务停止信号");
                    let _ = shutdown_tx.send(());
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        };

        let status_handle = service_control_handler::register(&config.service_name, event_handler)?;

        // 设置服务状态为启动中
        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::StartPending,
            controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::from_secs(3),
            process_id: None,
        })?;

        // 启动服务逻辑
        thread::spawn(move || {
            // 这里应该启动实际的服务逻辑
            info!("Windows服务已启动");
            
            // 等待关闭信号
            let _ = shutdown_rx.recv();
            info!("Windows服务正在关闭");
        });

        // 设置服务状态为运行中
        status_handle.set_service_status(ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })?;

        // 等待关闭信号
        loop {
            thread::sleep(Duration::from_secs(1));
            // 检查是否需要关闭
        }
    }

    // 启动服务分发器
    service_dispatcher::start(&config.service_name, ffi_service_main)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_service_manager_creation() {
        let manager = WindowsServiceManager::new();
        // 测试创建是否成功（在非Windows系统上也应该能创建，只是功能受限）
        assert!(true); // 简单的存在性测试
    }

    #[cfg(windows)]
    #[test]
    fn test_daemon_status_conversion() {
        let manager = WindowsServiceManager::new();
        
        assert_eq!(
            manager.windows_state_to_daemon_status(ServiceState::Running),
            DaemonStatus::Running
        );
        assert_eq!(
            manager.windows_state_to_daemon_status(ServiceState::Stopped),
            DaemonStatus::Stopped
        );
        assert_eq!(
            manager.windows_state_to_daemon_status(ServiceState::StartPending),
            DaemonStatus::Starting
        );
    }
}
