//! 错误处理模块
//!
//! 定义应用程序的统一错误类型

use thiserror::Error;

/// Service Vitals 应用程序的主要错误类型
#[derive(Error, Debug)]
pub enum ServiceVitalsError {
    /// 配置相关错误
    #[error("配置错误: {0}")]
    Config(#[from] ConfigError),

    /// 健康检测相关错误
    #[error("健康检测错误: {0}")]
    HealthCheck(#[from] HealthCheckError),

    /// 通知相关错误
    #[error("通知错误: {0}")]
    Notification(#[from] NotificationError),

    /// IO错误
    #[error("IO错误: {0}")]
    Io(#[from] std::io::Error),

    /// JSON序列化/反序列化错误
    #[error("JSON错误: {0}")]
    Json(#[from] serde_json::Error),

    /// 其他错误
    #[error("其他错误: {0}")]
    Other(#[from] anyhow::Error),
}

/// 配置错误类型
#[derive(Error, Debug)]
pub enum ConfigError {
    /// 配置文件解析错误
    #[error("配置文件解析失败: {0}")]
    ParseError(String),

    /// 配置验证错误
    #[error("配置验证失败: {0}")]
    ValidationError(String),

    /// 配置文件不存在
    #[error("配置文件不存在: {path}")]
    FileNotFound { path: String },

    /// 环境变量替换错误
    #[error("环境变量替换失败: {var}")]
    EnvVarError { var: String },
}

/// 健康检测错误类型
#[derive(Error, Debug)]
pub enum HealthCheckError {
    /// HTTP请求错误
    #[error("HTTP请求失败: {0}")]
    RequestError(#[from] reqwest::Error),

    /// 超时错误
    #[error("请求超时")]
    Timeout,

    /// 状态码不匹配
    #[error("状态码不匹配: 期望 {expected:?}, 实际 {actual}")]
    StatusCodeMismatch { expected: Vec<u16>, actual: u16 },

    /// 连接错误
    #[error("连接失败: {url}")]
    ConnectionError { url: String },
}

/// 通知错误类型
#[derive(Error, Debug)]
pub enum NotificationError {
    /// 发送失败
    #[error("通知发送失败: {0}")]
    SendError(String),

    /// 模板渲染错误
    #[error("模板渲染失败: {0}")]
    TemplateError(String),

    /// 配置错误
    #[error("通知配置错误: {0}")]
    ConfigError(String),
}

/// 结果类型别名
pub type Result<T> = std::result::Result<T, ServiceVitalsError>;
