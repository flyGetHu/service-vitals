# 服务监控面板修复说明

## 修复的问题

### 1. 服务统计计算问题

**问题描述：**
当启用"只显示离线服务"过滤器时，页面顶部的统计信息（总服务数量、在线数量、离线数量）显示的是过滤后的数据，而不是所有服务的真实统计数据。

**根本原因：**
- 在 `src/web/handlers.rs` 的 `dashboard` 函数中，统计数据是基于过滤后的服务列表计算的
- 在 `api_status` 函数中，也存在类似问题，且缺少对 `unknown_services` 的统计

**修复方案：**
1. **后端修复（src/web/handlers.rs）：**
   - 修改 `dashboard` 函数：先计算所有服务的统计数据，再进行显示过滤
   - 修改 `api_status` 函数：API 端点总是返回所有服务数据，统计数据基于完整的服务列表
   - 添加 `unknown_services` 字段到 API 响应中

2. **前端修复（templates/dashboard.html）：**
   - 添加前端过滤器控制：用户可以通过复选框切换"只显示离线服务"
   - 修改 `updateServicesGrid` 函数：支持前端过滤，不影响统计数据
   - 修改 `updateStats` 函数：正确处理 `unknown_services` 字段

### 2. 自动刷新功能问题

**问题描述：**
页面的自动刷新数据功能存在以下问题：
- 连续失败后会永久停止刷新
- 页面可见性检测可能过于严格
- 模板语法在 JavaScript 中导致语法错误

**修复方案：**
1. **改进错误处理：**
   - 移除连续失败后停止刷新的逻辑
   - 实现错误计数重置机制，避免永久停止
   - 保持持续尝试，提高系统健壮性

2. **优化页面可见性检测：**
   - 添加窗口焦点事件监听
   - 页面重新可见时稍微延迟刷新，确保页面完全可见
   - 改进事件处理逻辑

3. **修复模板语法问题：**
   - 避免在 JavaScript 中直接使用模板变量
   - 改用从 DOM 元素获取配置的方式
   - 修复 Askama 模板语法错误

## 技术实现细节

### 后端变更

1. **API 响应结构更新：**
```rust
struct ApiStatusResponse {
    services: Vec<ApiServiceStatus>,
    last_updated: String,
    total_services: usize,
    online_services: usize,
    offline_services: usize,
    unknown_services: usize,  // 新增字段
}
```

2. **统计逻辑分离：**
- 统计计算与显示过滤完全分离
- API 端点总是返回完整的服务列表和准确的统计数据

### 前端变更

1. **添加过滤器控制：**
```html
<input type="checkbox" id="show-problems-only" {% if show_problems_only %}checked{% endif %}>
<span>只显示离线服务</span>
```

2. **前端过滤逻辑：**
```javascript
// 根据前端过滤器状态过滤服务
let filteredServices = services;
if (showProblemsOnly) {
  filteredServices = services.filter(service => 
    service.status === 'Offline' || service.status === 'Unknown'
  );
}
```

3. **改进的错误处理：**
```javascript
// 不再因连续失败停止刷新
if (errorCount > maxErrors * 2) {
    errorCount = Math.floor(maxErrors / 2);
}
```

## 测试验证

添加了新的测试用例 `test_api_status_statistics_accuracy` 来验证：
- 统计数据的准确性
- 过滤功能不影响统计计算
- API 响应的一致性

## 使用说明

### 配置文件设置
```toml
[global.web]
enabled = true
port = 8080
bind_address = "127.0.0.1"
show_problems_only = false  # 默认显示所有服务
refresh_interval_seconds = 3
```

### 功能特性
1. **统计数据始终准确：** 无论是否启用过滤，统计数据都反映所有服务的真实状态
2. **灵活的过滤控制：** 用户可以通过界面复选框实时切换显示模式
3. **健壮的自动刷新：** 即使遇到网络问题也会持续尝试更新数据
4. **响应式设计：** 支持不同屏幕尺寸的设备

## 兼容性说明

- 保持向后兼容：现有配置文件无需修改
- API 响应格式扩展：添加了 `unknown_services` 字段，但不影响现有客户端
- 前端功能增强：添加了用户控制选项，提升用户体验
