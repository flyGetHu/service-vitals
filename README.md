# Service Vitals

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70+-blue.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey.svg)](https://github.com/flyGetHu/service-vitals)

一个跨平台的服务健康监控工具，支持HTTP/HTTPS服务检测、实时告警通知、Web监控界面和Prometheus指标导出。

## 🚀 项目概述

Service Vitals 是一个使用Rust开发的现代化服务健康监控解决方案，专为企业级应用设计。它提供了完整的服务监控生态系统，包括：

- **实时健康检测** - 支持HTTP/HTTPS服务的定期健康检查
- **智能告警系统** - 集成飞书webhook，支持自定义消息模板
- **配置热重载** - 无需重启即可更新监控配置
- **Web监控界面** - 直观的仪表板和实时状态展示
- **Prometheus集成** - 完整的指标导出和监控数据
- **跨平台支持** - 原生支持Windows、Linux和macOS
- **守护进程模式** - 支持系统服务安装和后台运行

## ✨ 功能特性

### 🔍 服务健康检测系统
- 支持HTTP/HTTPS协议检测
- 可配置的检测间隔和超时时间
- 多状态码验证支持
- 失败阈值和重试机制
- 并发检测优化

### 🔄 配置热重载功能
- 实时监控配置文件变化
- 无需重启服务即可更新配置
- 配置验证和错误处理
- 防抖动处理机制

### 📊 状态管理系统
- 实时服务状态跟踪
- 历史状态记录
- 状态持久化存储
- 多格式状态输出（JSON/YAML/表格）

### 🖥️ 跨平台守护进程支持
- Windows服务注册和管理
- Linux/macOS systemd集成
- 进程生命周期管理
- 优雅关闭和信号处理

### 🌐 Web监控界面
- 响应式仪表板设计
- 实时状态更新
- 服务详情展示
- RESTful API接口
- 可选的API密钥认证

### 📈 Prometheus指标导出
- 完整的监控指标收集
- 标准Prometheus格式
- 自定义指标标签
- 性能和可用性指标

### 🔔 告警通知系统
- 飞书webhook集成
- 自定义消息模板（Handlebars语法）
- 告警去重和频率控制
- 多通知渠道支持（规划中）

## 📦 安装指南

### 系统要求
- **操作系统**: Windows 10+, Linux (Ubuntu 18.04+, CentOS 7+), macOS 10.15+
- **内存**: 最少64MB RAM
- **磁盘空间**: 最少50MB可用空间
- **网络**: 需要访问被监控服务的网络连接

### 预编译二进制文件安装

#### Windows (PowerShell)
```powershell
# 下载最新版本
Invoke-WebRequest -Uri "https://github.com/flyGetHu/service-vitals/releases/latest/download/service-vitals-windows.exe" -OutFile "service-vitals.exe"

# 移动到系统路径
Move-Item "service-vitals.exe" "$env:ProgramFiles\ServiceVitals\service-vitals.exe"

# 添加到PATH环境变量
$env:PATH += ";$env:ProgramFiles\ServiceVitals"
```

#### Linux/macOS (Bash)
```bash
# 下载最新版本 (Linux)
curl -L "https://github.com/flyGetHu/service-vitals/releases/latest/download/service-vitals-linux" -o service-vitals

# 下载最新版本 (macOS)
curl -L "https://github.com/flyGetHu/service-vitals/releases/latest/download/service-vitals-macos" -o service-vitals

# 设置执行权限
chmod +x service-vitals

# 移动到系统路径
sudo mv service-vitals /usr/local/bin/
```

### 从源码编译

#### 安装Rust工具链
```bash
# 安装rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 重新加载环境变量
source ~/.cargo/env
```

#### 编译项目
```bash
# 克隆仓库
git clone https://github.com/flyGetHu/service-vitals.git
cd service-vitals

# 编译发布版本
cargo build --release

# 安装到系统路径
cargo install --path .
```

### Docker安装
```bash
# 拉取镜像
docker pull flygethu/service-vitals:latest

# 运行容器
docker run -d \
  --name service-vitals \
  -v /path/to/config.toml:/app/config.toml \
  -p 8080:8080 \
  flygethu/service-vitals:latest
```

## ⚙️ 配置说明

Service Vitals使用TOML格式的配置文件。以下是完整的配置示例：

### 基础配置文件 (`config.toml`)

```toml
[global]
# 默认飞书webhook URL（可选）
default_feishu_webhook_url = "https://open.feishu.cn/open-apis/bot/v2/hook/your-webhook-token"

# 消息模板（可选，使用Handlebars语法）
message_template = """
🚨 **服务告警**
- **服务名称**: {{service_name}}
- **服务URL**: {{service_url}}
- **状态码**: {{#if status_code}}{{status_code}}{{else}}N/A{{/if}}
- **响应时间**: {{response_time}}ms
- **检测时间**: {{timestamp}}
{{#if error_message}}
- **错误信息**: {{error_message}}
{{/if}}
"""

# 全局检测间隔，单位秒（默认60）
check_interval_seconds = 30

# 日志级别（可选，默认"info"）
# 支持：debug, info, warn, error
log_level = "info"

# 请求超时时间，单位秒（默认10）
request_timeout_seconds = 10

# 最大并发检测数（默认50）
max_concurrent_checks = 100

# 失败重试次数（默认3）
retry_attempts = 3

# 重试间隔，单位秒（默认5）
retry_delay_seconds = 5

# 全局请求头（可选）
[global.headers]
"User-Agent" = "ServiceVitals/1.0"
"Accept" = "application/json"

# Web界面配置（可选）
[web]
enabled = true
bind_address = "0.0.0.0"
port = 8080
api_key = "your-secure-api-key"
disable_auth = false
cors_enabled = true
cors_origins = ["*"]

# 服务配置列表
[[services]]
name = "主站API"
url = "https://api.example.com/health"
method = "GET"
expected_status_codes = [200, 201]
failure_threshold = 2
check_interval_seconds = 60  # 覆盖全局配置
enabled = true
description = "主站API健康检测"

# 服务特定的请求头
[services.headers]
"Authorization" = "Bearer ${API_TOKEN}"
"Content-Type" = "application/json"

# 服务特定的飞书webhook（可选）
feishu_webhook_url = "https://open.feishu.cn/open-apis/bot/v2/hook/service-specific-token"

[[services]]
name = "数据库服务"
url = "https://db.example.com/ping"
method = "GET"
expected_status_codes = [200]
failure_threshold = 1
enabled = true
description = "数据库连接检测"
```

### 最小配置示例

```toml
[global]
# 最小配置只需要指定必要的全局设置

[[services]]
name = "示例服务"
url = "https://httpbin.org/status/200"
expected_status_codes = [200]
```

### 配置参数说明

| 参数                      | 类型   | 默认值 | 说明               |
| ------------------------- | ------ | ------ | ------------------ |
| `check_interval_seconds`  | u64    | 60     | 全局检测间隔（秒） |
| `request_timeout_seconds` | u64    | 10     | 请求超时时间（秒） |
| `max_concurrent_checks`   | usize  | 50     | 最大并发检测数     |
| `retry_attempts`          | u32    | 3      | 失败重试次数       |
| `retry_delay_seconds`     | u64    | 5      | 重试间隔（秒）     |
| `log_level`               | String | "info" | 日志级别           |
| `failure_threshold`       | u32    | 1      | 失败阈值           |
| `enabled`                 | bool   | true   | 是否启用服务       |

## 🎯 使用教程

### CLI命令概览

Service Vitals提供了完整的命令行界面，支持以下主要命令：

```bash
service-vitals [OPTIONS] <COMMAND>
```

### 基本命令

#### 初始化配置
```bash
# Windows (PowerShell)
service-vitals init --template minimal

# Linux/macOS (Bash)
service-vitals init --template minimal
```

#### 验证配置
```bash
# Windows (PowerShell)
service-vitals validate --config config.toml --verbose

# Linux/macOS (Bash)
service-vitals validate --config config.toml --verbose
```

#### 执行健康检测
```bash
# 检测所有服务
service-vitals check

# 检测特定服务
service-vitals check "主站API"

# 指定输出格式
service-vitals check --format json
service-vitals check --format table
```

### 服务管理命令

#### 启动服务
```bash
# 前台运行
service-vitals start --foreground

# 后台运行
service-vitals start

# 自定义参数
service-vitals start --interval 30 --max-concurrent 100
```

#### 停止服务
```bash
# 正常停止
service-vitals stop

# 强制停止
service-vitals stop --force

# 指定超时时间
service-vitals stop --timeout 60
```

#### 查看状态
```bash
# 查看基本状态
service-vitals status

# 查看详细状态
service-vitals status --verbose

# JSON格式输出
service-vitals status --format json
```

### 系统服务管理

#### 安装系统服务
```bash
# Windows (PowerShell)
service-vitals install --service-name "ServiceVitals"

# Linux/macOS (Bash)
service-vitals install --service-name "service-vitals"
```

#### 启动系统服务
```bash
# Windows (PowerShell)
service-vitals start-service --service-name "ServiceVitals"

# Linux/macOS (Bash)
service-vitals start-service --service-name "service-vitals"
```

#### 查看系统服务状态
```bash
service-vitals service-status --service-name "service-vitals"
```

#### 卸载系统服务
```bash
service-vitals uninstall --service-name "service-vitals"
```

### 测试和调试

#### 测试通知功能
```bash
# 测试飞书通知
service-vitals test-notification feishu "这是一条测试消息"
```

#### 查看日志
```bash
# 实时查看日志
service-vitals logs --follow

# 查看特定级别日志
service-vitals logs --level error

# 查看最近的日志
service-vitals logs --tail 100
```

### 环境变量支持

Service Vitals支持通过环境变量配置关键参数：

```bash
# 配置文件路径
export SERVICE_VITALS_CONFIG="/path/to/config.toml"

# 日志级别
export SERVICE_VITALS_LOG_LEVEL="debug"

# 检测间隔
export SERVICE_VITALS_INTERVAL="30"

# 最大并发数
export SERVICE_VITALS_MAX_CONCURRENT="100"

# 工作目录
export SERVICE_VITALS_WORKDIR="/var/lib/service-vitals"
```

## 🌐 Web界面

Service Vitals提供了现代化的Web监控界面，支持实时状态展示和API访问。

### 访问Web界面

启动服务后，可以通过以下地址访问：

- **仪表板**: `http://localhost:8080/dashboard`
- **API文档**: `http://localhost:8080/api/v1/status`
- **健康检查**: `http://localhost:8080/api/v1/health`
- **Prometheus指标**: `http://localhost:8080/metrics`

### Web界面功能

#### 仪表板功能
- 📊 **实时状态概览** - 显示所有服务的健康状态统计
- 🔍 **服务详情** - 查看每个服务的详细监控信息
- ⏱️ **响应时间图表** - 可视化服务响应时间趋势
- 🔔 **告警历史** - 查看历史告警记录和处理状态
- 🔄 **自动刷新** - 可配置的自动数据刷新间隔

#### API端点

| 端点                       | 方法 | 说明                 | 认证 |
| -------------------------- | ---- | -------------------- | ---- |
| `/api/v1/status`           | GET  | 获取所有服务状态     | 可选 |
| `/api/v1/status/{service}` | GET  | 获取特定服务状态     | 可选 |
| `/api/v1/config`           | GET  | 获取配置信息（脱敏） | 可选 |
| `/api/v1/health`           | GET  | 系统健康检查         | 无   |
| `/metrics`                 | GET  | Prometheus指标       | 无   |

### API认证

Web界面支持可选的API密钥认证：

```bash
# 使用API密钥访问
curl -H "X-API-Key: your-secure-api-key" http://localhost:8080/api/v1/status
```

### CORS配置

支持跨域资源共享配置，适用于前端集成：

```toml
[web]
cors_enabled = true
cors_origins = ["https://your-frontend.com", "http://localhost:3000"]
```

## 📈 Prometheus集成

Service Vitals提供完整的Prometheus指标导出功能，支持与Grafana等监控系统集成。

### 指标端点

访问 `http://localhost:8080/metrics` 获取Prometheus格式的指标数据。

### 可用指标

#### 核心监控指标

| 指标名称                               | 类型      | 标签                | 说明                       |
| -------------------------------------- | --------- | ------------------- | -------------------------- |
| `service_vitals_health_check_total`    | Counter   | `service`, `status` | 健康检查总次数             |
| `service_vitals_response_time_seconds` | Histogram | `service`           | 响应时间分布（秒）         |
| `service_vitals_up`                    | Gauge     | `service`, `url`    | 服务状态（1=正常，0=异常） |
| `service_vitals_last_check_timestamp`  | Gauge     | `service`           | 最后检查时间戳             |
| `service_vitals_consecutive_failures`  | Gauge     | `service`           | 连续失败次数               |
| `service_vitals_start_time`            | Gauge     | -                   | 服务启动时间戳             |

#### 系统指标

| 指标名称                                  | 类型    | 说明         |
| ----------------------------------------- | ------- | ------------ |
| `service_vitals_config_reloads_total`     | Counter | 配置重载次数 |
| `service_vitals_notifications_sent_total` | Counter | 发送通知总数 |
| `service_vitals_active_services`          | Gauge   | 活跃服务数量 |

### Prometheus配置

在Prometheus配置文件中添加Service Vitals作为监控目标：

```yaml
# prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'service-vitals'
    static_configs:
      - targets: ['localhost:8080']
    scrape_interval: 30s
    metrics_path: '/metrics'
```

### Grafana仪表板

#### 导入预制仪表板

1. 下载仪表板配置文件：

```bash
# Windows (PowerShell)
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/flyGetHu/service-vitals/main/grafana/dashboard.json" -OutFile "service-vitals-dashboard.json"

# Linux/macOS (Bash)
curl -o service-vitals-dashboard.json https://raw.githubusercontent.com/flyGetHu/service-vitals/main/grafana/dashboard.json
```

2. 在Grafana中导入仪表板：
   - 访问Grafana Web界面
   - 点击 "+" → "Import"
   - 上传下载的JSON文件

#### 关键监控面板

- **服务可用性概览** - 显示所有服务的实时状态
- **响应时间趋势** - 服务响应时间的时间序列图表
- **错误率统计** - 失败检查的百分比和趋势
- **告警频率** - 告警触发的频率和分布
- **系统性能** - Service Vitals自身的性能指标

### 告警规则

创建Prometheus告警规则文件：

```yaml
# service-vitals-alerts.yml
groups:
  - name: service-vitals
    rules:
      - alert: ServiceDown
        expr: service_vitals_up == 0
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: "服务 {{ $labels.service }} 不可用"
          description: "服务 {{ $labels.service }} ({{ $labels.url }}) 已经不可用超过2分钟"

      - alert: HighResponseTime
        expr: histogram_quantile(0.95, service_vitals_response_time_seconds_bucket) > 5
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "服务 {{ $labels.service }} 响应时间过高"
          description: "服务 {{ $labels.service }} 的95%响应时间超过5秒"

      - alert: HighFailureRate
        expr: rate(service_vitals_health_check_total{status="failed"}[5m]) / rate(service_vitals_health_check_total[5m]) > 0.1
        for: 3m
        labels:
          severity: warning
        annotations:
          summary: "服务 {{ $labels.service }} 失败率过高"
          description: "服务 {{ $labels.service }} 在过去5分钟内失败率超过10%"
```

## 🛠️ 开发指南

### 项目结构

```text
service-vitals/
├── src/
│   ├── main.rs                 # 程序入口点
│   ├── lib.rs                  # 库入口，导出公共接口
│   ├── cli/                    # CLI命令模块
│   │   ├── mod.rs
│   │   ├── commands.rs         # 命令定义和处理
│   │   └── args.rs             # 命令行参数解析
│   ├── config/                 # 配置管理模块
│   │   ├── mod.rs
│   │   ├── types.rs            # 配置数据结构
│   │   ├── loader.rs           # 配置文件加载
│   │   └── watcher.rs          # 配置文件热重载
│   ├── health/                 # 健康检测模块
│   │   ├── mod.rs
│   │   ├── checker.rs          # 健康检测核心逻辑
│   │   ├── scheduler.rs        # 检测任务调度
│   │   └── result.rs           # 检测结果数据结构
│   ├── notification/           # 通知系统模块
│   │   ├── mod.rs
│   │   ├── feishu.rs           # 飞书webhook通知
│   │   └── template.rs         # 消息模板引擎
│   ├── web/                    # Web界面模块
│   │   ├── mod.rs
│   │   ├── server.rs           # Web服务器
│   │   ├── api.rs              # API端点
│   │   ├── dashboard.rs        # 仪表板
│   │   ├── metrics.rs          # Prometheus指标
│   │   └── auth.rs             # 认证中间件
│   ├── daemon/                 # 守护进程模块
│   │   ├── mod.rs
│   │   ├── unix.rs             # Unix系统守护进程
│   │   └── windows.rs          # Windows服务
│   ├── status.rs               # 状态管理
│   ├── error.rs                # 错误处理
│   └── logging.rs              # 日志系统
├── examples/                   # 配置示例
├── docs/                       # 文档
├── tests/                      # 测试文件
├── Cargo.toml                  # 项目配置
└── README.md                   # 项目说明
```

### 核心架构

Service Vitals采用模块化架构设计，主要组件包括：

1. **配置管理** - 支持TOML配置文件和热重载
2. **健康检测** - 异步HTTP检测和结果处理
3. **任务调度** - 基于tokio的并发任务调度
4. **通知系统** - 可扩展的通知渠道支持
5. **Web服务** - 基于warp的HTTP服务器
6. **状态管理** - 内存和持久化状态存储
7. **守护进程** - 跨平台系统服务支持

### 开发环境设置

#### 1. 安装依赖

```bash
# 安装Rust工具链
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装开发工具
cargo install cargo-watch cargo-audit cargo-outdated
```

#### 2. 克隆项目

```bash
git clone https://github.com/flyGetHu/service-vitals.git
cd service-vitals
```

#### 3. 运行开发环境

```bash
# 编译项目
cargo build

# 运行测试
cargo test

# 启动开发服务器（自动重载）
cargo watch -x run
```

### 贡献指南

我们欢迎社区贡献！请遵循以下步骤：

#### 1. Fork项目

点击GitHub页面右上角的"Fork"按钮

#### 2. 创建功能分支

```bash
git checkout -b feature/your-feature-name
```

#### 3. 提交更改

```bash
git add .
git commit -m "feat: 添加新功能描述"
```

#### 4. 推送分支

```bash
git push origin feature/your-feature-name
```

#### 5. 创建Pull Request

在GitHub上创建Pull Request，详细描述您的更改。

### 代码规范

- 使用 `cargo fmt` 格式化代码
- 使用 `cargo clippy` 检查代码质量
- 编写单元测试和集成测试
- 遵循Rust官方编码规范
- 添加适当的文档注释

### 测试

```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test health_checker

# 运行集成测试
cargo test --test integration

# 生成测试覆盖率报告
cargo tarpaulin --out Html
```

## 📄 许可证

本项目采用 [MIT许可证](https://opensource.org/licenses/MIT) 开源。

```text
MIT License

Copyright (c) 2024 flyGetHu

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## 🤝 支持与反馈

### 获取帮助

- 📖 **文档**: [项目Wiki](https://github.com/flyGetHu/service-vitals/wiki)
- 🐛 **问题报告**: [GitHub Issues](https://github.com/flyGetHu/service-vitals/issues)
- 💬 **讨论**: [GitHub Discussions](https://github.com/flyGetHu/service-vitals/discussions)
- 📧 **邮件**: <970780868@qq.com>

### 常见问题

#### Q: 如何配置多个飞书群组通知？

A: 可以为每个服务单独配置 `feishu_webhook_url`，或使用全局配置作为默认值。

#### Q: 支持哪些HTTP认证方式？

A: 目前支持Bearer Token、Basic Auth和自定义Header认证。

#### Q: 如何监控内网服务？

A: Service Vitals支持监控任何可访问的HTTP/HTTPS服务，包括内网地址。

#### Q: 配置文件支持环境变量吗？

A: 是的，配置文件中可以使用 `${VARIABLE_NAME}` 语法引用环境变量。

### 路线图

- [ ] 支持更多通知渠道（邮件、Slack、钉钉）
- [ ] 添加数据库健康检测支持
- [ ] 实现分布式监控节点
- [ ] 支持自定义检测脚本
- [ ] 添加移动端应用
- [ ] 集成更多监控系统

---

**⭐ 如果这个项目对您有帮助，请给我们一个Star！**

[![GitHub stars](https://img.shields.io/github/stars/flyGetHu/service-vitals.svg?style=social&label=Star)](https://github.com/flyGetHu/service-vitals)

## 🔗 相关链接

- **GitHub仓库**: <https://github.com/flyGetHu/service-vitals>
- **发布页面**: <https://github.com/flyGetHu/service-vitals/releases>
- **问题追踪**: <https://github.com/flyGetHu/service-vitals/issues>
- **贡献指南**: <https://github.com/flyGetHu/service-vitals/blob/main/CONTRIBUTING.md>
- **更新日志**: <https://github.com/flyGetHu/service-vitals/blob/main/CHANGELOG.md>
