# Service Vitals

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70+-blue.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20macOS%20%7C%20Windows-lightgrey.svg)](https://github.com/flyGetHu/service-vitals)

一个跨平台的服务健康监控工具，支持HTTP/HTTPS服务检测和实时告警通知。

## 🚀 项目概述

Service Vitals 是一个使用Rust开发的现代化服务健康监控解决方案，专为企业级应用设计。它提供了完整的服务监控生态系统，包括：

- **实时健康检测** - 支持HTTP/HTTPS服务的定期健康检查
- **智能告警系统** - 集成飞书webhook，支持自定义消息模板
- **配置热重载** - 无需重启即可更新监控配置
- **跨平台支持** - 原生支持Linux、macOS和Windows
- **守护进程模式** - 支持系统服务安装和后台运行
- **Web监控界面** - 实时状态监控面板和RESTful API

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

### 🖥️ 守护进程支持
- Linux/macOS/Windows系统服务集成
- 进程生命周期管理
- 优雅关闭和信号处理

### 🌐 Web监控界面
- 响应式仪表板设计
- 实时状态更新
- 服务详情展示
- RESTful API接口
- 可配置的显示选项

### 🔔 告警通知系统
- 飞书webhook集成
- 自定义消息模板（Handlebars语法）
- 告警去重和频率控制
- 多通知渠道支持（规划中）

## 📦 安装指南

### 系统要求
- **操作系统**: Linux (Ubuntu 18.04+, CentOS 7+), macOS 10.15+, Windows 10+
- **内存**: 最少64MB RAM
- **磁盘空间**: 最少50MB可用空间
- **网络**: 需要访问被监控服务的网络连接

### 预编译二进制文件安装

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

#### Windows (PowerShell)
```powershell
# 下载最新版本 (Windows)
Invoke-WebRequest -Uri "https://github.com/flyGetHu/service-vitals/releases/latest/download/service-vitals-windows.exe" -OutFile "service-vitals.exe"

# 移动到系统路径
Move-Item -Path ".\service-vitals.exe" -Destination "$env:USERPROFILE\AppData\Local\Microsoft\WindowsApps"
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

# Web界面配置
[global.web]
enabled = true
port = 8080
bind_address = "0.0.0.0"
show_problems_only = false
layout_type = "cards"
refresh_interval_seconds = 3

# 全局请求头（可选）
[global.headers]
"User-Agent" = "ServiceVitals/1.0"
"Accept" = "application/json"



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
alert_cooldown_secs = 60  # 可选，告警最小间隔（秒），时间退避，默认60

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

[[services]]
name = "示例服务"
url = "https://httpbin.org/status/200"
expected_status_codes = [200]
```

### 配置参数说明

| 参数                      | 类型   | 默认值 | 说明                                                 |
| ------------------------- | ------ | ------ | ---------------------------------------------------- |
| `check_interval_seconds`  | u64    | 60     | 全局检测间隔（秒）                                   |
| `request_timeout_seconds` | u64    | 10     | 请求超时时间（秒）                                   |
| `max_concurrent_checks`   | usize  | 50     | 最大并发检测数                                       |
| `alert_cooldown_secs`     | u64    | 60     | （服务级）告警最小间隔（秒），时间退避，防止频繁告警 |
| `retry_attempts`          | u32    | 3      | 失败重试次数                                         |
| `retry_delay_seconds`     | u64    | 5      | 重试间隔（秒）                                       |
| `log_level`               | String | "info" | 日志级别                                             |
| `failure_threshold`       | u32    | 1      | 失败阈值                                             |
| `enabled`                 | bool   | true   | 是否启用服务                                         |

### Web界面配置参数

| 参数                      | 类型   | 默认值 | 说明                                                 |
| ------------------------- | ------ | ------ | ---------------------------------------------------- |
| `enabled`                 | bool   | false  | 是否启用Web界面                                      |
| `port`                    | u16    | 8080   | Web服务监听端口                                      |
| `bind_address`            | String | "0.0.0.0" | Web服务绑定地址                                    |
| `show_problems_only`      | bool   | false  | 是否只显示有问题的服务                               |
| `layout_type`             | String | "cards" | 界面布局类型（cards/table）                        |
| `refresh_interval_seconds`| u64    | 3      | 状态刷新间隔（秒）                                   |

## 🎯 使用教程

### CLI命令概览

Service Vitals提供了完整的命令行界面，支持以下主要命令：

```bash
service-vitals [OPTIONS] <COMMAND>
```

### 基本命令

#### 初始化配置
```bash
service-vitals init --template minimal
```

#### 验证配置
```bash
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

#### 重启服务
```bash
# 前台重启
service-vitals restart --foreground

# 后台重启
service-vitals restart

# 指定超时时间
service-vitals restart --timeout 60
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
service-vitals install --service-name "service-vitals"
```

#### 启动系统服务
```bash
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

#### 查看版本信息
```bash
# 文本格式
service-vitals version

# JSON格式
service-vitals version --format json
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

## 🛠️ 开发指南

### 项目结构

```text
service-vitals/
├── src/
│   ├── main.rs                 # 程序入口点
│   ├── lib.rs                  # 库入口，导出公共接口
│   ├── cli/                    # CLI命令模块
│   │   ├── mod.rs
│   │   ├── args.rs             # 命令行参数解析
│   │   └── commands.rs         # 命令定义和处理
│   ├── config/                 # 配置管理模块
│   │   ├── mod.rs
│   │   ├── types.rs            # 配置数据结构
│   │   ├── loader.rs           # 配置文件加载
│   │   ├── manager.rs          # 配置管理器
│   │   └── watcher.rs          # 配置文件热重载
│   ├── health/                 # 健康检测模块
│   │   ├── mod.rs
│   │   ├── checker.rs          # 健康检测核心逻辑
│   │   ├── scheduler.rs        # 检测任务调度
│   │   └── result.rs           # 检测结果数据结构
│   ├── notification/           # 通知系统模块
│   │   ├── mod.rs
│   │   ├── feishu.rs           # 飞书webhook通知
│   │   ├── sender.rs           # 通知发送器
│   │   └── template.rs         # 消息模板引擎
│   ├── web/                    # Web界面模块
│   │   ├── mod.rs
│   │   └── handlers.rs         # Web处理器
│   ├── daemon/                 # 守护进程模块
│   │   ├── mod.rs
│   │   ├── service_manager.rs  # 服务管理器
│   │   ├── signal_handler.rs   # 信号处理器
│   │   └── unix.rs             # Unix系统守护进程
│   ├── core/                   # 核心应用模块
│   │   ├── mod.rs
│   │   ├── app.rs              # 应用程序入口
│   │   ├── service.rs          # 服务管理
│   │   ├── daemon_service.rs   # 守护进程服务
│   │   └── foreground_service.rs # 前台服务
│   ├── common/                 # 通用功能模块
│   │   ├── mod.rs
│   │   ├── error.rs            # 错误处理
│   │   ├── logging.rs          # 日志系统
│   │   └── status.rs           # 状态管理
├── examples/                   # 配置示例
├── docs/                       # 文档
├── tests/                      # 测试文件
├── benches/                    # 基准测试
├── Cargo.toml                  # 项目配置
└── README.md                   # 项目说明
```

### 核心架构

Service Vitals采用模块化架构设计，主要组件包括：

1. **配置管理** - 支持TOML配置文件和热重载
2. **健康检测** - 异步HTTP检测和结果处理
3. **任务调度** - 基于tokio的并发任务调度
4. **通知系统** - 可扩展的通知渠道支持
5. **Web服务** - 基于axum的HTTP服务器
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

# 运行基准测试
cargo bench

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

#### Q: 如何访问Web监控界面？

A: 启用Web界面后，通过浏览器访问 `http://localhost:8080/dashboard` 查看监控面板。

### 路线图

- [ ] 支持更多通知渠道（邮件、Slack、钉钉）
- [ ] 添加数据库健康检测支持
- [ ] 实现分布式监控节点
- [ ] 支持自定义检测脚本
- [ ] 添加移动端应用
- [ ] 集成更多监控系统
- [ ] 增强Web界面功能（历史数据、图表等）

---

**⭐ 如果这个项目对您有帮助，请给我们一个Star！**

[![GitHub stars](https://img.shields.io/github/stars/flyGetHu/service-vitals.svg?style=social&label=Star)](https://github.com/flyGetHu/service-vitals)

## 🔗 相关链接

- **GitHub仓库**: <https://github.com/flyGetHu/service-vitals>
- **发布页面**: <https://github.com/flyGetHu/service-vitals/releases>
- **问题追踪**: <https://github.com/flyGetHu/service-vitals/issues>
- **贡献指南**: <https://github.com/flyGetHu/service-vitals/blob/main/CONTRIBUTING.md>
- **更新日志**: <https://github.com/flyGetHu/service-vitals/blob/main/CHANGELOG.md>

## ⚙️ Git 提交钩子

本仓库在 `githooks/` 目录下提供跨平台 `pre-commit` 钩子：

```powershell
# Windows (PowerShell)
.\githooks\pre-commit.ps1
```
```bash
# Linux/macOS (Bash)
./githooks/pre-commit
```

启用方式：

```powershell
# Windows (PowerShell)
# 设置 hooksPath 指向 githooks 目录
git config core.hooksPath .\githooks
# 给予执行权限（Git Bash）
chmod +x .\githooks\pre-commit
```
```bash
# Linux/macOS (Bash)
# 设置 hooksPath 指向 githooks 目录
git config core.hooksPath ./githooks
# 给予执行权限
chmod +x ./githooks/pre-commit
```

每次 `git commit` 将自动执行：
1. `cargo fmt --all -- --check`  ─ 格式未通过则中止提交。
2. `cargo clippy --all-targets --all-features -- -D warnings`  ─ 存在警告亦中止提交。
