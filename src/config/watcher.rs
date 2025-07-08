//! 配置文件监控模块
//!
//! 提供配置文件的实时监控和热重载功能

use crate::config::types::Config;
use crate::config::loader::{ConfigLoader, TomlConfigLoader};
use anyhow::{Context, Result};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

/// 配置变更事件
#[derive(Debug, Clone)]
pub struct ConfigChangeEvent {
    /// 配置文件路径
    pub config_path: PathBuf,
    /// 新配置
    pub new_config: Config,
    /// 变更时间
    pub timestamp: Instant,
    /// 配置版本号
    pub version: u64,
}

/// 配置文件监控器
pub struct ConfigWatcher {
    /// 配置文件路径
    config_path: PathBuf,
    /// 文件系统监控器
    watcher: Option<RecommendedWatcher>,
    /// 配置加载器
    loader: TomlConfigLoader,
    /// 事件发送器
    event_sender: broadcast::Sender<ConfigChangeEvent>,
    /// 防抖动延迟
    debounce_delay: Duration,
    /// 当前配置版本
    current_version: u64,
    /// 最后变更时间
    last_change_time: Option<Instant>,
}

impl ConfigWatcher {
    /// 创建新的配置监控器
    ///
    /// # 参数
    /// * `config_path` - 配置文件路径
    /// * `debounce_delay` - 防抖动延迟时间
    ///
    /// # 返回
    /// * `Result<(Self, broadcast::Receiver<ConfigChangeEvent>)>` - 监控器和事件接收器
    pub fn new<P: AsRef<Path>>(
        config_path: P,
        debounce_delay: Duration,
    ) -> Result<(Self, broadcast::Receiver<ConfigChangeEvent>)> {
        let config_path = config_path.as_ref().to_path_buf();
        
        // 验证配置文件路径
        Self::validate_config_path(&config_path)?;
        
        let loader = TomlConfigLoader::new(true);
        let (event_sender, event_receiver) = broadcast::channel(32);
        
        let watcher = Self {
            config_path,
            watcher: None,
            loader,
            event_sender,
            debounce_delay,
            current_version: 0,
            last_change_time: None,
        };
        
        Ok((watcher, event_receiver))
    }

    /// 验证配置文件路径
    fn validate_config_path(path: &Path) -> Result<()> {
        // 检查文件是否存在
        if !path.exists() {
            return Err(anyhow::anyhow!("配置文件不存在: {}", path.display()));
        }
        
        // 检查是否为文件
        if !path.is_file() {
            return Err(anyhow::anyhow!("路径不是文件: {}", path.display()));
        }
        
        // 检查文件扩展名
        if let Some(extension) = path.extension() {
            if extension != "toml" {
                warn!("配置文件扩展名不是.toml: {}", path.display());
            }
        }
        
        // 检查文件权限（读取权限）
        match std::fs::File::open(path) {
            Ok(_) => {
                debug!("配置文件权限验证通过: {}", path.display());
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!(
                "无法读取配置文件 {}: {}",
                path.display(),
                e
            )),
        }
    }

    /// 启动配置文件监控
    ///
    /// # 返回
    /// * `Result<()>` - 启动结果
    pub fn start(&mut self) -> Result<()> {
        info!("启动配置文件监控: {}", self.config_path.display());
        
        let (tx, rx) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(
            tx,
            notify::Config::default().with_poll_interval(Duration::from_secs(1)),
        )
        .context("创建文件监控器失败")?;
        
        // 监控配置文件所在目录
        let watch_path = self.config_path.parent().unwrap_or(&self.config_path);
        watcher
            .watch(watch_path, RecursiveMode::NonRecursive)
            .with_context(|| format!("监控目录失败: {}", watch_path.display()))?;
        
        self.watcher = Some(watcher);
        
        // 启动事件处理任务
        let config_path = self.config_path.clone();
        let event_sender = self.event_sender.clone();
        let loader = self.loader.clone();
        let debounce_delay = self.debounce_delay;
        
        tokio::spawn(async move {
            Self::handle_file_events(rx, config_path, event_sender, loader, debounce_delay).await;
        });
        
        info!("配置文件监控已启动");
        Ok(())
    }

    /// 处理文件系统事件
    async fn handle_file_events(
        rx: mpsc::Receiver<notify::Result<Event>>,
        config_path: PathBuf,
        event_sender: broadcast::Sender<ConfigChangeEvent>,
        loader: TomlConfigLoader,
        debounce_delay: Duration,
    ) {
        let mut last_event_time: Option<Instant> = None;
        let mut version = 1u64;
        
        for res in rx {
            match res {
                Ok(event) => {
                    // 检查是否是我们关心的文件
                    if !Self::is_target_file_event(&event, &config_path) {
                        continue;
                    }
                    
                    debug!("检测到配置文件变更事件: {:?}", event);
                    
                    // 防抖动处理
                    let now = Instant::now();
                    if let Some(last_time) = last_event_time {
                        if now.duration_since(last_time) < debounce_delay {
                            debug!("跳过重复事件（防抖动）");
                            continue;
                        }
                    }
                    last_event_time = Some(now);
                    
                    // 延迟处理，确保文件写入完成
                    tokio::time::sleep(debounce_delay).await;
                    
                    // 重新加载配置
                    match Self::reload_config(&loader, &config_path, version).await {
                        Ok(change_event) => {
                            info!("配置重载成功，版本: {}", version);
                            version += 1;
                            
                            if let Err(e) = event_sender.send(change_event) {
                                error!("发送配置变更事件失败: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("配置重载失败: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("文件监控事件错误: {}", e);
                }
            }
        }
    }

    /// 检查是否是目标文件的事件
    fn is_target_file_event(event: &Event, target_path: &Path) -> bool {
        match &event.kind {
            EventKind::Modify(_) | EventKind::Create(_) => {
                event.paths.iter().any(|path| path == target_path)
            }
            _ => false,
        }
    }

    /// 重新加载配置
    async fn reload_config(
        loader: &TomlConfigLoader,
        config_path: &Path,
        version: u64,
    ) -> Result<ConfigChangeEvent> {
        debug!("重新加载配置文件: {}", config_path.display());
        
        let new_config = loader
            .load_from_file(config_path)
            .await
            .context("重新加载配置失败")?;
        
        Ok(ConfigChangeEvent {
            config_path: config_path.to_path_buf(),
            new_config,
            timestamp: Instant::now(),
            version,
        })
    }

    /// 停止监控
    pub fn stop(&mut self) {
        if let Some(watcher) = self.watcher.take() {
            drop(watcher);
            info!("配置文件监控已停止");
        }
    }

    /// 获取事件发送器的克隆
    pub fn get_event_sender(&self) -> broadcast::Sender<ConfigChangeEvent> {
        self.event_sender.clone()
    }
}

impl Drop for ConfigWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_config_watcher_creation() {
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(temp_file.path(), "[global]\nlog_level = \"info\"").unwrap();
        
        let result = ConfigWatcher::new(temp_file.path(), Duration::from_millis(100));
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_config_file_validation() {
        // 测试不存在的文件
        let result = ConfigWatcher::validate_config_path(Path::new("/nonexistent/file.toml"));
        assert!(result.is_err());
        
        // 测试有效文件
        let temp_file = NamedTempFile::new().unwrap();
        let result = ConfigWatcher::validate_config_path(temp_file.path());
        assert!(result.is_ok());
    }
}
