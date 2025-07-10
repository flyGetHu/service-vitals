//! Unix系统守护进程管理
//!
//! 提供systemd服务管理和传统Unix守护进程支持

use crate::daemon::{DaemonConfig, DaemonManager, DaemonStatus};
use crate::error::{Result, ServiceVitalsError};
use async_trait::async_trait;
use tracing::{debug, error, info, warn};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tokio::process::Command as AsyncCommand;

/// Unix守护进程管理器
pub struct UnixDaemonManager {
    /// 是否使用systemd
    use_systemd: bool,
}

impl Default for UnixDaemonManager {
    fn default() -> Self {
        Self::new()
    }
}

impl UnixDaemonManager {
    /// 创建新的Unix守护进程管理器
    pub fn new() -> Self {
        let use_systemd = Self::is_systemd_available();
        debug!("Unix守护进程管理器初始化，使用systemd: {use_systemd}");

        Self { use_systemd }
    }

    /// 检查systemd是否可用
    fn is_systemd_available() -> bool {
        // 检查systemctl命令是否存在
        Command::new("systemctl")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// 生成systemd服务文件内容
    fn generate_systemd_service(&self, config: &DaemonConfig) -> String {
        let mut service_content = format!(
            r#"[Unit]
Description={}
After=network.target
Wants=network.target

[Service]
Type=simple
ExecStart={} --config {} start 
ExecReload=/bin/kill -USR1 $MAINPID
Restart=always
RestartSec=5
KillMode=mixed
TimeoutStopSec=30
"#,
            config.description,
            config.executable_path.display(),
            config.config_path.display()
        );

        // 添加用户和组配置
        if let Some(ref user) = config.user {
            service_content.push_str(&format!("User={user}\n"));
        }
        if let Some(ref group) = config.group {
            service_content.push_str(&format!("Group={group}\n"));
        }

        // 添加工作目录
        let working_directory = config.working_directory.clone();
        if !working_directory.is_dir() {
            if let Err(e) = fs::create_dir_all(&working_directory) {
                error!("创建工作目录失败: {e}");
            }
            info!("创建工作目录: {}", working_directory.display());
        }

        service_content.push_str(&format!(
            "WorkingDirectory={}\n",
            working_directory.display()
        ));

        // 添加PID文件
        if let Some(ref pid_file) = config.pid_file {
            service_content.push_str(&format!("PIDFile={}\n", pid_file.display()));
        }

        // 添加日志配置
        if let Some(ref log_file) = config.log_file {
            service_content.push_str(&format!(
                "StandardOutput=append:{}\nStandardError=append:{}\n",
                log_file.display(),
                log_file.display()
            ));
        }

        // 添加安全配置
        service_content.push_str(
            r#"
# Security settings
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/service-vitals /var/log /tmp
PrivateTmp=true
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true

[Install]
WantedBy=multi-user.target
"#,
        );

        service_content
    }

    /// 获取systemd服务文件路径
    fn get_systemd_service_path(&self, service_name: &str) -> PathBuf {
        PathBuf::from(format!("/etc/systemd/system/{service_name}.service"))
    }

    /// 执行systemctl命令
    async fn run_systemctl(&self, args: &[&str]) -> Result<String> {
        let output = AsyncCommand::new("systemctl").args(args).output().await?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            Err(ServiceVitalsError::DaemonError(format!(
                "systemctl命令执行失败: {error_msg}"
            )))
        }
    }

    /// 检查进程是否运行（通过PID文件）
    #[allow(dead_code)]
    fn is_process_running_by_pid(&self, pid_file: &PathBuf) -> bool {
        if !pid_file.exists() {
            return false;
        }

        match fs::read_to_string(pid_file) {
            Ok(pid_str) => {
                if let Ok(pid) = pid_str.trim().parse::<i32>() {
                    // 检查进程是否存在
                    unsafe { libc::kill(pid, 0) == 0 }
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }

    /// 传统守护进程状态检查
    #[allow(dead_code)]
    fn get_traditional_daemon_status(&self, config: &DaemonConfig) -> DaemonStatus {
        if let Some(ref pid_file) = config.pid_file {
            if self.is_process_running_by_pid(pid_file) {
                DaemonStatus::Running
            } else {
                DaemonStatus::Stopped
            }
        } else {
            DaemonStatus::Unknown
        }
    }
}

#[async_trait]
impl DaemonManager for UnixDaemonManager {
    async fn install(&self, config: &DaemonConfig) -> Result<()> {
        info!("安装Unix守护进程服务: {}", config.service_name);

        if self.use_systemd {
            // 生成systemd服务文件
            let service_content = self.generate_systemd_service(config);
            let service_path = self.get_systemd_service_path(&config.service_name);

            // 写入服务文件
            fs::write(&service_path, service_content)?;
            info!("systemd服务文件已创建: {}", service_path.display());

            // 重新加载systemd配置
            self.run_systemctl(&["daemon-reload"]).await?;
            info!("systemd配置已重新加载");

            // 启用服务
            self.run_systemctl(&["enable", &config.service_name])
                .await?;
            info!("服务已启用: {}", config.service_name);
        } else {
            // 传统守护进程安装
            warn!("systemd不可用，使用传统守护进程模式");

            // 创建必要的目录
            if let Some(parent) = config.working_directory.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::create_dir_all(&config.working_directory)?;

            if let Some(ref log_file) = config.log_file {
                if let Some(parent) = log_file.parent() {
                    fs::create_dir_all(parent)?;
                }
            }

            info!("传统守护进程环境已准备完成");
        }

        Ok(())
    }

    async fn uninstall(&self, service_name: &str) -> Result<()> {
        info!("卸载Unix守护进程服务: {}", service_name);

        if self.use_systemd {
            // 停止服务
            let _ = self.stop(service_name).await;

            // 禁用服务
            let _ = self.run_systemctl(&["disable", service_name]).await;

            // 删除服务文件
            let service_path = self.get_systemd_service_path(service_name);
            if service_path.exists() {
                fs::remove_file(&service_path)?;
                info!("systemd服务文件已删除: {}", service_path.display());
            }

            // 重新加载systemd配置
            self.run_systemctl(&["daemon-reload"]).await?;
            info!("systemd配置已重新加载");
        } else {
            info!("传统守护进程模式，无需特殊卸载操作");
        }

        Ok(())
    }

    async fn start(&self, service_name: &str) -> Result<()> {
        info!("启动Unix守护进程服务: {}", service_name);

        if self.use_systemd {
            self.run_systemctl(&["start", service_name]).await?;
            info!("systemd服务已启动: {service_name}");
        } else {
            return Err(ServiceVitalsError::DaemonError(
                "传统守护进程模式不支持远程启动，请直接运行程序".to_string(),
            ));
        }

        Ok(())
    }

    async fn stop(&self, service_name: &str) -> Result<()> {
        info!("停止Unix守护进程服务: {}", service_name);

        if self.use_systemd {
            self.run_systemctl(&["stop", service_name]).await?;
            info!("systemd服务已停止: {service_name}");
        } else {
            return Err(ServiceVitalsError::DaemonError(
                "传统守护进程模式不支持远程停止，请使用信号终止进程".to_string(),
            ));
        }

        Ok(())
    }

    async fn restart(&self, service_name: &str) -> Result<()> {
        info!("重启Unix守护进程服务: {}", service_name);

        if self.use_systemd {
            self.run_systemctl(&["restart", service_name]).await?;
            info!("systemd服务已重启: {service_name}");
        } else {
            return Err(ServiceVitalsError::DaemonError(
                "传统守护进程模式不支持远程重启".to_string(),
            ));
        }

        Ok(())
    }

    async fn status(&self, service_name: &str) -> Result<DaemonStatus> {
        if self.use_systemd {
            match self.run_systemctl(&["is-active", service_name]).await {
                Ok(output) => {
                    let status = output.trim();
                    match status {
                        "active" => Ok(DaemonStatus::Running),
                        "inactive" => Ok(DaemonStatus::Stopped),
                        "activating" => Ok(DaemonStatus::Starting),
                        "deactivating" => Ok(DaemonStatus::Stopping),
                        _ => Ok(DaemonStatus::Unknown),
                    }
                }
                Err(_) => Ok(DaemonStatus::Unknown),
            }
        } else {
            // 传统守护进程状态检查需要配置信息
            Ok(DaemonStatus::Unknown)
        }
    }

    async fn is_installed(&self, service_name: &str) -> Result<bool> {
        if self.use_systemd {
            let service_path = self.get_systemd_service_path(service_name);
            Ok(service_path.exists())
        } else {
            // 传统守护进程模式，假设总是"已安装"
            Ok(true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unix_daemon_manager_creation() {
        let manager = UnixDaemonManager::new();
        // 测试创建是否成功
        assert!(manager.use_systemd || !manager.use_systemd); // 总是为真，只是确保创建成功
    }

    #[test]
    fn test_systemd_service_generation() {
        let manager = UnixDaemonManager::new();
        let config = DaemonConfig::default();
        let service_content = manager.generate_systemd_service(&config);

        assert!(service_content.contains("[Unit]"));
        assert!(service_content.contains("[Service]"));
        assert!(service_content.contains("[Install]"));
        assert!(service_content.contains(&config.description));
    }

    #[test]
    fn test_systemd_service_path() {
        let manager = UnixDaemonManager::new();
        let path = manager.get_systemd_service_path("test-service");
        assert_eq!(
            path,
            PathBuf::from("/etc/systemd/system/test-service.service")
        );
    }
}
