//! 核心数据结构定义 (表达原则：用数据结构表达逻辑)

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
    pub fn is_writable(&self) -> bool {
        match self {
            EnvSource::System => false,
            _ => true,
        }
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
#[derive(Debug, Clone)]
pub struct Config {
    pub verbose: bool,      // 是否详细输出
}

impl Default for Config {
    fn default() -> Self {
        Self {
            verbose: false,
        }
    }
}

/// 输出格式类型
#[derive(Debug, Clone, PartialEq)]
pub enum OutputFormat {
    ENV,
    JSON,
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::ENV
    }
}

impl From<&str> for OutputFormat {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" | "j" => OutputFormat::JSON,
            _ => OutputFormat::ENV,
        }
    }
}