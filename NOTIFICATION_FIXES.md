# 通知机制修复和改进总结

## 问题分析

### 1. 从未发送警告错误信息，却会发送回复通知的问题

**问题根源**：
- 在 `src/health/scheduler.rs` 的 `handle_notification_static` 方法中，恢复通知的判断逻辑存在缺陷
- 只检查了 `consecutive_failures > 0`，但没有检查是否真的发送过告警通知
- 如果服务从未达到告警阈值（`failure_threshold`），`consecutive_failures` 可能大于0但从未发送过告警，这时服务恢复就会发送不必要的恢复通知

### 2. 通知刷屏问题

**问题根源**：
- 没有对通知频率进行有效控制
- 告警冷却时间（`alert_cooldown_secs`）默认只有60秒，太短
- 没有全局通知频率限制
- 恢复通知没有冷却时间

## 修复方案

### 1. 修复恢复通知逻辑

**修改文件**: `src/health/scheduler.rs`

**主要改进**:
- 添加了 `has_sent_alert` 字段来跟踪是否真的发送过告警通知
- 只有在之前发送过告警通知的情况下才发送恢复通知
- 添加了恢复通知的冷却时间（告警冷却时间的1/5）

```rust
// 修复前
let need_recover_notify = is_healthy && notification_state.consecutive_failures > 0;

// 修复后
let need_recover_notify = is_healthy 
    && notification_state.consecutive_failures > 0 
    && notification_state.has_sent_alert;
```

### 2. 改进通知频率控制

**修改文件**: `src/health/scheduler.rs`

**主要改进**:
- 将默认告警冷却时间从60秒增加到300秒（5分钟）
- 添加了恢复通知冷却时间
- 实现了全局通知频率限制机制

**新增字段**:
```rust
pub struct ServiceNotificationState {
    // ... 现有字段 ...
    pub has_sent_alert: bool,                    // 是否已发送过告警
    pub recovery_cooldown_until: Option<Instant>, // 恢复通知冷却时间
    pub last_recovery_notification_time: Option<Instant>, // 上次恢复通知时间
}

pub struct NotificationStats {
    // ... 现有字段 ...
    pub global_cooldown_until: Option<Instant>,  // 全局通知冷却时间
    pub recent_notifications: Vec<Instant>,      // 最近1小时内的通知记录
}
```

**全局频率限制规则**:
- 最近1小时内最多发送10次通知
- 超过限制后，设置30分钟的全局冷却时间
- 所有通知（告警和恢复）都受此限制

### 3. 改进配置默认值

**修改文件**: `src/config/types.rs`

**主要改进**:
- 将 `alert_cooldown_secs` 从 `Option<u64>` 改为 `u64`，设置默认值为300秒
- 添加了 `default_alert_cooldown()` 函数

```rust
// 修复前
pub alert_cooldown_secs: Option<u64>,

// 修复后
#[serde(default = "default_alert_cooldown")]
pub alert_cooldown_secs: u64,

fn default_alert_cooldown() -> u64 {
    300 // 5分钟默认冷却时间
}
```

### 4. 改进通知模板

**修改文件**: `src/notification/template.rs`

**主要改进**:
- 更新了默认告警和恢复模板，使其更加友好和有用
- 添加了更多有用的信息字段
- 改进了消息格式和内容

**新增模板字段**:
- `http_method`: HTTP检测方法
- `failure_threshold`: 失败阈值
- `alert_cooldown_secs`: 告警冷却时间
- `check_interval_secs`: 检测间隔
- `consecutive_failures`: 连续失败次数
- `response_size_bytes`: 响应体大小

### 5. 增强模板上下文

**修改文件**: `src/notification/feishu.rs`

**主要改进**:
- 在 `create_template_context` 方法中添加了更多有用的字段
- 提供了更丰富的通知信息

## 修复效果

### 1. 解决恢复通知问题
- ✅ 只有在真正发送过告警通知后才会发送恢复通知
- ✅ 避免了从未告警就发送恢复通知的问题

### 2. 解决通知刷屏问题
- ✅ 默认告警冷却时间从60秒增加到300秒
- ✅ 添加了恢复通知冷却时间（60秒）
- ✅ 实现了全局通知频率限制（1小时内最多10次）
- ✅ 超过频率限制后自动设置30分钟冷却时间

### 3. 改进用户体验
- ✅ 通知消息更加友好和有用
- ✅ 包含更多有用的诊断信息
- ✅ 提供了建议操作步骤
- ✅ 明确说明了下次告警的时间

## 配置建议

### 推荐的配置参数

```yaml
global:
  default_feishu_webhook_url: "your_webhook_url"
  check_interval_seconds: 60
  request_timeout_seconds: 10

services:
  - name: "example-service"
    url: "https://example.com/health"
    expected_status_codes: [200]
    failure_threshold: 3        # 连续失败3次才告警
    alert_cooldown_secs: 300    # 5分钟告警冷却时间
    check_interval_seconds: 30  # 30秒检测间隔
    description: "示例服务"
```

### 频率控制说明

1. **服务级控制**:
   - 每个服务独立的告警冷却时间
   - 恢复通知冷却时间为告警冷却时间的1/5

2. **全局控制**:
   - 1小时内最多发送10次通知
   - 超过限制后30分钟内不发送任何通知

3. **建议设置**:
   - 告警冷却时间: 300-600秒（5-10分钟）
   - 检测间隔: 30-60秒
   - 失败阈值: 2-3次

## 测试建议

1. **功能测试**:
   - 测试服务故障时的告警通知
   - 测试服务恢复时的恢复通知
   - 验证从未告警不会发送恢复通知

2. **频率控制测试**:
   - 测试告警冷却时间是否生效
   - 测试恢复通知冷却时间是否生效
   - 测试全局频率限制是否生效

3. **模板测试**:
   - 验证通知消息格式是否正确
   - 检查所有模板变量是否正确渲染
   - 确认消息内容是否友好有用