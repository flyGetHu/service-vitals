//! 守护进程/服务管理模块
//!
//! 提供跨平台的守护进程和系统服务支持

use crate::error::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::sync::broadcast;

pub mod service_manager;
pub mod signal_handler;

#[cfg(unix)]
pub mod unix;

/// 守护进程配置
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// 服务名称
    pub service_name: String,
    /// 服务显示名称
    pub display_name: String,
    /// 服务描述
    pub description: String,
    /// 可执行文件路径
    pub executable_path: PathBuf,
    /// 配置文件路径
    pub config_path: PathBuf,
    /// 工作目录
    pub working_directory: PathBuf,
    /// PID文件路径（Unix系统）
    pub pid_file: Option<PathBuf>,
    /// 日志文件路径
    pub log_file: Option<PathBuf>,
    /// 运行用户（Unix系统）
    pub user: Option<String>,
    /// 运行组（Unix系统）
    pub group: Option<String>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            service_name: "service-vitals".to_string(),
            display_name: "Service Vitals Monitor".to_string(),
            description: "Service health monitoring and alerting system".to_string(),
            executable_path: std::env::current_exe()
                .unwrap_or_else(|_| PathBuf::from("service-vitals")),
            config_path: PathBuf::from("/etc/service-vitals/config.toml"),
            working_directory: PathBuf::from("/var/lib/service-vitals"),
            pid_file: Some(PathBuf::from("/var/run/service-vitals.pid")),
            log_file: Some(PathBuf::from("/var/log/service-vitals.log")),
            user: Some("service-vitals".to_string()),
            group: Some("service-vitals".to_string()),
        }
    }
}

impl DaemonConfig {
    /// 创建开发环境配置
    pub fn for_development() -> Self {
        let mut config = Self::default();
        config.config_path = PathBuf::from("./config.toml");
        config.working_directory = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        config.pid_file = Some(PathBuf::from("./service-vitals.pid"));
        config.log_file = Some(PathBuf::from("./service-vitals.log"));
        config.user = None;
        config.group = None;
        config
    }
}

/// 守护进程状态
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DaemonStatus {
    /// 运行中
    Running,
    /// 已停止
    Stopped,
    /// 未知状态
    Unknown,
    /// 正在启动
    Starting,
    /// 正在停止
    Stopping,
}

/// 守护进程管理器特征
#[async_trait]
pub trait DaemonManager: Send + Sync {
    /// 安装服务
    async fn install(&self, config: &DaemonConfig) -> Result<()>;

    /// 卸载服务
    async fn uninstall(&self, service_name: &str) -> Result<()>;

    /// 启动服务
    async fn start(&self, service_name: &str) -> Result<()>;

    /// 停止服务
    async fn stop(&self, service_name: &str) -> Result<()>;

    /// 重启服务
    async fn restart(&self, service_name: &str) -> Result<()>;

    /// 获取服务状态
    async fn status(&self, service_name: &str) -> Result<DaemonStatus>;

    /// 检查服务是否已安装
    async fn is_installed(&self, service_name: &str) -> Result<bool>;
}

/// Linux/Unix守护进程管理器
#[cfg(unix)]
pub struct PlatformDaemonManager {
    unix_manager: unix::UnixDaemonManager,
}

#[cfg(unix)]
impl PlatformDaemonManager {
    /// 创建Linux/Unix守护进程管理器
    pub fn new() -> Self {
        Self {
            unix_manager: unix::UnixDaemonManager::new(),
        }
    }
}

#[cfg(unix)]
#[async_trait]
impl DaemonManager for PlatformDaemonManager {
    async fn install(&self, config: &DaemonConfig) -> Result<()> {
        self.unix_manager.install(config).await
    }

    async fn uninstall(&self, service_name: &str) -> Result<()> {
        self.unix_manager.uninstall(service_name).await
    }

    async fn start(&self, service_name: &str) -> Result<()> {
        self.unix_manager.start(service_name).await
    }

    async fn stop(&self, service_name: &str) -> Result<()> {
        self.unix_manager.stop(service_name).await
    }

    async fn restart(&self, service_name: &str) -> Result<()> {
        self.unix_manager.restart(service_name).await
    }

    async fn status(&self, service_name: &str) -> Result<DaemonStatus> {
        self.unix_manager.status(service_name).await
    }

    async fn is_installed(&self, service_name: &str) -> Result<bool> {
        self.unix_manager.is_installed(service_name).await
    }
}

/// 获取平台特定的守护进程管理器
#[cfg(unix)]
pub fn get_daemon_manager() -> PlatformDaemonManager {
    PlatformDaemonManager::new()
}

/// 守护进程运行时
pub struct DaemonRuntime {
    /// 配置
    config: DaemonConfig,
    /// 关闭信号发送器
    shutdown_tx: broadcast::Sender<()>,
    /// 关闭信号接收器
    shutdown_rx: broadcast::Receiver<()>,
}

impl DaemonRuntime {
    /// 创建新的守护进程运行时
    pub fn new(config: DaemonConfig) -> Self {
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

        Self {
            config,
            shutdown_tx,
            shutdown_rx,
        }
    }

    /// 获取配置
    pub fn config(&self) -> &DaemonConfig {
        &self.config
    }

    /// 获取关闭信号发送器
    pub fn shutdown_sender(&self) -> broadcast::Sender<()> {
        self.shutdown_tx.clone()
    }

    /// 获取关闭信号接收器
    pub fn shutdown_receiver(&self) -> broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }

    /// 启动守护进程运行时
    pub async fn run<F, Fut>(&mut self, service_main: F) -> Result<()>
    where
        F: FnOnce(broadcast::Receiver<()>) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        // 设置信号处理器
        signal_handler::setup_signal_handlers(self.shutdown_tx.clone()).await?;

        // 创建PID文件（Unix系统）
        #[cfg(unix)]
        if let Some(ref pid_file) = self.config.pid_file {
            self.create_pid_file(pid_file)?;
        }

        // 运行主服务逻辑
        let shutdown_rx = self.shutdown_receiver();
        let result = service_main(shutdown_rx).await;

        // 清理PID文件
        #[cfg(unix)]
        if let Some(ref pid_file) = self.config.pid_file {
            let _ = std::fs::remove_file(pid_file);
        }

        result
    }

    /// 创建PID文件
    #[cfg(unix)]
    fn create_pid_file(&self, pid_file: &PathBuf) -> Result<()> {
        use std::fs;
        use std::io::Write;

        // 确保目录存在
        if let Some(parent) = pid_file.parent() {
            fs::create_dir_all(parent)?;
        }

        // 写入当前进程ID
        let pid = std::process::id();
        let mut file = fs::File::create(pid_file)?;
        writeln!(file, "{}", pid)?;

        Ok(())
    }

    /// 发送关闭信号
    pub fn shutdown(&self) -> Result<()> {
        let _ = self.shutdown_tx.send(());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daemon_config_default() {
        let config = DaemonConfig::default();
        assert_eq!(config.service_name, "service-vitals");
        assert_eq!(config.display_name, "Service Vitals Monitor");
    }

    #[test]
    fn test_daemon_config_for_development() {
        let config = DaemonConfig::for_development();
        assert_eq!(config.config_path, PathBuf::from("./config.toml"));
        assert!(config.user.is_none());
        assert!(config.group.is_none());
    }

    #[tokio::test]
    async fn test_daemon_runtime_creation() {
        let config = DaemonConfig::for_development();
        let runtime = DaemonRuntime::new(config);
        assert_eq!(runtime.config().service_name, "service-vitals");
    }
}
