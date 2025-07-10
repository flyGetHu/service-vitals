# Service Status 功能改进演示

## 概述

改进后的 `service-status` 命令提供了更详细、更易读的系统服务状态报告，并增加了对指标更新的检查功能。

## 主要改进

### 1. 格式化输出
- 美观的文本格式输出，使用表情符号和分层结构
- 支持 JSON、YAML、Text 三种输出格式
- 响应式表格显示，自动调整内容长度

### 2. 综合状态展示
- **系统服务状态**：显示服务是否安装、运行状态等
- **应用监控状态**：显示健康检查服务的运行情况
- **指标更新检查**：验证监控指标是否正常更新

### 3. 智能建议
- 根据当前状态提供操作建议
- 自动诊断常见问题并给出解决方案

## 使用示例

### 基本用法
```bash
# 查看系统服务状态（简化模式）
service-vitals service-status

# 详细模式显示完整服务列表
service-vitals service-status --verbose

# JSON 格式输出
service-vitals service-status --format json

# YAML 格式输出
service-vitals service-status --format yaml --verbose
```

### 输出示例

#### 文本格式（简化模式）
```
🔍 Service Vitals 系统服务状态报告
生成时间: 2024-01-15 10:30:45 UTC

📋 系统服务状态:
  服务名称: service-vitals
  平台: Linux
  安装状态: ✅ 已安装
  运行状态: ✅ 运行中

📊 应用监控状态:
  启动时间: 2024-01-15 08:15:30 UTC
  配置文件: /etc/service-vitals/config.toml
  运行时长: 2小时15分钟

  📈 服务统计:
    总服务数: 5
    健康服务: 4 ✅
    异常服务: 1 ❌
    禁用服务: 0 ⏸️
    健康度: 80.0%

  📋 最近检测活动:
    1. ✅ web-api (30秒前)
    2. ✅ database (1分钟前)
    3. ❌ cache-server (2分钟前)
    4. ✅ message-queue (3分钟前)
    5. ✅ auth-service (5分钟前)

🔄 指标更新检查:
  更新状态: ✅ 正常更新
  最后更新: 30秒前
  状态总结: 服务正常运行，指标持续更新
```

#### 文本格式（详细模式）
```
🔍 Service Vitals 系统服务状态报告
生成时间: 2024-01-15 10:30:45 UTC

📋 系统服务状态:
  服务名称: service-vitals
  平台: Linux
  安装状态: ✅ 已安装
  运行状态: ✅ 运行中

📊 应用监控状态:
  启动时间: 2024-01-15 08:15:30 UTC
  配置文件: /etc/service-vitals/config.toml
  运行时长: 2小时15分钟

  📈 服务统计:
    总服务数: 5
    健康服务: 4 ✅
    异常服务: 1 ❌
    禁用服务: 0 ⏸️
    健康度: 80.0%

  📋 服务详情:
┌─────────────────────────────────────────────────────────────────────────────────────┐
│ 服务名称                │ 状态 │ 状态码 │ 响应时间 │ 最后检测时间              │
├─────────────────────────────────────────────────────────────────────────────────────┤
│ web-api                 │ ✅   │ 200    │ 45ms     │ 01-15 10:30:15            │
│ database                │ ✅   │ 200    │ 12ms     │ 01-15 10:29:45            │
│ cache-server            │ ❌   │ 500    │ 120ms    │ 01-15 10:28:30            │
│   错误: Connection timeout after 5000ms                                        │
│ message-queue           │ ✅   │ 200    │ 8ms      │ 01-15 10:27:45            │
│ auth-service            │ ✅   │ 200    │ 23ms     │ 01-15 10:25:30            │
└─────────────────────────────────────────────────────────────────────────────────────┘

🔄 指标更新检查:
  更新状态: ✅ 正常更新
  最后更新: 30秒前
  状态总结: 服务正常运行，指标持续更新
```

#### JSON 格式
```json
{
  "system_service": {
    "name": "service-vitals",
    "status": "Running",
    "is_installed": true,
    "platform": "Linux"
  },
  "application_status": {
    "start_time": "2024-01-15T08:15:30Z",
    "config_path": "/etc/service-vitals/config.toml",
    "total_services": 5,
    "healthy_services": 4,
    "unhealthy_services": 1,
    "disabled_services": 0,
    "services": [
      {
        "name": "web-api",
        "url": "https://api.example.com/health",
        "status": "Up",
        "last_check": "2024-01-15T10:30:15Z",
        "status_code": 200,
        "response_time_ms": 45,
        "enabled": true
      }
    ]
  },
  "metrics_update_check": {
    "is_updating": true,
    "last_update_age_seconds": 30,
    "status_summary": "服务正常运行，指标持续更新"
  }
}
```

## 指标更新检查功能

### 检查逻辑
1. **更新频率检查**：检查最近5分钟内是否有新的健康检查记录
2. **服务状态分析**：分析系统服务和应用状态的一致性
3. **异常诊断**：识别潜在的问题并提供解决建议

### 状态判断
- ✅ **正常更新**：最近5分钟内有检查记录且系统服务运行正常
- ❌ **更新异常**：以下情况之一：
  - 系统服务未运行
  - 应用状态文件不存在
  - 最近5分钟内无检查记录

### 故障排查建议
当检测到更新异常时，系统会自动提供相应的排查建议：

```
💡 建议操作:
  - 启动系统服务: service-vitals start-service
  - 检查服务配置和日志
  - 手动启动测试: service-vitals start --foreground
  - 检查服务日志: journalctl -u service-vitals -f
  - 重启服务: service-vitals restart-service
```

## 使用场景

### 1. 运维监控
- 快速了解服务整体运行状态
- 检查监控系统是否正常工作
- 识别需要注意的异常服务

### 2. 故障排查
- 详细的错误信息和时间戳
- 系统服务与应用状态的关联分析
- 自动化的故障诊断建议

### 3. 性能监控
- 服务响应时间统计
- 健康度百分比计算
- 历史趋势分析（通过时间戳）

## 配置建议

为了获得最佳的监控体验，建议：

1. **定期运行**：设置 cron job 定期检查状态
2. **日志记录**：将状态输出记录到日志文件
3. **告警集成**：结合监控系统实现自动告警

### 示例 cron 配置
```bash
# 每5分钟检查一次状态，记录到日志
*/5 * * * * /usr/local/bin/service-vitals service-status --format json >> /var/log/service-vitals-status.log 2>&1

# 每小时生成详细报告
0 * * * * /usr/local/bin/service-vitals service-status --verbose > /var/log/service-vitals-hourly.log 2>&1
```

## 开发说明

### 新增功能
1. **MetricsUpdateCheck 结构**：封装指标更新检查结果
2. **时间格式化函数**：友好的时间显示
3. **响应式表格**：自动调整显示宽度
4. **智能建议系统**：根据状态提供操作建议

### 代码改进
1. **模块化设计**：将不同格式的显示逻辑分离
2. **错误处理**：优雅处理各种异常情况
3. **性能优化**：减少不必要的文件读取操作
4. **用户体验**：直观的视觉提示和清晰的信息层次

这些改进使 `service-status` 命令成为一个强大而易用的系统监控工具，适合各种运维场景的需求。