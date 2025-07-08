//! 配置加载器实现
//!
//! 提供TOML配置文件解析、环境变量替换和错误处理功能

use crate::config::types::{validate_config, Config};
use crate::error::{ConfigError, Result};
use async_trait::async_trait;
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

/// 配置加载器trait，定义配置加载接口
#[async_trait]
pub trait ConfigLoader: Send + Sync {
    /// 从文件加载配置
    ///
    /// # 参数
    /// * `path` - 配置文件路径
    ///
    /// # 返回
    /// * `Result<Config>` - 加载的配置或错误
    async fn load_from_file<P: AsRef<Path> + Send>(&self, path: P) -> Result<Config>;

    /// 从字符串加载配置
    ///
    /// # 参数
    /// * `content` - 配置文件内容
    ///
    /// # 返回
    /// * `Result<Config>` - 加载的配置或错误
    async fn load_from_string(&self, content: &str) -> Result<Config>;

    /// 验证配置
    ///
    /// # 参数
    /// * `config` - 要验证的配置
    ///
    /// # 返回
    /// * `Result<()>` - 验证结果
    fn validate(&self, config: &Config) -> Result<()>;
}

/// TOML配置加载器实现
#[derive(Debug, Clone)]
pub struct TomlConfigLoader {
    /// 是否启用环境变量替换
    enable_env_substitution: bool,
    /// 环境变量缓存
    env_cache: HashMap<String, String>,
}

impl TomlConfigLoader {
    /// 创建新的TOML配置加载器
    ///
    /// # 参数
    /// * `enable_env_substitution` - 是否启用环境变量替换
    ///
    /// # 返回
    /// * `Self` - 配置加载器实例
    pub fn new(enable_env_substitution: bool) -> Self {
        Self {
            enable_env_substitution,
            env_cache: HashMap::new(),
        }
    }

    /// 替换字符串中的环境变量
    ///
    /// # 参数
    /// * `content` - 要处理的字符串
    ///
    /// # 返回
    /// * `Result<String>` - 替换后的字符串或错误
    fn substitute_env_vars(&self, content: &str) -> Result<String> {
        if !self.enable_env_substitution {
            return Ok(content.to_string());
        }

        // 匹配 ${VAR_NAME} 格式的环境变量
        let env_var_regex = Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}")
            .map_err(|e| ConfigError::ParseError(format!("正则表达式错误: {}", e)))?;

        let mut result = content.to_string();

        for captures in env_var_regex.captures_iter(content) {
            let full_match = &captures[0];
            let var_name = &captures[1];

            match std::env::var(var_name) {
                Ok(value) => {
                    result = result.replace(full_match, &value);
                }
                Err(_) => {
                    return Err(ConfigError::EnvVarError {
                        var: var_name.to_string(),
                    }
                    .into());
                }
            }
        }

        Ok(result)
    }

    /// 解析TOML内容
    ///
    /// # 参数
    /// * `content` - TOML内容
    ///
    /// # 返回
    /// * `Result<Config>` - 解析的配置或错误
    fn parse_toml(&self, content: &str) -> Result<Config> {
        // 替换环境变量
        let processed_content = self.substitute_env_vars(content)?;

        // 解析TOML
        let config: Config = toml::from_str(&processed_content)
            .map_err(|e| ConfigError::ParseError(format!("TOML解析失败: {}", e)))?;

        Ok(config)
    }
}

#[async_trait]
impl ConfigLoader for TomlConfigLoader {
    async fn load_from_file<P: AsRef<Path> + Send>(&self, path: P) -> Result<Config> {
        let path = path.as_ref();

        // 检查文件是否存在
        if !path.exists() {
            return Err(ConfigError::FileNotFound {
                path: path.to_string_lossy().to_string(),
            }
            .into());
        }

        // 读取文件内容
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ConfigError::ParseError(format!("读取文件失败: {}", e)))?;

        // 解析配置
        let config = self.parse_toml(&content)?;

        // 验证配置
        self.validate(&config)?;

        log::info!("成功加载配置文件: {}", path.display());
        log::debug!("配置内容: {:?}", config);

        Ok(config)
    }

    async fn load_from_string(&self, content: &str) -> Result<Config> {
        // 解析配置
        let config = self.parse_toml(content)?;

        // 验证配置
        self.validate(&config)?;

        log::debug!("成功解析配置字符串");

        Ok(config)
    }

    fn validate(&self, config: &Config) -> Result<()> {
        validate_config(config).map_err(|e| ConfigError::ValidationError(e).into())
    }
}

/// 获取默认配置文件路径
pub fn get_default_config_path() -> std::path::PathBuf {
    #[cfg(unix)]
    {
        // Linux/macOS: ~/.config/service-vitals/config.toml 或 当前目录/config.toml
        // 先检测当前目录是否存在config.toml，不存在则检测~/.config/service-vitals/config.toml
        if std::path::Path::new("config.toml").exists() {
            std::path::PathBuf::from("config.toml")
        } else {
            dirs::config_dir()
                .map(|config_dir| config_dir.join("service-vitals").join("config.toml"))
                .unwrap_or_else(|| std::path::PathBuf::from("config.toml"))
        }
    }

    #[cfg(not(unix))]
    {
        // Windows: %APPDATA%\service-vitals\config.toml
        if let Some(config_dir) = dirs::config_dir() {
            config_dir.join("service-vitals").join("config.toml")
        } else {
            std::path::PathBuf::from("config.toml")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    const TEST_CONFIG_TOML: &str = r#"
[global]
check_interval_seconds = 30
log_level = "info"
request_timeout_seconds = 10
max_concurrent_checks = 50

[[services]]
name = "Test Service"
url = "https://example.com/health"
method = "GET"
expected_status_codes = [200, 201]
enabled = true
"#;

    const TEST_CONFIG_WITH_ENV_VARS: &str = r#"
[global]
check_interval_seconds = 30
log_level = "info"
default_feishu_webhook_url = "${WEBHOOK_URL}"

[[services]]
name = "Test Service"
url = "https://example.com/health"
method = "GET"
expected_status_codes = [200]
enabled = true

[services.headers]
"Authorization" = "Bearer ${API_TOKEN}"
"#;

    #[tokio::test]
    async fn test_toml_parsing() {
        let loader = TomlConfigLoader::new(false);
        let config = loader.load_from_string(TEST_CONFIG_TOML).await.unwrap();

        assert_eq!(config.global.check_interval_seconds, 30);
        assert_eq!(config.global.log_level, "info");
        assert_eq!(config.services.len(), 1);
        assert_eq!(config.services[0].name, "Test Service");
        assert_eq!(config.services[0].expected_status_codes, vec![200, 201]);
    }

    #[tokio::test]
    async fn test_env_var_substitution() {
        // 设置测试环境变量
        env::set_var("WEBHOOK_URL", "https://test.webhook.url");
        env::set_var("API_TOKEN", "test-token-123");

        let loader = TomlConfigLoader::new(true);
        let config = loader
            .load_from_string(TEST_CONFIG_WITH_ENV_VARS)
            .await
            .unwrap();

        assert_eq!(
            config.global.default_feishu_webhook_url,
            Some("https://test.webhook.url".to_string())
        );
        assert_eq!(
            config.services[0].headers.get("Authorization"),
            Some(&"Bearer test-token-123".to_string())
        );

        // 清理环境变量
        env::remove_var("WEBHOOK_URL");
        env::remove_var("API_TOKEN");
    }

    #[tokio::test]
    async fn test_env_var_substitution_missing_var() {
        let config_with_missing_var = r#"
[global]
default_feishu_webhook_url = "${MISSING_VAR}"

[[services]]
name = "Test"
url = "https://example.com"
expected_status_codes = [200]
"#;

        let loader = TomlConfigLoader::new(true);
        let result = loader.load_from_string(config_with_missing_var).await;

        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("MISSING_VAR"));
        }
    }

    #[tokio::test]
    async fn test_load_sample_config() {
        let loader = TomlConfigLoader::new(false);

        // 测试加载示例配置文件
        let config_path = "examples/minimal_config.toml";
        if std::path::Path::new(config_path).exists() {
            let result = loader.load_from_file(config_path).await;
            assert!(result.is_ok());

            let config = result.unwrap();
            assert!(!config.services.is_empty());
            assert_eq!(config.services[0].name, "示例服务");
        }
    }

    #[test]
    fn test_substitute_env_vars_disabled() {
        let loader = TomlConfigLoader::new(false);
        let content = "test ${VAR} content";
        let result = loader.substitute_env_vars(content).unwrap();
        assert_eq!(result, content);
    }

    #[test]
    fn test_get_default_config_path() {
        let path = get_default_config_path();
        assert!(path.is_absolute());
        assert!(path.to_string_lossy().contains("config.toml"));

        assert!(path.to_string_lossy().contains("service-vitals"));
    }
}
