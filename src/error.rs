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

    /// 守护进程相关错误
    #[error("守护进程错误: {0}")]
    DaemonError(String),

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

/// 错误严重程度
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// 低级错误 - 不影响核心功能
    Low,
    /// 中级错误 - 影响部分功能
    Medium,
    /// 高级错误 - 影响核心功能
    High,
    /// 致命错误 - 系统无法继续运行
    Critical,
}

/// 错误恢复策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryStrategy {
    /// 不需要恢复
    None,
    /// 重试操作
    Retry,
    /// 跳过当前操作
    Skip,
    /// 使用默认值
    UseDefault,
    /// 重启组件
    RestartComponent,
    /// 停止服务
    StopService,
}

/// 错误上下文信息
#[derive(Debug, Clone)]
pub struct ErrorContext {
    /// 操作名称
    pub operation: String,
    /// 服务名称（如果适用）
    pub service_name: Option<String>,
    /// 组件名称
    pub component: String,
    /// 时间戳
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// 额外的上下文信息
    pub metadata: std::collections::HashMap<String, String>,
}

impl ErrorContext {
    /// 创建新的错误上下文
    pub fn new(operation: impl Into<String>, component: impl Into<String>) -> Self {
        Self {
            operation: operation.into(),
            service_name: None,
            component: component.into(),
            timestamp: chrono::Utc::now(),
            metadata: std::collections::HashMap::new(),
        }
    }

    /// 设置服务名称
    pub fn with_service(mut self, service_name: impl Into<String>) -> Self {
        self.service_name = Some(service_name.into());
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

impl ServiceVitalsError {
    /// 获取错误的严重程度
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            ServiceVitalsError::Config(_) => ErrorSeverity::High,
            ServiceVitalsError::HealthCheck(e) => match e {
                HealthCheckError::Timeout => ErrorSeverity::Medium,
                HealthCheckError::ConnectionError { .. } => ErrorSeverity::Medium,
                HealthCheckError::StatusCodeMismatch { .. } => ErrorSeverity::Low,
                HealthCheckError::RequestError(_) => ErrorSeverity::Medium,
            },
            ServiceVitalsError::Notification(_) => ErrorSeverity::Low,
            ServiceVitalsError::DaemonError(_) => ErrorSeverity::High,
            ServiceVitalsError::Io(_) => ErrorSeverity::High,
            ServiceVitalsError::Json(_) => ErrorSeverity::Medium,
            ServiceVitalsError::Other(_) => ErrorSeverity::Medium,
        }
    }

    /// 获取推荐的恢复策略
    pub fn recovery_strategy(&self) -> RecoveryStrategy {
        match self {
            ServiceVitalsError::Config(_) => RecoveryStrategy::StopService,
            ServiceVitalsError::HealthCheck(e) => match e {
                HealthCheckError::Timeout => RecoveryStrategy::Retry,
                HealthCheckError::ConnectionError { .. } => RecoveryStrategy::Retry,
                HealthCheckError::StatusCodeMismatch { .. } => RecoveryStrategy::Skip,
                HealthCheckError::RequestError(_) => RecoveryStrategy::Retry,
            },
            ServiceVitalsError::Notification(_) => RecoveryStrategy::Retry,
            ServiceVitalsError::DaemonError(_) => RecoveryStrategy::StopService,
            ServiceVitalsError::Io(_) => RecoveryStrategy::UseDefault,
            ServiceVitalsError::Json(_) => RecoveryStrategy::Skip,
            ServiceVitalsError::Other(_) => RecoveryStrategy::None,
        }
    }

    /// 是否可以重试
    pub fn is_retryable(&self) -> bool {
        matches!(
            self.recovery_strategy(),
            RecoveryStrategy::Retry | RecoveryStrategy::UseDefault
        )
    }

    /// 是否是致命错误
    pub fn is_fatal(&self) -> bool {
        matches!(self.severity(), ErrorSeverity::Critical)
    }

    /// 创建带上下文的错误
    pub fn with_context(self, context: ErrorContext) -> ContextualError {
        ContextualError {
            error: self,
            context,
        }
    }
}

/// 带上下文的错误
#[derive(Debug)]
pub struct ContextualError {
    /// 原始错误
    pub error: ServiceVitalsError,
    /// 错误上下文
    pub context: ErrorContext,
}

impl std::fmt::Display for ContextualError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{}] {} in {}: {}",
            self.context.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            self.context.operation,
            self.context.component,
            self.error
        )?;

        if let Some(ref service_name) = self.context.service_name {
            write!(f, " (service: {service_name})")?;
        }

        if !self.context.metadata.is_empty() {
            write!(f, " - metadata: {:?}", self.context.metadata)?;
        }

        Ok(())
    }
}

impl std::error::Error for ContextualError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}
