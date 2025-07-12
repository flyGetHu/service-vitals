# 批量通知机制实现总结

## 需求分析

根据用户需求，需要实现以下功能：
1. 默认检查时间改为5秒
2. 5秒钟一个周期，所有需要通知的警告在一次警告信息中通知
3. 恢复通知也一样，通知中要显示所有有问题的服务
4. 紧促显示服务名称、链接、问题原因信息

## 实现方案

### 1. 批量通知架构

**核心组件**:
- `BatchNotificationItem`: 批量通知项，包含服务配置、检测结果、通知类型等
- `BatchNotificationType`: 批量通知类型枚举（告警/恢复）
- `TaskScheduler`: 调度器，包含批量通知队列和任务

**数据结构**:
```rust
pub struct BatchNotificationItem {
    pub service: ServiceConfig,           // 服务配置
    pub result: HealthResult,             // 健康检测结果
    pub notification_type: BatchNotificationType, // 通知类型
    pub notification_time: Instant,       // 通知时间
}

pub enum BatchNotificationType {
    Alert,    // 告警通知
    Recovery, // 恢复通知
}
```

### 2. 批量通知流程

**通知收集阶段**:
1. 健康检测任务检测到服务异常或恢复
2. 调用 `handle_notification_static` 方法
3. 判断是否需要发送通知（基于冷却时间和阈值）
4. 如果需要通知，记录到通知状态中，但不立即发送

**批量发送阶段**:
1. 批量通知任务每5秒运行一次
2. 收集队列中所有待发送的通知
3. 按类型分组（告警/恢复）
4. 分别发送批量告警通知和批量恢复通知

### 3. 关键修改

#### 3.1 修改默认检查间隔
```rust
// src/config/types.rs
fn default_check_interval() -> u64 {
    5 // 5秒检查间隔
}
```

#### 3.2 添加批量通知队列
```rust
// src/health/scheduler.rs
pub struct TaskScheduler {
    // ... 现有字段 ...
    batch_notifications: Arc<RwLock<Vec<BatchNotificationItem>>>, // 批量通知队列
    batch_task_handle: Option<JoinHandle<()>>,                    // 批量通知任务句柄
}
```

#### 3.3 修改通知处理逻辑
```rust
// 原来的立即发送逻辑改为记录到状态中
if can_alert {
    debug!("服务 {} 需要告警通知，等待批量发送", service.name);
    notification_state.notification_count += 1;
    notification_state.last_notification_time = Some(now);
    notification_state.has_sent_alert = true;
    notification_state.alert_cooldown_until = Some(now + Duration::from_secs(cooldown_secs));
}
```

#### 3.4 实现批量通知任务
```rust
async fn start_batch_notification_task(&mut self) {
    let task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5)); // 5秒批量间隔
        
        loop {
            interval.tick().await;
            
            // 获取待发送的通知
            let notifications = {
                let mut queue = batch_notifications.write().await;
                queue.drain(..).collect::<Vec<_>>()
            };

            if !notifications.is_empty() {
                // 按类型分组并发送
                let (alerts, recoveries) = notifications.into_iter()
                    .partition(|item| matches!(item.notification_type, BatchNotificationType::Alert));

                // 发送批量告警通知
                if !alerts.is_empty() {
                    Self::send_batch_alert(notifier, &alerts).await;
                }

                // 发送批量恢复通知
                if !recoveries.is_empty() {
                    Self::send_batch_recovery(notifier, &recoveries).await;
                }
            }
        }
    });
}
```

### 4. 批量通知消息格式

#### 4.1 批量告警通知
```
🚨 **批量服务告警通知**

**检测时间**: 2024-01-01 12:00:00
**告警服务数量**: 3

**1. 用户服务**
- **服务地址**: https://user-service.example.com
- **服务描述**: 用户管理服务
- **HTTP状态码**: 500
- **响应时间**: 5000ms
- **检测方法**: GET
- **失败阈值**: 3次
- **错误信息**: Internal Server Error

**2. 订单服务**
- **服务地址**: https://order-service.example.com
- **HTTP状态码**: 连接失败
- **响应时间**: 10000ms
- **检测方法**: GET
- **失败阈值**: 3次
- **错误信息**: Connection timeout

**建议操作**
1. 检查服务是否正常运行
2. 查看服务器日志
3. 验证网络连接
4. 检查配置是否正确

---
*此通知由 Service Vitals 自动发送*
```

#### 4.2 批量恢复通知
```
✅ **批量服务恢复通知**

**恢复时间**: 2024-01-01 12:05:00
**恢复服务数量**: 2

**1. 用户服务**
- **服务地址**: https://user-service.example.com
- **服务描述**: 用户管理服务
- **HTTP状态码**: 200
- **响应时间**: 150ms
- **检测方法**: GET
- **服务状态**: 正常运行 ✅

**2. 订单服务**
- **服务地址**: https://order-service.example.com
- **HTTP状态码**: 200
- **响应时间**: 200ms
- **检测方法**: GET
- **服务状态**: 正常运行 ✅

---
*此通知由 Service Vitals 自动发送*
```

### 5. 优势特点

#### 5.1 减少通知频率
- 将多个单独的通知合并为批量通知
- 避免短时间内发送大量通知造成刷屏
- 提高通知的可读性和重要性

#### 5.2 信息完整性
- 在一个通知中包含所有相关服务的信息
- 提供完整的诊断信息（状态码、响应时间、错误信息等）
- 便于运维人员快速了解整体状况

#### 5.3 紧促显示
- 服务名称、链接、问题原因信息紧凑排列
- 使用编号和格式化提高可读性
- 支持服务描述等额外信息

#### 5.4 智能分组
- 告警和恢复通知分别发送
- 避免混合通知造成混淆
- 便于快速识别问题类型

### 6. 配置建议

#### 6.1 推荐配置
```yaml
global:
  check_interval_seconds: 5  # 5秒检查间隔
  default_feishu_webhook_url: "your_webhook_url"

services:
  - name: "用户服务"
    url: "https://user-service.example.com/health"
    expected_status_codes: [200]
    failure_threshold: 3        # 连续失败3次才告警
    alert_cooldown_secs: 300    # 5分钟告警冷却时间
    description: "用户管理服务"
    
  - name: "订单服务"
    url: "https://order-service.example.com/health"
    expected_status_codes: [200]
    failure_threshold: 2        # 连续失败2次就告警
    alert_cooldown_secs: 180    # 3分钟告警冷却时间
    description: "订单管理服务"
```

#### 6.2 批量通知参数
- **批量间隔**: 5秒（固定）
- **检查间隔**: 5秒（可配置）
- **告警冷却**: 300秒（可配置）
- **恢复冷却**: 告警冷却时间的1/5

### 7. 测试建议

#### 7.1 功能测试
1. **批量告警测试**: 同时让多个服务异常，验证是否在5秒后发送批量告警
2. **批量恢复测试**: 同时恢复多个服务，验证是否发送批量恢复通知
3. **混合场景测试**: 部分服务告警，部分服务恢复，验证分组发送

#### 7.2 性能测试
1. **大量服务测试**: 配置大量服务，验证批量通知的性能
2. **高频检测测试**: 验证5秒间隔检测的性能影响
3. **通知频率测试**: 验证批量通知是否有效减少通知频率

#### 7.3 消息格式测试
1. **消息长度测试**: 验证大量服务时的消息长度是否合适
2. **特殊字符测试**: 验证服务名称、URL、错误信息中的特殊字符处理
3. **空值处理测试**: 验证缺少描述、错误信息等字段时的处理

### 8. 后续优化建议

#### 8.1 功能增强
- 支持按服务组分组通知
- 支持自定义批量通知模板
- 支持通知优先级设置

#### 8.2 性能优化
- 支持通知队列大小限制
- 支持通知发送失败重试
- 支持通知历史记录

#### 8.3 用户体验
- 支持通知消息的折叠展开
- 支持快速操作按钮（如重启服务）
- 支持通知消息的搜索过滤