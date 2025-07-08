//! 任务调度器模块
//!
//! 提供健康检测任务的调度、管理和并发控制功能

use crate::config::types::{GlobalConfig, ServiceConfig};
use crate::health::HealthChecker;
use crate::notification::NotificationSender;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::{interval, Instant};
use tracing::{debug, error, info, warn};

/// 调度器状态
#[derive(Debug, Clone)]
pub struct SchedulerStatus {
    /// 运行中的任务数量
    pub running_tasks: usize,
    /// 总服务数量
    pub total_services: usize,
    /// 调度器是否运行中
    pub is_running: bool,
    /// 最后更新时间
    pub last_update: Instant,
}

/// 任务调度器trait，定义调度接口
#[async_trait]
pub trait Scheduler: Send + Sync {
    /// 启动调度器
    ///
    /// # 参数
    /// * `services` - 服务配置列表
    ///
    /// # 返回
    /// * `Result<()>` - 启动结果
    async fn start(&self, services: Vec<ServiceConfig>) -> Result<()>;

    /// 停止调度器
    ///
    /// # 返回
    /// * `Result<()>` - 停止结果
    async fn stop(&self) -> Result<()>;

    /// 重新加载配置
    ///
    /// # 参数
    /// * `services` - 新的服务配置列表
    ///
    /// # 返回
    /// * `Result<()>` - 重载结果
    async fn reload_config(&self, services: Vec<ServiceConfig>) -> Result<()>;

    /// 获取调度器状态
    ///
    /// # 返回
    /// * `SchedulerStatus` - 当前状态
    async fn get_status(&self) -> SchedulerStatus;
}

/// 任务调度器实现
pub struct TaskScheduler {
    /// 健康检测器
    checker: Arc<dyn HealthChecker>,
    /// 通知发送器
    notifier: Option<Arc<dyn NotificationSender>>,
    /// 运行中的任务
    tasks: Arc<RwLock<HashMap<String, JoinHandle<()>>>>,
    /// 全局配置
    config: Arc<RwLock<GlobalConfig>>,
    /// 并发控制信号量
    semaphore: Arc<Semaphore>,
    /// 调度器状态
    status: Arc<RwLock<SchedulerStatus>>,
}

impl TaskScheduler {
    /// 创建新的任务调度器
    ///
    /// # 参数
    /// * `checker` - 健康检测器
    /// * `notifier` - 通知发送器（可选）
    /// * `config` - 全局配置
    ///
    /// # 返回
    /// * `Self` - 调度器实例
    pub fn new(
        checker: Arc<dyn HealthChecker>,
        notifier: Option<Arc<dyn NotificationSender>>,
        config: GlobalConfig,
    ) -> Self {
        let max_concurrent = config.max_concurrent_checks;
        let status = SchedulerStatus {
            running_tasks: 0,
            total_services: 0,
            is_running: false,
            last_update: Instant::now(),
        };

        Self {
            checker,
            notifier,
            tasks: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(config)),
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            status: Arc::new(RwLock::new(status)),
        }
    }

    /// 启动单个服务的检测任务
    async fn start_service_task(&self, service: ServiceConfig) -> Result<()> {
        let service_name = service.name.clone();
        let service_name_for_task = service_name.clone();
        let checker = Arc::clone(&self.checker);
        let notifier = self.notifier.clone();
        let config = Arc::clone(&self.config);
        let semaphore = Arc::clone(&self.semaphore);

        // 计算检测间隔
        let check_interval = service.check_interval_seconds.unwrap_or_else(|| {
            let config = config.try_read().unwrap();
            config.check_interval_seconds
        });

        // 创建检测任务
        let task = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(check_interval));
            let mut consecutive_failures = 0u32;

            info!("启动服务检测任务: {}", service_name_for_task);

            loop {
                interval.tick().await;

                // 获取信号量许可
                let _permit = match semaphore.acquire().await {
                    Ok(permit) => permit,
                    Err(_) => {
                        warn!("获取并发许可失败，跳过本次检测: {}", service_name_for_task);
                        continue;
                    }
                };

                debug!("开始检测服务: {}", service_name_for_task);

                // 执行健康检测
                let result = match checker.check(&service).await {
                    Ok(result) => result,
                    Err(e) => {
                        error!("检测服务失败 {}: {}", service_name_for_task, e);
                        continue;
                    }
                };

                // 处理检测结果
                if result.status.is_healthy() {
                    if consecutive_failures > 0 {
                        info!(
                            "服务恢复正常: {} (之前连续失败{}次)",
                            service_name_for_task, consecutive_failures
                        );
                        consecutive_failures = 0;
                    } else {
                        debug!("服务检测正常: {}", service_name_for_task);
                    }
                } else {
                    consecutive_failures += 1;
                    warn!(
                        "服务检测失败: {} (连续失败{}次)",
                        service_name_for_task, consecutive_failures
                    );

                    // 检查是否达到告警阈值
                    if consecutive_failures >= service.failure_threshold {
                        if let Some(ref notifier) = notifier {
                            if let Err(e) = notifier.send_health_alert(&service, &result).await {
                                error!("发送告警通知失败: {}", e);
                            }
                        }
                    }
                }
            }
        });

        // 保存任务句柄
        let mut tasks = self.tasks.write().await;
        tasks.insert(service_name, task);

        Ok(())
    }

    /// 停止单个服务的检测任务
    async fn stop_service_task(&self, service_name: &str) -> Result<()> {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.remove(service_name) {
            task.abort();
            info!("停止服务检测任务: {}", service_name);
        }
        Ok(())
    }

    /// 更新调度器状态
    async fn update_status(&self) {
        let tasks = self.tasks.read().await;
        let mut status = self.status.write().await;

        status.running_tasks = tasks.len();
        status.last_update = Instant::now();
    }
}

#[async_trait]
impl Scheduler for TaskScheduler {
    async fn start(&self, services: Vec<ServiceConfig>) -> Result<()> {
        info!("启动任务调度器，服务数量: {}", services.len());

        // 更新状态
        {
            let mut status = self.status.write().await;
            status.total_services = services.len();
            status.is_running = true;
        }

        // 启动所有服务的检测任务
        for service in services {
            if service.enabled {
                let service_name = service.name.clone();
                self.start_service_task(service)
                    .await
                    .with_context(|| format!("启动服务任务失败: {}", service_name))?;
            } else {
                debug!("跳过已禁用的服务: {}", service.name);
            }
        }

        self.update_status().await;
        info!("任务调度器启动完成");
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        info!("停止任务调度器");

        // 停止所有任务
        let mut tasks = self.tasks.write().await;
        for (service_name, task) in tasks.drain() {
            task.abort();
            debug!("停止任务: {}", service_name);
        }

        // 更新状态
        {
            let mut status = self.status.write().await;
            status.is_running = false;
            status.running_tasks = 0;
        }

        info!("任务调度器已停止");
        Ok(())
    }

    async fn reload_config(&self, services: Vec<ServiceConfig>) -> Result<()> {
        info!("重新加载配置，服务数量: {}", services.len());

        // 获取当前运行的任务列表
        let current_tasks: Vec<String> = {
            let tasks = self.tasks.read().await;
            tasks.keys().cloned().collect()
        };

        // 获取新配置中的服务列表
        let new_services: HashMap<String, ServiceConfig> = services
            .into_iter()
            .filter(|s| s.enabled)
            .map(|s| (s.name.clone(), s))
            .collect();

        // 停止不再需要的任务
        for service_name in &current_tasks {
            if !new_services.contains_key(service_name) {
                self.stop_service_task(service_name).await?;
            }
        }

        // 启动新的或更新的任务
        for (service_name, service) in new_services {
            if current_tasks.contains(&service_name) {
                // 重启已存在的任务（配置可能已更改）
                self.stop_service_task(&service_name).await?;
            }
            self.start_service_task(service).await?;
        }

        self.update_status().await;
        info!("配置重新加载完成");
        Ok(())
    }

    async fn get_status(&self) -> SchedulerStatus {
        self.status.read().await.clone()
    }
}

impl Drop for TaskScheduler {
    fn drop(&mut self) {
        // 确保在调度器被销毁时停止所有任务
        // 注意：这里不能使用async，所以只能发出停止信号
        let tasks = self.tasks.clone();
        tokio::spawn(async move {
            let mut tasks = tasks.write().await;
            for (_, task) in tasks.drain() {
                task.abort();
            }
        });
    }
}
