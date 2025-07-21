//! 任务调度器模块
//!
//! 提供健康检测任务的调度、管理和并发控制功能

use crate::config::types::{GlobalConfig, ServiceConfig};
use crate::config::{ConfigDiff, ConfigUpdateNotification};
use crate::health::{HealthChecker, HealthResult, HealthStatus};
use crate::notification::NotificationSender;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock, Semaphore};
use tokio::task::JoinHandle;
use tokio::time::{interval, Instant};
use tracing::{debug, error, info, warn};

/// 健康检测结果回调函数类型
pub type HealthResultCallback = Arc<dyn Fn(&HealthResult) + Send + Sync>;

/// 服务通知状态
#[derive(Debug, Clone, Default)]
pub struct ServiceNotificationState {
    /// 上次健康状态
    pub last_health_status: Option<HealthStatus>,
    /// 上次通知时间
    pub last_notification_time: Option<Instant>,
    /// 连续失败次数
    pub consecutive_failures: u32,
    /// 通知发送次数统计
    pub notification_count: u32,
    /// 下次告警的失败次数
    pub next_alert_threshold: u32,
    /// 下次可告警的最早时间（仅用于非首次失败通知）
    /// 首次达到失败阈值时会立即通知，不受此限制
    pub alert_cooldown_until: Option<Instant>,
}

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
    /// 通知统计
    pub notification_stats: NotificationStats,
}

/// 通知统计信息
#[derive(Debug, Clone, Default)]
pub struct NotificationStats {
    /// 总通知发送次数
    pub total_sent: u32,
    /// 通知发送成功次数
    pub successful_sent: u32,
    /// 通知发送失败次数
    pub failed_sent: u32,
    /// 最后通知时间
    pub last_notification_time: Option<Instant>,
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
    /// 服务通知状态
    notification_states: Arc<RwLock<HashMap<String, ServiceNotificationState>>>,
    /// 配置更新接收器
    config_update_receiver: Option<broadcast::Receiver<ConfigUpdateNotification>>,
    /// 健康检测结果回调
    health_result_callback: Arc<RwLock<Option<HealthResultCallback>>>,
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
            notification_stats: NotificationStats::default(),
        };

        Self {
            checker,
            notifier,
            tasks: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(config)),
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            status: Arc::new(RwLock::new(status)),
            notification_states: Arc::new(RwLock::new(HashMap::new())),
            config_update_receiver: None,
            health_result_callback: Arc::new(RwLock::new(None)),
        }
    }

    /// 设置健康检测结果回调
    ///
    /// # 参数
    /// * `callback` - 健康检测结果回调函数
    pub async fn set_health_result_callback(&self, callback: HealthResultCallback) {
        let mut cb = self.health_result_callback.write().await;
        *cb = Some(callback);
    }

    /// 静态方法处理通知逻辑
    async fn handle_notification_static(
        service: &ServiceConfig,
        result: &crate::health::HealthResult,
        notification_state: &mut ServiceNotificationState,
        notifier: &Option<Arc<dyn NotificationSender>>,
        status_arc: &Arc<RwLock<SchedulerStatus>>,
    ) -> Result<()> {
        let current_status = result.status;
        let now = Instant::now();
        let is_healthy = current_status.is_healthy();

        // 1. 检查是否需要发送恢复通知
        let need_recover_notify = is_healthy && notification_state.consecutive_failures > 0;
        if need_recover_notify {
            if let Some(ref notifier) = notifier {
                let send_result = notifier.send_health_alert(service, result).await;
                match send_result {
                    Ok(()) => {
                        info!("发送服务恢复通知成功: {}", service.name);
                        Self::update_notification_stats_static(status_arc, true).await;
                    }
                    Err(e) => {
                        error!("发送服务恢复通知失败: {} - {}", service.name, e);
                        Self::update_notification_stats_static(status_arc, false).await;
                    }
                }
            }
            notification_state.consecutive_failures = 0;
            // 恢复时重置告警冷却时间
            notification_state.alert_cooldown_until = None;
        }

        // 2. 检查是否需要发送告警通知
        // 新逻辑：第一次失败立即发送通知，后续失败受冷却时间限制
        // 这样可以确保用户第一时间知道服务异常，同时避免通知轰炸
        if !is_healthy {
            notification_state.consecutive_failures += 1;
            if notification_state.consecutive_failures >= service.failure_threshold {
                let cooldown_secs = service.alert_cooldown_secs.unwrap_or(60); // 默认60秒
                
                // 判断是否为第一次达到失败阈值
                let is_first_threshold_failure = notification_state.consecutive_failures == service.failure_threshold;
                
                // 第一次达到阈值时立即通知，后续失败受冷却时间限制
                let can_alert = is_first_threshold_failure || 
                    notification_state
                        .alert_cooldown_until
                        .is_none_or(|until| now >= until);
                
                if can_alert {
                    if let Some(ref notifier) = notifier {
                        let send_result = notifier.send_health_alert(service, result).await;
                        match send_result {
                            Ok(()) => {
                                info!("发送服务告警通知成功: {}", service.name);
                                notification_state.notification_count += 1;
                                notification_state.last_notification_time = Some(now);
                                Self::update_notification_stats_static(status_arc, true).await;
                            }
                            Err(e) => {
                                error!("发送服务告警通知失败: {} - {}", service.name, e);
                                Self::update_notification_stats_static(status_arc, false).await;
                            }
                        }
                    }
                    
                    // 只有非首次达到阈值时才设置冷却时间
                    // 这样第一次失败会立即通知，后续失败受冷却时间限制
                    if !is_first_threshold_failure {
                        notification_state.alert_cooldown_until =
                            Some(now + Duration::from_secs(cooldown_secs));
                    }
                }
            }
        }

        // 3. 更新最后健康状态
        notification_state.last_health_status = Some(current_status);

        Ok(())
    }

    /// 静态方法更新通知统计
    async fn update_notification_stats_static(
        status_arc: &Arc<RwLock<SchedulerStatus>>,
        success: bool,
    ) {
        let mut status = status_arc.write().await;
        status.notification_stats.total_sent += 1;
        if success {
            status.notification_stats.successful_sent += 1;
        } else {
            status.notification_stats.failed_sent += 1;
        }
        status.notification_stats.last_notification_time = Some(Instant::now());
    }

    /// 启动单个服务的检测任务
    async fn start_service_task(&self, service: ServiceConfig) -> Result<()> {
        let service_name = service.name.clone();
        let service_name_for_task = service_name.clone();
        let checker = Arc::clone(&self.checker);
        let config = Arc::clone(&self.config);
        let semaphore = Arc::clone(&self.semaphore);
        let notification_states = Arc::clone(&self.notification_states);

        // 计算检测间隔
        let check_interval = service.check_interval_seconds.unwrap_or_else(|| {
            let config = config.try_read().unwrap();
            config.check_interval_seconds
        });

        // 初始化通知状态
        {
            let mut states = notification_states.write().await;
            states.insert(service_name.clone(), ServiceNotificationState::default());
        }

        // 创建检测任务
        let notifier = self.notifier.clone();
        let status_arc = Arc::clone(&self.status);
        let health_callback = self.health_result_callback.clone();
        let task = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(check_interval));

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

                // 处理通知逻辑
                {
                    let mut states = notification_states.write().await;
                    if let Some(notification_state) = states.get_mut(&service_name_for_task) {
                        if let Err(e) = Self::handle_notification_static(
                            &service,
                            &result,
                            notification_state,
                            &notifier,
                            &status_arc,
                        )
                        .await
                        {
                            error!("处理通知失败: {}", e);
                        }
                    }
                }

                // 调用健康检测结果回调
                {
                    let callback_guard = health_callback.read().await;
                    if let Some(ref callback) = *callback_guard {
                        callback(&result);
                    }
                }

                // 记录检测结果
                if result.status.is_healthy() {
                    debug!("服务检测正常: {}", service_name_for_task);
                } else {
                    warn!(
                        "服务检测失败: {},{}",
                        service_name_for_task,
                        result.error_message.unwrap_or_else(|| "N/A".to_string())
                    );
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

    /// 启用配置热重载
    ///
    /// # 参数
    /// * `config_update_receiver` - 配置更新通知接收器
    pub fn enable_hot_reload(
        &mut self,
        config_update_receiver: broadcast::Receiver<ConfigUpdateNotification>,
    ) {
        info!("启用任务调度器配置热重载");
        self.config_update_receiver = Some(config_update_receiver);
    }

    /// 启动配置更新监听器
    pub async fn start_config_update_listener(&mut self) {
        if let Some(mut receiver) = self.config_update_receiver.take() {
            let tasks = Arc::clone(&self.tasks);
            let config = Arc::clone(&self.config);
            let status = Arc::clone(&self.status);
            let checker = Arc::clone(&self.checker);
            let notifier = self.notifier.clone();
            let semaphore = Arc::clone(&self.semaphore);
            let notification_states = Arc::clone(&self.notification_states);

            tokio::spawn(async move {
                info!("配置更新监听器已启动");
                while let Ok(update) = receiver.recv().await {
                    info!("收到配置更新通知，版本: {}", update.version);

                    if let Err(e) = TaskScheduler::handle_config_update(
                        update,
                        &tasks,
                        &config,
                        &status,
                        &checker,
                        &notifier,
                        &semaphore,
                        &notification_states,
                    )
                    .await
                    {
                        error!("处理配置更新失败: {}", e);
                    }
                }
                info!("配置更新监听器已停止");
            });
        }
    }

    /// 处理配置更新
    #[allow(clippy::too_many_arguments)]
    async fn handle_config_update(
        update: ConfigUpdateNotification,
        tasks: &Arc<RwLock<HashMap<String, JoinHandle<()>>>>,
        config: &Arc<RwLock<GlobalConfig>>,
        status: &Arc<RwLock<SchedulerStatus>>,
        checker: &Arc<dyn HealthChecker>,
        notifier: &Option<Arc<dyn NotificationSender>>,
        semaphore: &Arc<Semaphore>,
        notification_states: &Arc<RwLock<HashMap<String, ServiceNotificationState>>>,
    ) -> Result<()> {
        info!(
            "处理配置更新，版本: {}, 变更数量: {}",
            update.version,
            update.diffs.len()
        );

        for diff in &update.diffs {
            match diff {
                ConfigDiff::GlobalConfigModified => {
                    info!("全局配置已修改");
                    // 全局配置修改可能需要更新并发限制等
                    // 这里可以根据需要实现具体的更新逻辑
                }
                ConfigDiff::ServiceAdded(service) => {
                    info!("添加新服务: {}", service.name);
                    if let Err(e) = TaskScheduler::start_new_service_task(
                        (**service).clone(),
                        tasks,
                        checker,
                        notifier,
                        config,
                        semaphore,
                        notification_states,
                        status,
                    )
                    .await
                    {
                        error!("启动新服务任务失败: {}", e);
                    }
                }
                ConfigDiff::ServiceRemoved(service_name) => {
                    info!("移除服务: {}", service_name);
                    TaskScheduler::stop_service_task_by_name(
                        service_name,
                        tasks,
                        notification_states,
                    )
                    .await;
                }
                ConfigDiff::ServiceModified { old: _, new } => {
                    info!("修改服务: {}", new.name);
                    // 先停止旧任务
                    TaskScheduler::stop_service_task_by_name(&new.name, tasks, notification_states)
                        .await;
                    // 启动新任务
                    if let Err(e) = TaskScheduler::start_new_service_task(
                        (**new).clone(),
                        tasks,
                        checker,
                        notifier,
                        config,
                        semaphore,
                        notification_states,
                        status,
                    )
                    .await
                    {
                        error!("重启修改的服务任务失败: {}", e);
                    }
                }
            }
        }

        // 更新状态统计
        {
            let mut status_guard = status.write().await;
            let tasks_guard = tasks.read().await;
            status_guard.running_tasks = tasks_guard.len();
            status_guard.last_update = Instant::now();
        }

        info!("配置更新处理完成");
        Ok(())
    }

    /// 启动新服务任务
    #[allow(clippy::too_many_arguments)]
    async fn start_new_service_task(
        service: ServiceConfig,
        tasks: &Arc<RwLock<HashMap<String, JoinHandle<()>>>>,
        checker: &Arc<dyn HealthChecker>,
        notifier: &Option<Arc<dyn NotificationSender>>,
        _config: &Arc<RwLock<GlobalConfig>>,
        semaphore: &Arc<Semaphore>,
        notification_states: &Arc<RwLock<HashMap<String, ServiceNotificationState>>>,
        status: &Arc<RwLock<SchedulerStatus>>,
    ) -> Result<()> {
        let service_name = service.name.clone();
        let service_name_for_task = service_name.clone();
        let checker = Arc::clone(checker);
        let notifier = notifier.clone();
        let semaphore = Arc::clone(semaphore);
        let notification_states = Arc::clone(notification_states);
        let status_arc = Arc::clone(status);

        // 计算检测间隔
        let check_interval = service.check_interval_seconds.unwrap_or({
            // 从全局配置获取默认值
            60 // 默认60秒，实际应该从config中读取
        });

        // 初始化通知状态
        {
            let mut states = notification_states.write().await;
            states.insert(service_name.clone(), ServiceNotificationState::default());
        }

        // 创建检测任务
        let task = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(check_interval));
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

                // 处理通知逻辑
                {
                    let mut states = notification_states.write().await;
                    if let Some(notification_state) = states.get_mut(&service_name_for_task) {
                        if let Err(e) = TaskScheduler::handle_notification_static(
                            &service,
                            &result,
                            notification_state,
                            &notifier,
                            &status_arc,
                        )
                        .await
                        {
                            error!("处理通知失败: {}", e);
                        }
                    }
                }

                // 记录检测结果
                if result.status.is_healthy() {
                    debug!("服务检测正常: {}", service_name_for_task);
                } else {
                    warn!(
                        "服务检测失败: {},{}",
                        service_name_for_task,
                        result.error_message.unwrap_or_else(|| "N/A".to_string())
                    );
                }
            }
        });

        // 将任务添加到tasks映射中
        {
            let mut task_map = tasks.write().await;
            task_map.insert(service_name.clone(), task);
        }

        info!("服务任务已启动: {}", service_name);
        Ok(())
    }

    /// 按名称停止服务任务
    async fn stop_service_task_by_name(
        service_name: &str,
        tasks: &Arc<RwLock<HashMap<String, JoinHandle<()>>>>,
        notification_states: &Arc<RwLock<HashMap<String, ServiceNotificationState>>>,
    ) {
        // 停止任务
        {
            let mut task_map = tasks.write().await;
            if let Some(task) = task_map.remove(service_name) {
                task.abort();
                info!("已停止服务任务: {}", service_name);
            }
        }

        // 清理通知状态
        {
            let mut states = notification_states.write().await;
            states.remove(service_name);
        }
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
                    .with_context(|| format!("启动服务任务失败: {service_name}"))?;
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
