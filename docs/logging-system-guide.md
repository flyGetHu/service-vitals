# Service Vitals 日志系统指南

## 🎯 概述

Service Vitals 采用现代化的日志系统设计，基于 `tracing` 和 `tracing-subscriber` 生态系统，提供：

- **线程安全的单次初始化**：避免重复初始化警告
- **优雅的错误处理**：清晰的错误信息和恢复机制
- **测试友好**：支持测试环境中的重新初始化
- **高性能**：零开销的日志记录
- **灵活配置**：支持多种输出格式和级别控制

## 🛠️ 核心特性

### 1. 现代化初始化模式

```rust
use service_vitals::logging::{LogConfig, LoggingSystem};
use log::LevelFilter;

// 基本初始化
let config = LogConfig {
    level: LevelFilter::Info,
    console: true,
    json_format: false,
    // ... 其他配置
};

let logging_system = LoggingSystem::setup_logging(config)?;
```

### 2. 线程安全的全局状态管理

- 使用 `std::sync::OnceLock` 和 `Mutex` 确保线程安全
- 避免使用 `unsafe` 代码
- 支持多次调用而不会产生警告

### 3. 智能错误处理

```rust
// 自动检测并处理重复初始化
match LoggingSystem::setup_logging(config) {
    Ok(system) => {
        // 初始化成功或已经初始化过
        println!("日志系统就绪");
    }
    Err(e) => {
        // 真正的错误（非重复初始化）
        eprintln!("日志系统初始化失败: {}", e);
    }
}
```

## 📋 配置选项

### 基本配置

```rust
use service_vitals::logging::{LogConfig, LogRotation};
use std::collections::HashMap;
use std::time::Duration;

let config = LogConfig {
    level: LevelFilter::Info,           // 全局日志级别
    file_path: None,                    // 可选的文件输出路径
    console: true,                      // 是否输出到控制台
    json_format: false,                 // 是否使用JSON格式
    rotation: LogRotation::Never,       // 日志轮转策略
    max_files: 5,                       // 最大保留文件数
    module_levels: HashMap::new(),      // 模块级别控制
    enable_metrics: false,              // 是否启用性能指标
    metrics_interval: Duration::from_secs(60), // 指标收集间隔
};
```

### 高级配置

```rust
// 模块级别日志控制
let mut module_levels = HashMap::new();
module_levels.insert("service_vitals::health".to_string(), LevelFilter::Debug);
module_levels.insert("service_vitals::notification".to_string(), LevelFilter::Warn);

let config = LogConfig {
    level: LevelFilter::Info,
    console: true,
    json_format: true,                  // JSON格式便于日志分析
    module_levels,                      // 精细化控制
    enable_metrics: true,               // 启用性能监控
    metrics_interval: Duration::from_secs(30),
    ..Default::default()
};
```

## 🧪 测试支持

### 测试环境初始化

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_with_logging() {
        // 重置测试状态
        LoggingSystem::reset_for_testing();
        
        let config = create_test_config();
        let _system = LoggingSystem::setup_logging(config).unwrap();
        
        // 测试代码...
    }

    #[tokio::test]
    async fn test_force_reinit() {
        let config = create_test_config();
        
        // 强制重新初始化（用于测试不同配置）
        let _system = LoggingSystem::setup_logging_with_options(config, true).unwrap();
        
        // 测试代码...
    }
}
```

## 🔧 最佳实践

### 1. 应用程序启动

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // 尽早初始化日志系统
    let log_config = LogConfig {
        level: args.log_level.into(),
        console: true,
        json_format: false,
        ..Default::default()
    };

    let _logging_system = LoggingSystem::setup_logging(log_config)
        .context("初始化日志系统失败")?;

    info!("应用程序启动");
    
    // 应用程序逻辑...
    
    Ok(())
}
```

### 2. 库中的使用

```rust
// 在库代码中，不要初始化日志系统
// 只使用 tracing 宏记录日志
use tracing::{info, warn, error, debug, trace};

pub fn library_function() {
    debug!("库函数开始执行");
    
    // 业务逻辑...
    
    info!("库函数执行完成");
}
```

### 3. 错误处理

```rust
// 检查日志系统状态
if LoggingSystem::is_initialized() {
    info!("日志系统已就绪");
} else {
    eprintln!("警告：日志系统未初始化");
}

// 获取当前配置
if let Some(config) = LoggingSystem::current_config() {
    println!("当前日志级别: {:?}", config.level);
}
```

## 🚀 性能优化

### 1. 条件编译

```rust
// 在发布版本中禁用调试日志
#[cfg(debug_assertions)]
let log_level = LevelFilter::Debug;
#[cfg(not(debug_assertions))]
let log_level = LevelFilter::Info;
```

### 2. 异步日志记录

```rust
// 启用指标收集（异步执行）
let config = LogConfig {
    enable_metrics: true,
    metrics_interval: Duration::from_secs(60),
    ..Default::default()
};

let system = LoggingSystem::setup_logging(config)?;
// 指标收集任务会自动在后台运行
```

## 🔍 故障排除

### 常见问题

1. **重复初始化警告**
   - ✅ 已解决：新系统自动处理重复初始化
   - 不再出现 "attempted to set a logger" 警告

2. **测试中的日志冲突**
   - ✅ 使用 `LoggingSystem::reset_for_testing()` 重置状态
   - ✅ 使用 `setup_logging_with_options(config, true)` 强制重新初始化

3. **性能问题**
   - ✅ 使用适当的日志级别
   - ✅ 避免在热路径中使用高级别日志

### 调试技巧

```bash
# Linux/macOS (Bash)
# 启用详细日志
RUST_LOG=debug ./service-vitals start --foreground

# 启用特定模块的调试日志
RUST_LOG=service_vitals::health=debug ./service-vitals start --foreground
```

```powershell
# Windows (PowerShell)
# 启用详细日志
$env:RUST_LOG="debug"; .\service-vitals.exe start --foreground
```

## 📚 相关资源

- [tracing 文档](https://docs.rs/tracing/)
- [tracing-subscriber 文档](https://docs.rs/tracing-subscriber/)
- [Rust 日志最佳实践](https://rust-lang-nursery.github.io/rust-cookbook/development_tools/debugging/log.html)
