//! 配置管理器模块
//!
//! 提供线程安全的配置管理和热重载功能

use crate::config::types::{Config, ServiceConfig};
use crate::config::watcher::{ConfigChangeEvent, ConfigWatcher};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, error, info, warn};

/// 配置差异类型
#[derive(Debug, Clone)]
pub enum ConfigDiff {
    /// 服务添加
    ServiceAdded(ServiceConfig),
    /// 服务移除
    ServiceRemoved(String),
    /// 服务修改
    ServiceModified {
        old: Box<ServiceConfig>,
        new: Box<ServiceConfig>,
    },
    /// 全局配置修改
    GlobalConfigModified,
}

/// 配置变更通知
#[derive(Debug, Clone)]
pub struct ConfigUpdateNotification {
    /// 配置版本号
    pub version: u64,
    /// 配置差异列表
    pub diffs: Vec<ConfigDiff>,
    /// 变更时间
    pub timestamp: Instant,
    /// 是否需要重启服务
    pub requires_restart: bool,
}

/// 配置管理器
pub struct ConfigManager {
    /// 当前配置
    current_config: Arc<RwLock<Config>>,
    /// 配置版本号
    version: Arc<RwLock<u64>>,
    /// 配置文件监控器
    watcher: Option<ConfigWatcher>,
    /// 配置更新通知发送器
    update_sender: broadcast::Sender<ConfigUpdateNotification>,
    /// 配置变更事件接收器
    change_receiver: Option<broadcast::Receiver<ConfigChangeEvent>>,
    /// 最后更新时间
    last_update: Arc<RwLock<Instant>>,
}

impl ConfigManager {
    /// 创建新的配置管理器
    ///
    /// # 参数
    /// * `initial_config` - 初始配置
    ///
    /// # 返回
    /// * `(Self, broadcast::Receiver<ConfigUpdateNotification>)` - 管理器和更新通知接收器
    pub fn new(initial_config: Config) -> (Self, broadcast::Receiver<ConfigUpdateNotification>) {
        let (update_sender, update_receiver) = broadcast::channel(32);

        let manager = Self {
            current_config: Arc::new(RwLock::new(initial_config)),
            version: Arc::new(RwLock::new(1)),
            watcher: None,
            update_sender,
            change_receiver: None,
            last_update: Arc::new(RwLock::new(Instant::now())),
        };

        (manager, update_receiver)
    }

    /// 启用配置文件监控
    ///
    /// # 参数
    /// * `config_path` - 配置文件路径
    /// * `debounce_delay` - 防抖动延迟
    ///
    /// # 返回
    /// * `Result<()>` - 启动结果
    pub async fn enable_hot_reload<P: AsRef<Path>>(
        &mut self,
        config_path: P,
        debounce_delay: Duration,
    ) -> Result<()> {
        info!("启用配置热重载功能");

        let (mut watcher, change_receiver) =
            ConfigWatcher::new(config_path, debounce_delay).context("创建配置监控器失败")?;

        watcher.start().context("启动配置监控失败")?;

        self.watcher = Some(watcher);
        self.change_receiver = Some(change_receiver);

        // 启动配置变更处理任务
        self.start_change_handler().await;

        info!("配置热重载功能已启用");
        Ok(())
    }

    /// 启动配置变更处理任务
    async fn start_change_handler(&mut self) {
        if let Some(mut receiver) = self.change_receiver.take() {
            let current_config = Arc::clone(&self.current_config);
            let version = Arc::clone(&self.version);
            let update_sender = self.update_sender.clone();
            let last_update = Arc::clone(&self.last_update);

            tokio::spawn(async move {
                while let Ok(change_event) = receiver.recv().await {
                    if let Err(e) = Self::handle_config_change(
                        change_event,
                        &current_config,
                        &version,
                        &update_sender,
                        &last_update,
                    )
                    .await
                    {
                        error!("处理配置变更失败: {}", e);
                    }
                }
            });
        }
    }

    /// 处理配置变更事件
    async fn handle_config_change(
        change_event: ConfigChangeEvent,
        current_config: &Arc<RwLock<Config>>,
        version: &Arc<RwLock<u64>>,
        update_sender: &broadcast::Sender<ConfigUpdateNotification>,
        last_update: &Arc<RwLock<Instant>>,
    ) -> Result<()> {
        info!("处理配置变更，版本: {}", change_event.version);

        // 计算配置差异
        let old_config = current_config.read().await.clone();
        let diffs = Self::calculate_config_diff(&old_config, &change_event.new_config);

        if diffs.is_empty() {
            debug!("配置无实质性变更，跳过更新");
            return Ok(());
        }

        // 检查是否需要重启服务
        let requires_restart = Self::requires_service_restart(&diffs);

        // 更新配置
        {
            let mut config = current_config.write().await;
            *config = change_event.new_config;
        }

        // 更新版本号
        {
            let mut ver = version.write().await;
            *ver = change_event.version;
        }

        // 更新最后更新时间
        {
            let mut last = last_update.write().await;
            *last = change_event.timestamp;
        }

        // 发送更新通知
        let notification = ConfigUpdateNotification {
            version: change_event.version,
            diffs,
            timestamp: change_event.timestamp,
            requires_restart,
        };

        if let Err(e) = update_sender.send(notification) {
            warn!("发送配置更新通知失败: {}", e);
        }

        info!("配置更新完成，版本: {}", change_event.version);
        Ok(())
    }

    /// 计算配置差异
    fn calculate_config_diff(old_config: &Config, new_config: &Config) -> Vec<ConfigDiff> {
        let mut diffs = Vec::new();

        // 检查全局配置变更
        if old_config.global != new_config.global {
            diffs.push(ConfigDiff::GlobalConfigModified);
        }

        // 创建服务映射以便比较
        let old_services: HashMap<String, &ServiceConfig> = old_config
            .services
            .iter()
            .map(|s| (s.name.clone(), s))
            .collect();

        let new_services: HashMap<String, &ServiceConfig> = new_config
            .services
            .iter()
            .map(|s| (s.name.clone(), s))
            .collect();

        // 检查新增和修改的服务
        for (name, new_service) in &new_services {
            match old_services.get(name) {
                Some(old_service) => {
                    // 服务存在，检查是否有修改
                    if **old_service != **new_service {
                        diffs.push(ConfigDiff::ServiceModified {
                            old: Box::new((*old_service).clone()),
                            new: Box::new((*new_service).clone()),
                        });
                    }
                }
                None => {
                    // 新增服务
                    diffs.push(ConfigDiff::ServiceAdded((*new_service).clone()));
                }
            }
        }

        // 检查删除的服务
        for name in old_services.keys() {
            if !new_services.contains_key(name) {
                diffs.push(ConfigDiff::ServiceRemoved(name.clone()));
            }
        }

        diffs
    }

    /// 检查是否需要重启服务
    fn requires_service_restart(diffs: &[ConfigDiff]) -> bool {
        diffs.iter().any(|diff| match diff {
            ConfigDiff::GlobalConfigModified => true,
            ConfigDiff::ServiceAdded(_) => false,
            ConfigDiff::ServiceRemoved(_) => false,
            ConfigDiff::ServiceModified { old, new } => {
                // 检查关键配置是否变更
                old.url != new.url
                    || old.method != new.method
                    || old.expected_status_codes != new.expected_status_codes
                    || old.check_interval_seconds != new.check_interval_seconds
            }
        })
    }

    /// 获取当前配置
    pub async fn get_config(&self) -> Config {
        self.current_config.read().await.clone()
    }

    /// 获取当前版本号
    pub async fn get_version(&self) -> u64 {
        *self.version.read().await
    }

    /// 获取最后更新时间
    pub async fn get_last_update(&self) -> Instant {
        *self.last_update.read().await
    }

    /// 手动更新配置
    ///
    /// # 参数
    /// * `new_config` - 新配置
    ///
    /// # 返回
    /// * `Result<u64>` - 新版本号
    pub async fn update_config(&self, new_config: Config) -> Result<u64> {
        info!("手动更新配置");

        let old_config = self.current_config.read().await.clone();
        let diffs = Self::calculate_config_diff(&old_config, &new_config);

        if diffs.is_empty() {
            debug!("配置无变更");
            return Ok(*self.version.read().await);
        }

        // 更新配置
        {
            let mut config = self.current_config.write().await;
            *config = new_config;
        }

        // 更新版本号
        let new_version = {
            let mut ver = self.version.write().await;
            *ver += 1;
            *ver
        };

        // 更新最后更新时间
        let now = Instant::now();
        {
            let mut last = self.last_update.write().await;
            *last = now;
        }

        // 发送更新通知
        let requires_restart = Self::requires_service_restart(&diffs);
        let notification = ConfigUpdateNotification {
            version: new_version,
            diffs,
            timestamp: now,
            requires_restart,
        };

        if let Err(e) = self.update_sender.send(notification) {
            warn!("发送配置更新通知失败: {}", e);
        }

        info!("配置手动更新完成，版本: {}", new_version);
        Ok(new_version)
    }

    /// 获取配置更新通知发送器
    pub fn get_update_sender(&self) -> broadcast::Sender<ConfigUpdateNotification> {
        self.update_sender.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::GlobalConfig;
    use std::collections::HashMap;

    fn create_test_config() -> Config {
        Config {
            global: GlobalConfig {
                default_feishu_webhook_url: None,
                message_template: None,
                check_interval_seconds: 60,
                log_level: "info".to_string(),
                request_timeout_seconds: 10,
                max_concurrent_checks: 50,
                retry_attempts: 3,
                retry_delay_seconds: 5,
                headers: HashMap::new(),
            },
            services: vec![],
        }
    }

    #[tokio::test]
    async fn test_config_manager_creation() {
        let config = create_test_config();
        let (manager, _receiver) = ConfigManager::new(config);

        let current_config = manager.get_config().await;
        assert_eq!(current_config.global.log_level, "info");
    }

    #[tokio::test]
    async fn test_manual_config_update() {
        let config = create_test_config();
        let (manager, _receiver) = ConfigManager::new(config);

        let mut new_config = create_test_config();
        new_config.global.log_level = "debug".to_string();

        let new_version = manager.update_config(new_config).await.unwrap();
        assert_eq!(new_version, 2);

        let updated_config = manager.get_config().await;
        assert_eq!(updated_config.global.log_level, "debug");
    }
}
