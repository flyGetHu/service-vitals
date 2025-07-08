# 🔍 Service Vitals

[![CI](https://github.com/flyGetHu/service-vitals/workflows/CI/badge.svg)](https://github.com/flyGetHu/service-vitals/actions)
[![codecov](https://codecov.io/gh/flyGetHu/service-vitals/branch/master/graph/badge.svg)](https://codecov.io/gh/flyGetHu/service-vitals)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70+-blue.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-Linux-green.svg)](https://www.linux.org)

**Service Vitals** 是一个专为 **Linux 服务器环境** 设计的现代化服务健康监控和告警系统。通过实时监控 HTTP/HTTPS 服务状态，提供 Web 界面、Prometheus 指标导出和智能告警功能，为 DevOps 和 SRE 团队提供完整的服务监控解决方案。

## ✨ 核心特性

### 🔍 **智能监控**
- **HTTP/HTTPS 健康检查** - 支持 GET/POST/PUT/DELETE 等多种请求方法
- **响应时间监控** - 毫秒级精度的响应时间统计和趋势分析
- **状态码验证** - 灵活的状态码匹配规则和自定义验证逻辑
- **自定义请求** - 支持自定义请求头、请求体和认证信息
- **并发检查** - 高效的并发健康检查，支持大规模服务监控
- **重试机制** - 智能重试策略，减少误报和网络抖动影响

### 🌐 **现代化 Web 界面**
- **实时监控仪表板** - 直观的服务状态可视化界面
- **RESTful API** - 完整的 REST API 支持程序化访问
- **响应式设计** - 支持桌面和移动设备的自适应界面
- **API 密钥认证** - 安全的 API 访问控制和权限管理
- **实时更新** - 30 秒自动刷新，支持手动刷新和暂停

### 📊 **Prometheus 集成**
- **标准指标格式** - 完全兼容 Prometheus 抓取格式
- **丰富指标集合** - 健康检查计数、响应时间分布、服务状态等
- **多维度标签** - 服务名称、URL、状态等多维度标签支持
- **Grafana 就绪** - 提供预配置的 Grafana 仪表板模板

### 🔧 **Linux 系统服务**
- **systemd 集成** - 原生 systemd 服务支持，完整的生命周期管理
- **优雅关闭** - 支持 SIGINT、SIGTERM、SIGUSR1 信号处理
- **自动重启** - 服务故障时自动重启和恢复机制
- **日志管理** - 集成 systemd 日志，支持 journalctl 查看
- **权限控制** - 支持非特权用户运行，增强安全性

### 📢 **智能告警**
- **飞书 Webhook** - 原生飞书机器人集成，支持富文本消息
- **自定义模板** - 灵活的消息模板系统，支持变量替换
- **告警节流** - 防止告警风暴的智能节流机制
- **恢复通知** - 服务恢复时的自动通知功能

## 🚀 快速开始

### 📋 系统要求

- **操作系统**: Linux (Ubuntu 18.04+, CentOS 7+, Debian 9+)
- **架构**: x86_64 (amd64)
- **Rust**: 1.70+ (仅编译时需要)
- **系统权限**: 普通用户权限即可运行

### 📦 安装方式

#### 方式一：从源码编译 (推荐)

```bash
# 克隆仓库
git clone https://github.com/flyGetHu/service-vitals.git
cd service-vitals

# 编译发布版本
cargo build --release

# 复制到系统路径
sudo cp target/release/service-vitals /usr/local/bin/

# 验证安装
service-vitals --version
```

#### 方式二：下载预编译二进制

```bash
# 下载最新版本 (替换为实际版本号)
wget https://github.com/flyGetHu/service-vitals/releases/download/v0.1.0/service-vitals-linux-x86_64

# 添加执行权限
chmod +x service-vitals-linux-x86_64

# 移动到系统路径
sudo mv service-vitals-linux-x86_64 /usr/local/bin/service-vitals
```

### ⚙️ 基础配置

创建配置文件 `/etc/service-vitals/config.toml`:

```toml
# 全局配置
[global]
check_interval_seconds = 60
request_timeout_seconds = 10
max_concurrent_checks = 50
log_level = "info"

# Web 界面配置
[web]
enabled = true
bind_address = "0.0.0.0"
port = 8080
api_key = "your-secure-api-key-here"

# 服务监控配置
[[services]]
name = "example-api"
url = "https://api.example.com/health"
method = "GET"
expected_status_codes = [200]
enabled = true

[[services]]
name = "example-web"
url = "https://www.example.com"
method = "GET"
expected_status_codes = [200, 301, 302]
enabled = true
```

### 🔄 systemd 服务安装

```bash
# 安装为系统服务
sudo service-vitals install \
  --user service-vitals \
  --group service-vitals

# 启动服务
sudo systemctl start service-vitals

# 设置开机自启
sudo systemctl enable service-vitals

# 查看服务状态
sudo systemctl status service-vitals
```

## 📖 详细使用指南

### 🖥️ CLI 命令参考

#### 基础监控命令
```bash
# 运行一次性检查
service-vitals check --config /path/to/config.toml

# 启动持续监控
service-vitals start --config /path/to/config.toml

# 验证配置文件
service-vitals validate --config /path/to/config.toml

# 初始化配置文件
service-vitals init --output config.toml
```

#### 系统服务管理
```bash
# 安装 systemd 服务
service-vitals install [OPTIONS]

# 卸载 systemd 服务
service-vitals uninstall

# 启动系统服务
service-vitals start-service

# 停止系统服务
service-vitals stop-service

# 重启系统服务
service-vitals restart-service

# 查看服务状态
service-vitals service-status
```

### 🌐 Web 界面使用

启动服务后，访问 Web 界面：

- **监控仪表板**: http://localhost:8080/dashboard
- **API 文档**: http://localhost:8080/api/v1/status
- **Prometheus 指标**: http://localhost:8080/metrics

#### API 端点

| 端点 | 方法 | 描述 | 认证 |
|------|------|------|------|
| `/api/v1/status` | GET | 获取所有服务状态 | 需要 |
| `/api/v1/status/{service}` | GET | 获取特定服务状态 | 需要 |
| `/api/v1/config` | GET | 获取配置信息 | 需要 |
| `/api/v1/health` | GET | 系统健康检查 | 需要 |
| `/metrics` | GET | Prometheus 指标 | 需要 |
| `/dashboard` | GET | Web 仪表板 | 无需 |

#### API 认证示例

```bash
# 使用 Header 认证
curl -H "X-API-Key: your-api-key" http://localhost:8080/api/v1/status

# 使用查询参数认证
curl "http://localhost:8080/api/v1/status?api_key=your-api-key"
```

### 📊 Prometheus 集成

#### Prometheus 配置

在 `prometheus.yml` 中添加抓取配置：

```yaml
scrape_configs:
  - job_name: 'service-vitals'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: '/metrics'
    scrape_interval: 30s
    headers:
      X-API-Key: 'your-api-key'
```

#### 主要指标说明

```prometheus
# 健康检查总数（按服务和状态分类）
service_vitals_health_check_total{service="api-service", status="up"} 1250

# 响应时间分布（直方图）
service_vitals_response_time_seconds_bucket{service="api-service", le="0.1"} 800

# 服务状态（1=正常，0=异常）
service_vitals_up{service="api-service", url="https://api.example.com/health"} 1

# 最后检查时间戳
service_vitals_last_check_timestamp{service="api-service"} 1704672615

# 连续失败次数
service_vitals_consecutive_failures{service="api-service"} 0
```

## 🔧 高级配置

### 📝 完整配置示例

```toml
# 全局配置
[global]
check_interval_seconds = 30
request_timeout_seconds = 5
max_concurrent_checks = 100
retry_attempts = 3
retry_delay_seconds = 5
log_level = "info"

# 默认飞书 Webhook（可选）
default_feishu_webhook_url = "https://open.feishu.cn/open-apis/bot/v2/hook/your-webhook-url"

# 默认消息模板（可选）
message_template = """
🚨 服务告警通知

服务名称: {{service_name}}
服务状态: {{status}}
检查时间: {{timestamp}}
响应时间: {{response_time_ms}}ms
错误信息: {{error_message}}
"""

# Web 界面配置
[web]
enabled = true
bind_address = "0.0.0.0"
port = 8080
api_key = "your-very-secure-api-key-32-chars"
disable_auth = false
cors_enabled = true
cors_origins = ["https://monitoring.company.com"]

# 服务监控配置
[[services]]
name = "user-api"
url = "https://api.company.com/users/health"
method = "GET"
expected_status_codes = [200]
failure_threshold = 3
enabled = true

# 自定义请求头
[services.headers]
"Authorization" = "Bearer ${API_TOKEN}"
"User-Agent" = "ServiceVitals/1.0"

[[services]]
name = "payment-service"
url = "https://payment.company.com/health"
method = "POST"
expected_status_codes = [200, 201]
failure_threshold = 1
enabled = true

# 自定义请求体
body = '{"check": "health"}'

# 服务特定的飞书 Webhook
feishu_webhook_url = "https://open.feishu.cn/open-apis/bot/v2/hook/payment-webhook"

# 自定义请求头
[services.headers]
"Content-Type" = "application/json"
"X-Service-Check" = "true"
```

### 🔐 安全配置

#### API 密钥生成

```bash
# 生成安全的 API 密钥
openssl rand -hex 32

# 或使用 service-vitals 内置生成器
service-vitals generate-api-key
```

#### systemd 安全配置

Service Vitals 自动生成的 systemd 服务包含以下安全特性：

```ini
[Service]
# 安全配置
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ProtectKernelTunables=true
ProtectKernelModules=true
PrivateTmp=true
RestrictRealtime=true
RestrictSUIDSGID=true
```

### 📊 监控最佳实践

#### 检查间隔建议

| 服务类型 | 建议间隔 | 说明 |
|----------|----------|------|
| 关键业务 API | 30-60 秒 | 快速发现问题 |
| 内部服务 | 60-120 秒 | 平衡监控和性能 |
| 静态网站 | 300-600 秒 | 减少不必要的检查 |
| 第三方服务 | 300+ 秒 | 避免过度请求 |

#### 故障阈值设置

```toml
# 关键服务：快速告警
failure_threshold = 1

# 一般服务：避免误报
failure_threshold = 3

# 不稳定服务：更高容忍度
failure_threshold = 5
```

## 🐳 容器化部署

### Docker 部署

```dockerfile
# Dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/service-vitals /usr/local/bin/
COPY config.toml /etc/service-vitals/

EXPOSE 8080
CMD ["service-vitals", "start", "--config", "/etc/service-vitals/config.toml"]
```

```bash
# 构建镜像
docker build -t service-vitals:latest .

# 运行容器
docker run -d \
  --name service-vitals \
  -p 8080:8080 \
  -v $(pwd)/config.toml:/etc/service-vitals/config.toml:ro \
  service-vitals:latest
```

### Docker Compose

```yaml
# docker-compose.yml
version: '3.8'

services:
  service-vitals:
    build: .
    ports:
      - "8080:8080"
    volumes:
      - ./config.toml:/etc/service-vitals/config.toml:ro
      - ./logs:/var/log/service-vitals
    environment:
      - RUST_LOG=info
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/api/v1/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml:ro
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--web.console.libraries=/etc/prometheus/console_libraries'
      - '--web.console.templates=/etc/prometheus/consoles'

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    volumes:
      - grafana-storage:/var/lib/grafana

volumes:
  grafana-storage:
```

## 🔧 故障排除

### 常见问题解决

#### 1. 服务无法启动

```bash
# 检查配置文件语法
service-vitals validate --config /etc/service-vitals/config.toml

# 检查系统日志
sudo journalctl -u service-vitals -f

# 检查权限
ls -la /etc/service-vitals/
sudo chown -R service-vitals:service-vitals /etc/service-vitals/
```

#### 2. Web 界面无法访问

```bash
# 检查端口占用
sudo netstat -tlnp | grep :8080

# 检查防火墙
sudo ufw status
sudo ufw allow 8080

# 检查服务状态
sudo systemctl status service-vitals
```

#### 3. 监控检查失败

```bash
# 手动测试连接
curl -v https://your-service.com/health

# 检查 DNS 解析
nslookup your-service.com

# 检查证书
openssl s_client -connect your-service.com:443 -servername your-service.com
```

#### 4. 内存使用过高

```bash
# 检查内存使用
ps aux | grep service-vitals

# 调整并发检查数
# 在 config.toml 中设置
max_concurrent_checks = 20
```

### 日志分析

```bash
# 查看实时日志
sudo journalctl -u service-vitals -f

# 查看错误日志
sudo journalctl -u service-vitals -p err

# 查看特定时间段日志
sudo journalctl -u service-vitals --since "2024-01-01 00:00:00" --until "2024-01-01 23:59:59"

# 导出日志
sudo journalctl -u service-vitals --since today > service-vitals.log
```

### 性能调优

#### 系统资源优化

```toml
# config.toml 性能调优
[global]
# 根据服务器性能调整
max_concurrent_checks = 50        # CPU 核心数 * 10-20
check_interval_seconds = 60       # 平衡实时性和性能
request_timeout_seconds = 10      # 避免长时间等待

# Web 界面优化
[web]
# 仅在需要时启用
enabled = true
# 绑定到内网地址提高安全性
bind_address = "127.0.0.1"
```

#### 监控服务优化

```bash
# 系统级优化
echo 'net.core.somaxconn = 1024' | sudo tee -a /etc/sysctl.conf
echo 'net.ipv4.tcp_max_syn_backlog = 1024' | sudo tee -a /etc/sysctl.conf
sudo sysctl -p

# 文件描述符限制
echo 'service-vitals soft nofile 65536' | sudo tee -a /etc/security/limits.conf
echo 'service-vitals hard nofile 65536' | sudo tee -a /etc/security/limits.conf
```

## 🤝 贡献指南

我们欢迎社区贡献！请查看 [CONTRIBUTING.md](CONTRIBUTING.md) 了解详细信息。

### 开发环境设置

```bash
# 克隆仓库
git clone https://github.com/flyGetHu/service-vitals.git
cd service-vitals

# 安装开发依赖
cargo install cargo-watch cargo-audit

# 运行测试
cargo test

# 代码格式化
cargo fmt

# 代码检查
cargo clippy

# 监控模式开发
cargo watch -x run
```

### 提交规范

我们使用 [Conventional Commits](https://www.conventionalcommits.org/) 规范：

```
feat: 添加新功能
fix: 修复 bug
docs: 文档更新
style: 代码格式调整
refactor: 代码重构
test: 测试相关
chore: 构建过程或辅助工具的变动
```

## 📄 许可证

本项目采用 [MIT 许可证](LICENSE) 开源。

## 🙏 致谢

感谢所有为 Service Vitals 项目做出贡献的开发者和用户！

---

**Service Vitals** - 专业的 Linux 服务监控解决方案 🚀
