//! 日志系统模块
//!
//! 提供结构化日志配置和管理功能

use log::LevelFilter;
use std::path::PathBuf;

/// 日志配置结构
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// 日志级别
    pub level: LevelFilter,
    /// 日志文件路径（可选）
    pub file_path: Option<PathBuf>,
    /// 是否输出到控制台
    pub console: bool,
    /// 是否使用JSON格式
    pub json_format: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: LevelFilter::Info,
            file_path: None,
            console: true,
            json_format: false,
        }
    }
}

/// 日志系统管理器
pub struct LoggingSystem;

impl LoggingSystem {
    /// 初始化日志系统
    ///
    /// # 参数
    /// * `config` - 日志配置
    ///
    /// # 返回
    /// * `Result<(), anyhow::Error>` - 初始化结果
    pub fn setup_logging(config: &LogConfig) -> anyhow::Result<()> {
        let mut builder = env_logger::Builder::new();
        
        // 设置日志级别
        builder.filter_level(config.level);
        
        // 设置日志格式
        if config.json_format {
            builder.format(|buf, record| {
                use std::io::Write;
                writeln!(
                    buf,
                    r#"{{"timestamp":"{}","level":"{}","target":"{}","message":"{}"}}"#,
                    chrono::Utc::now().to_rfc3339(),
                    record.level(),
                    record.target(),
                    record.args()
                )
            });
        } else {
            builder.format(|buf, record| {
                use std::io::Write;
                writeln!(
                    buf,
                    "{} [{}] {} - {}",
                    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                    record.level(),
                    record.target(),
                    record.args()
                )
            });
        }
        
        // 初始化日志系统
        builder.try_init()?;
        
        log::info!("日志系统初始化完成");
        log::debug!("日志配置: {:?}", config);
        
        Ok(())
    }
}

/// 获取默认日志文件路径
pub fn get_default_log_path() -> PathBuf {
    #[cfg(unix)]
    {
        PathBuf::from("/var/log/service-vitals/service-vitals.log")
    }
    
    #[cfg(windows)]
    {
        if let Some(data_dir) = dirs::data_local_dir() {
            data_dir.join("ServiceVitals").join("logs").join("service-vitals.log")
        } else {
            PathBuf::from("C:\\ProgramData\\ServiceVitals\\logs\\service-vitals.log")
        }
    }
}
