//! 环境变量实体模型

use serde::{Deserialize, Serialize};
use std::fmt;

/// 环境变量来源层级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum EnvSource {
    /// 系统环境变量 (只读)
    System,
    /// 用户级配置 ~/.envcli/user.env
    User,
    /// 项目级配置 ./.envcli/project.env
    Project,
    /// 本地级配置 ./.envcli/local.env (gitignored)
    #[default]
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
    /// 从字符串解析
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "system" => Some(EnvSource::System),
            "user" => Some(EnvSource::User),
            "project" => Some(EnvSource::Project),
            "local" => Some(EnvSource::Local),
            _ => None,
        }
    }

    /// 从字符串转换 (已弃用，请使用 `parse`)
    #[deprecated(since = "0.2.0", note = "请使用 `parse` 方法代替")]
    #[allow(clippy::should_implement_trait)]
    #[must_use]
    pub fn from_str(s: &str) -> Option<Self> {
        Self::parse(s)
    }

    /// 是否可写
    #[must_use]
    pub fn is_writable(&self) -> bool {
        !matches!(self, EnvSource::System)
    }

    /// 优先级数值（越高越优先）
    #[must_use]
    pub fn priority(&self) -> u8 {
        match self {
            EnvSource::System => 0,
            EnvSource::User => 1,
            EnvSource::Project => 2,
            EnvSource::Local => 3,
        }
    }
}

/// 环境变量条目
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvVar {
    pub key: String,
    pub value: String,
    pub source: EnvSource,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl EnvVar {
    /// 创建新的环境变量条目
    #[must_use]
    pub fn new(key: String, value: String, source: EnvSource) -> Self {
        Self {
            key,
            value,
            source,
            timestamp: chrono::Utc::now(),
        }
    }

    /// 创建系统环境变量
    #[must_use]
    pub fn system(key: String, value: String) -> Self {
        Self::new(key, value, EnvSource::System)
    }
}

/// 输出格式类型
#[derive(Debug, Clone, PartialEq, Default)]
pub enum OutputFormat {
    #[default]
    Env,
    Json,
}

impl From<&str> for OutputFormat {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" | "j" => OutputFormat::Json,
            _ => OutputFormat::Env,
        }
    }
}
