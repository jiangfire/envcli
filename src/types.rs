//! 核心数据结构定义 (表达原则：用数据结构表达逻辑)

use crate::error::{EnvError, Result};
use serde::{Deserialize, Serialize};
use std::fmt;

/// 环境变量来源层级
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EnvSource {
    /// 系统环境变量 (只读)
    System,
    /// 用户级配置 ~/.envcli/user.env
    User,
    /// 项目级配置 ./.envcli/project.env
    Project,
    /// 本地级配置 ./.envcli/local.env (gitignored)
    Local,
}

impl fmt::Display for EnvSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnvSource::System => write!(f, "system"),
            EnvSource::User => write!(f, "user"),
            EnvSource::Project => write!(f, "project"),
            EnvSource::Local => write!(f, "local"),
        }
    }
}

impl EnvSource {
    /// 从字符串转换
    #[allow(clippy::should_implement_trait)]
    #[must_use]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "system" => Some(EnvSource::System),
            "user" => Some(EnvSource::User),
            "project" => Some(EnvSource::Project),
            "local" => Some(EnvSource::Local),
            _ => None,
        }
    }

    /// 是否可写
    #[must_use]
    pub fn is_writable(&self) -> bool {
        !matches!(self, EnvSource::System)
    }
}

/// 环境变量条目
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
    pub source: EnvSource,
    pub timestamp: u64,
}

impl EnvVar {
    /// 创建新的环境变量条目
    ///
    /// # Panics
    ///
    /// Panics if the system time is before the UNIX epoch (extremely unlikely).
    #[must_use]
    pub fn new(key: String, value: String, source: EnvSource) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            key,
            value,
            source,
            timestamp,
        }
    }
}

/// 配置选项 (支持详细/安静模式切换)
#[derive(Debug, Clone, Default)]
pub struct Config {
    pub verbose: bool, // 是否详细输出
}

/// 输出格式类型
#[derive(Debug, Clone, PartialEq, Default)]
pub enum OutputFormat {
    #[default]
    ENV,
    JSON,
}

impl From<&str> for OutputFormat {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" | "j" => OutputFormat::JSON,
            _ => OutputFormat::ENV,
        }
    }
}

/// 加密类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EncryptionType {
    /// 明文存储
    #[default]
    None,
    /// SOPS 加密
    Sops,
}

impl fmt::Display for EncryptionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EncryptionType::None => write!(f, "none"),
            EncryptionType::Sops => write!(f, "sops"),
        }
    }
}

/// 加密的环境变量条目
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EncryptedEnvVar {
    pub key: String,
    pub value: String, // 可能是加密的或明文的
    pub source: EnvSource,
    pub timestamp: u64,
    pub encryption_type: EncryptionType,
}

impl EncryptedEnvVar {
    /// 创建新的加密环境变量条目
    ///
    /// # Panics
    ///
    /// Panics if the system time is before the UNIX epoch (extremely unlikely).
    #[must_use]
    pub fn new(
        key: String,
        value: String,
        source: EnvSource,
        encryption_type: EncryptionType,
    ) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            key,
            value,
            source,
            timestamp,
            encryption_type,
        }
    }

    /// 检查是否已加密
    #[must_use]
    pub fn is_encrypted(&self) -> bool {
        match self.encryption_type {
            EncryptionType::None => false,
            EncryptionType::Sops => {
                // 检查值是否符合 SOPS 加密格式
                self.value.starts_with("ENC[SOPS:") && self.value.ends_with(']')
            }
        }
    }

    /// 转换为普通 `EnvVar`（如果已加密则返回错误）
    ///
    /// # Errors
    ///
    /// Returns `EnvError::EncryptionError` if the variable is encrypted.
    pub fn to_env_var(&self) -> Result<EnvVar> {
        if self.is_encrypted() {
            return Err(EnvError::EncryptionError(
                "无法将加密变量转换为普通变量".to_string(),
            ));
        }

        Ok(EnvVar {
            key: self.key.clone(),
            value: self.value.clone(),
            source: self.source.clone(),
            timestamp: self.timestamp,
        })
    }
}

/// 从 `EnvVar` 转换为 EncryptedEnvVar（明文）
impl From<EnvVar> for EncryptedEnvVar {
    fn from(var: EnvVar) -> Self {
        EncryptedEnvVar::new(var.key, var.value, var.source, EncryptionType::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod env_source_tests {
        use super::*;

        #[test]
        fn test_env_source_display() {
            assert_eq!(EnvSource::System.to_string(), "system");
            assert_eq!(EnvSource::User.to_string(), "user");
            assert_eq!(EnvSource::Project.to_string(), "project");
            assert_eq!(EnvSource::Local.to_string(), "local");
        }

        #[test]
        fn test_env_source_from_str() {
            assert_eq!(EnvSource::from_str("system"), Some(EnvSource::System));
            assert_eq!(EnvSource::from_str("SYSTEM"), Some(EnvSource::System));
            assert_eq!(EnvSource::from_str("user"), Some(EnvSource::User));
            assert_eq!(EnvSource::from_str("USER"), Some(EnvSource::User));
            assert_eq!(EnvSource::from_str("project"), Some(EnvSource::Project));
            assert_eq!(EnvSource::from_str("PROJECT"), Some(EnvSource::Project));
            assert_eq!(EnvSource::from_str("local"), Some(EnvSource::Local));
            assert_eq!(EnvSource::from_str("LOCAL"), Some(EnvSource::Local));
            assert_eq!(EnvSource::from_str("invalid"), None);
            assert_eq!(EnvSource::from_str(""), None);
        }

        #[test]
        fn test_env_source_is_writable() {
            assert!(!EnvSource::System.is_writable());
            assert!(EnvSource::User.is_writable());
            assert!(EnvSource::Project.is_writable());
            assert!(EnvSource::Local.is_writable());
        }

        #[test]
        fn test_env_source_hashmap_key() {
            use std::collections::HashMap;
            let mut map = HashMap::new();
            map.insert(EnvSource::User, "user_value");
            map.insert(EnvSource::Project, "project_value");

            assert_eq!(map.get(&EnvSource::User), Some(&"user_value"));
            assert_eq!(map.get(&EnvSource::Project), Some(&"project_value"));
        }
    }

    mod env_var_tests {
        use super::*;

        #[test]
        fn test_env_var_new() {
            let var = EnvVar::new(
                "TEST_KEY".to_string(),
                "test_value".to_string(),
                EnvSource::Local,
            );

            assert_eq!(var.key, "TEST_KEY");
            assert_eq!(var.value, "test_value");
            assert_eq!(var.source, EnvSource::Local);
            assert!(var.timestamp > 0);
        }

        #[test]
        fn test_env_var_clone() {
            let var1 = EnvVar::new("KEY".to_string(), "VALUE".to_string(), EnvSource::User);
            let var2 = var1.clone();

            assert_eq!(var1.key, var2.key);
            assert_eq!(var1.value, var2.value);
            assert_eq!(var1.source, var2.source);
            assert_eq!(var1.timestamp, var2.timestamp);
        }

        #[test]
        fn test_env_var_debug() {
            let var = EnvVar::new("KEY".to_string(), "VALUE".to_string(), EnvSource::Project);
            let debug_str = format!("{:?}", var);

            assert!(debug_str.contains("KEY"));
            assert!(debug_str.contains("VALUE"));
            assert!(debug_str.contains("Project"));
        }
    }

    mod config_tests {
        use super::*;

        #[test]
        fn test_config_default() {
            let config = Config::default();
            assert!(!config.verbose);
        }

        #[test]
        fn test_config_clone() {
            let config1 = Config { verbose: true };
            let config2 = config1.clone();

            assert_eq!(config1.verbose, config2.verbose);
        }

        #[test]
        fn test_config_debug() {
            let config = Config { verbose: true };
            let debug_str = format!("{:?}", config);

            assert!(debug_str.contains("verbose"));
            assert!(debug_str.contains("true"));
        }
    }

    mod output_format_tests {
        use super::*;

        #[test]
        fn test_output_format_default() {
            let format = OutputFormat::default();
            assert_eq!(format, OutputFormat::ENV);
        }

        #[test]
        fn test_output_format_from_str() {
            assert_eq!(OutputFormat::from("env"), OutputFormat::ENV);
            assert_eq!(OutputFormat::from("ENV"), OutputFormat::ENV);
            assert_eq!(OutputFormat::from("json"), OutputFormat::JSON);
            assert_eq!(OutputFormat::from("JSON"), OutputFormat::JSON);
            assert_eq!(OutputFormat::from("j"), OutputFormat::JSON);
            assert_eq!(OutputFormat::from("J"), OutputFormat::JSON);
            assert_eq!(OutputFormat::from("invalid"), OutputFormat::ENV);
            assert_eq!(OutputFormat::from(""), OutputFormat::ENV);
        }

        #[test]
        fn test_output_format_debug() {
            let format = OutputFormat::JSON;
            let debug_str = format!("{:?}", format);
            assert_eq!(debug_str, "JSON");
        }

        #[test]
        fn test_output_format_equality() {
            assert_eq!(OutputFormat::ENV, OutputFormat::ENV);
            assert_eq!(OutputFormat::JSON, OutputFormat::JSON);
            assert_ne!(OutputFormat::ENV, OutputFormat::JSON);
        }
    }
}
