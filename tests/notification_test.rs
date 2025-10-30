//! 通知系统测试
//!
//! 测试通知系统的状态变化检测和通知逻辑

use service_vitals::health::{
    result::HealthStatus,
    scheduler::{FailureState, NotificationState, ServiceNotificationState},
};
use std::time::Duration;
use tokio::time::Instant;

#[test]
fn test_failure_state_default() {
    let state = FailureState::default();
    assert_eq!(state.consecutive_failures, 0);
    assert!(state.first_failure_time.is_none());
}

#[test]
fn test_notification_state_default() {
    let state = NotificationState::default();
    assert_eq!(state.notification_count, 0);
    assert_eq!(state.notification_failures, 0);
    assert_eq!(state.missed_notifications_during_cooldown, 0);
    assert!(state.last_notification_time.is_none());
    assert!(state.alert_cooldown_until.is_none());
}

#[test]
fn test_service_notification_state_default() {
    let state = ServiceNotificationState::default();
    assert!(state.last_health_status.is_none());
    assert_eq!(state.failure_state.consecutive_failures, 0);
    assert_eq!(state.notification_state.notification_count, 0);
}

#[test]
fn test_check_status_change_first_time() {
    let state = ServiceNotificationState::default();
    let current_status = HealthStatus::Up;

    // 第一次检测，应该视为状态变化
    let (changed, recovered) =
        service_vitals::health::scheduler::TaskScheduler::check_status_change(
            current_status,
            &state,
        );

    assert!(changed);
    assert!(!recovered); // 第一次检测，没有"恢复"的概念
}

#[test]
fn test_check_status_change_no_change() {
    let mut state = ServiceNotificationState::default();
    state.last_health_status = Some(HealthStatus::Up);

    let current_status = HealthStatus::Up;

    // 状态没有变化
    let (changed, recovered) =
        service_vitals::health::scheduler::TaskScheduler::check_status_change(
            current_status,
            &state,
        );

    assert!(!changed);
    assert!(!recovered);
}

#[test]
fn test_check_status_change_recovered() {
    let mut state = ServiceNotificationState::default();
    state.last_health_status = Some(HealthStatus::Down);

    let current_status = HealthStatus::Up;

    // 从不健康状态恢复到健康状态
    let (changed, recovered) =
        service_vitals::health::scheduler::TaskScheduler::check_status_change(
            current_status,
            &state,
        );

    assert!(changed);
    assert!(recovered);
}

#[test]
fn test_should_send_alert_below_threshold() {
    let mut state = ServiceNotificationState::default();
    state.failure_state.consecutive_failures = 2;

    let service = service_vitals::config::types::ServiceConfig {
        name: "test".to_string(),
        url: "http://example.com".to_string(),
        method: "GET".to_string(),
        expected_status_codes: vec![200],
        failure_threshold: 3,
        alert_cooldown_secs: Some(60),
        enabled: true,
        description: None,
        feishu_webhook_url: None,
        check_interval_seconds: None,
        headers: std::collections::HashMap::new(),
        body: None,
    };

    let now = Instant::now();

    // 连续失败次数低于阈值，不应该发送告警
    let should_alert =
        service_vitals::health::scheduler::TaskScheduler::should_send_alert(&state, &service, now);

    assert!(!should_alert);
}

#[test]
fn test_should_send_alert_at_threshold() {
    let mut state = ServiceNotificationState::default();
    state.failure_state.consecutive_failures = 3;
    state.failure_state.first_failure_time = Some(Instant::now() - Duration::from_secs(30));

    let service = service_vitals::config::types::ServiceConfig {
        name: "test".to_string(),
        url: "http://example.com".to_string(),
        method: "GET".to_string(),
        expected_status_codes: vec![200],
        failure_threshold: 3,
        alert_cooldown_secs: Some(60),
        enabled: true,
        description: None,
        feishu_webhook_url: None,
        check_interval_seconds: None,
        headers: std::collections::HashMap::new(),
        body: None,
    };

    let now = Instant::now();

    // 连续失败次数达到阈值，应该发送告警
    let should_alert =
        service_vitals::health::scheduler::TaskScheduler::should_send_alert(&state, &service, now);

    assert!(should_alert);
}

#[test]
fn test_should_send_alert_in_cooldown() {
    let mut state = ServiceNotificationState::default();
    state.failure_state.consecutive_failures = 4;
    state.failure_state.first_failure_time = Some(Instant::now() - Duration::from_secs(120));
    state.notification_state.alert_cooldown_until = Some(Instant::now() + Duration::from_secs(30));

    let service = service_vitals::config::types::ServiceConfig {
        name: "test".to_string(),
        url: "http://example.com".to_string(),
        method: "GET".to_string(),
        expected_status_codes: vec![200],
        failure_threshold: 3,
        alert_cooldown_secs: Some(60),
        enabled: true,
        description: None,
        feishu_webhook_url: None,
        check_interval_seconds: None,
        headers: std::collections::HashMap::new(),
        body: None,
    };

    let now = Instant::now();

    // 在冷却期内，不应该发送告警
    let should_alert =
        service_vitals::health::scheduler::TaskScheduler::should_send_alert(&state, &service, now);

    assert!(!should_alert);
}

#[test]
fn test_should_send_alert_simplified_logic() {
    let mut state = ServiceNotificationState::default();
    let service = service_vitals::config::types::ServiceConfig {
        name: "test".to_string(),
        url: "http://example.com".to_string(),
        method: "GET".to_string(),
        expected_status_codes: vec![200],
        failure_threshold: 3,
        alert_cooldown_secs: Some(60),
        enabled: true,
        description: None,
        feishu_webhook_url: None,
        check_interval_seconds: None,
        headers: std::collections::HashMap::new(),
        body: None,
    };

    let now = Instant::now();

    // 测试1: 连续失败次数低于阈值，不应该发送告警
    state.failure_state.consecutive_failures = 2;
    assert!(
        !service_vitals::health::scheduler::TaskScheduler::should_send_alert(&state, &service, now)
    );

    // 测试2: 首次达到阈值，应该发送告警
    state.failure_state.consecutive_failures = 3;
    assert!(
        service_vitals::health::scheduler::TaskScheduler::should_send_alert(&state, &service, now)
    );

    // 测试3: 超过阈值但不在冷却期，应该发送告警
    state.failure_state.consecutive_failures = 4;
    state.notification_state.alert_cooldown_until = Some(now - Duration::from_secs(10)); // 冷却期已过
    assert!(
        service_vitals::health::scheduler::TaskScheduler::should_send_alert(&state, &service, now)
    );

    // 测试4: 超过阈值且在冷却期，不应该发送告警
    state.notification_state.alert_cooldown_until = Some(now + Duration::from_secs(10)); // 仍在冷却期
    assert!(
        !service_vitals::health::scheduler::TaskScheduler::should_send_alert(&state, &service, now)
    );
}

#[test]
fn test_update_alert_cooldown_simplified() {
    let mut state = ServiceNotificationState::default();
    let service = service_vitals::config::types::ServiceConfig {
        name: "test".to_string(),
        url: "http://example.com".to_string(),
        method: "GET".to_string(),
        expected_status_codes: vec![200],
        failure_threshold: 3,
        alert_cooldown_secs: Some(60),
        enabled: true,
        description: None,
        feishu_webhook_url: None,
        check_interval_seconds: None,
        headers: std::collections::HashMap::new(),
        body: None,
    };

    let now = Instant::now();

    // 测试1: 首次达到阈值，不应该设置冷却时间
    service_vitals::health::scheduler::TaskScheduler::update_alert_cooldown(
        &mut state, &service, now, true,
    );
    assert!(state.notification_state.alert_cooldown_until.is_none());

    // 测试2: 非首次达到阈值，应该设置冷却时间
    service_vitals::health::scheduler::TaskScheduler::update_alert_cooldown(
        &mut state, &service, now, false,
    );
    assert_eq!(
        state.notification_state.alert_cooldown_until,
        Some(now + Duration::from_secs(60))
    );
}

#[test]
fn test_reset_failure_state() {
    let mut state = ServiceNotificationState::default();
    state.failure_state.consecutive_failures = 5;
    state.failure_state.first_failure_time = Some(Instant::now());
    state.notification_state.alert_cooldown_until = Some(Instant::now() + Duration::from_secs(60));
    state
        .notification_state
        .missed_notifications_during_cooldown = 3;

    service_vitals::health::scheduler::TaskScheduler::reset_failure_state(&mut state);

    assert_eq!(state.failure_state.consecutive_failures, 0);
    assert!(state.failure_state.first_failure_time.is_none());
    assert!(state.notification_state.alert_cooldown_until.is_none());
    assert_eq!(
        state
            .notification_state
            .missed_notifications_during_cooldown,
        0
    );
}

#[test]
fn test_update_failure_state() {
    let mut state = ServiceNotificationState::default();
    let now = Instant::now();

    service_vitals::health::scheduler::TaskScheduler::update_failure_state(&mut state, now);

    assert_eq!(state.failure_state.consecutive_failures, 1);
    assert_eq!(state.failure_state.first_failure_time, Some(now));

    // 再次更新
    let later = now + Duration::from_secs(10);
    service_vitals::health::scheduler::TaskScheduler::update_failure_state(&mut state, later);

    assert_eq!(state.failure_state.consecutive_failures, 2);
    // 首次失败时间不应该更新
    assert_eq!(state.failure_state.first_failure_time, Some(now));
}

#[test]
fn test_update_alert_cooldown() {
    let mut state = ServiceNotificationState::default();
    let service = service_vitals::config::types::ServiceConfig {
        name: "test".to_string(),
        url: "http://example.com".to_string(),
        method: "GET".to_string(),
        expected_status_codes: vec![200],
        failure_threshold: 3,
        alert_cooldown_secs: Some(60),
        enabled: true,
        description: None,
        feishu_webhook_url: None,
        check_interval_seconds: None,
        headers: std::collections::HashMap::new(),
        body: None,
    };

    let now = Instant::now();

    // 第一次达到阈值，不应该设置冷却时间
    service_vitals::health::scheduler::TaskScheduler::update_alert_cooldown(
        &mut state, &service, now, true, false,
    );

    assert!(state.notification_state.alert_cooldown_until.is_none());

    // 非第一次达到阈值，应该设置冷却时间
    service_vitals::health::scheduler::TaskScheduler::update_alert_cooldown(
        &mut state, &service, now, false, false,
    );

    assert_eq!(
        state.notification_state.alert_cooldown_until,
        Some(now + Duration::from_secs(60))
    );
}

#[test]
fn test_reset_failure_state_comprehensive() {
    let mut state = ServiceNotificationState::default();

    // 设置各种状态
    state.failure_state.consecutive_failures = 5;
    state.failure_state.first_failure_time = Some(Instant::now());
    state.notification_state.alert_cooldown_until = Some(Instant::now() + Duration::from_secs(60));
    state
        .notification_state
        .missed_notifications_during_cooldown = 3;
    state.notification_state.notification_count = 2;
    state.notification_state.last_notification_time = Some(Instant::now());
    state.notification_state.notification_failures = 1;

    // 重置状态
    service_vitals::health::scheduler::TaskScheduler::reset_failure_state(&mut state);

    // 验证所有相关状态都被重置
    assert_eq!(state.failure_state.consecutive_failures, 0);
    assert!(state.failure_state.first_failure_time.is_none());
    assert!(state.notification_state.alert_cooldown_until.is_none());
    assert_eq!(
        state
            .notification_state
            .missed_notifications_during_cooldown,
        0
    );

    // 验证不相关的状态没有被重置
    assert_eq!(state.notification_state.notification_count, 2);
    assert!(state.notification_state.last_notification_time.is_some());
    assert_eq!(state.notification_state.notification_failures, 1);
}
