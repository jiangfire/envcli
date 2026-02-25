//! 存储接口定义（输出端口）

use crate::domain::error::Result;
use crate::domain::models::{EnvSource, EnvVar};
use async_trait::async_trait;
use std::path::Path;

/// 环境变量存储接口
#[async_trait]
pub trait EnvRepository: Send + Sync {
    /// 获取单个变量（按优先级合并）
    async fn get(&self, key: &str) -> Result<Option<String>>;

    /// 从指定源获取变量
    async fn get_from_source(&self, key: &str, source: &EnvSource) -> Result<Option<String>>;

    /// 设置变量
    async fn set(&self, key: &str, value: &str, source: EnvSource) -> Result<()>;

    /// 删除变量
    async fn delete(&self, key: &str, source: &EnvSource) -> Result<bool>;

    /// 列出变量
    async fn list(&self, source_filter: Option<EnvSource>) -> Result<Vec<EnvVar>>;

    /// 列出所有变量（合并后）
    async fn list_merged(&self) -> Result<Vec<EnvVar>> {
        self.list(None).await
    }

    /// 导入 .env 文件
    async fn import(&self, file_path: &Path, target_source: EnvSource) -> Result<usize>;

    /// 导出变量为 .env 格式
    async fn export(&self, source_filter: Option<EnvSource>) -> Result<String>;

    /// 清除缓存
    async fn clear_cache(&self);
}

/// Repository 工厂
pub trait RepositoryFactory: Send + Sync {
    fn create_env_repository(&self) -> Box<dyn EnvRepository>;
}

/// 存储配置
#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub user_dir: std::path::PathBuf,
    pub project_dir: std::path::PathBuf,
    pub cache_enabled: bool,
    pub cache_ttl_seconds: u64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            user_dir: dirs::home_dir()
                .map(|h| h.join(".envcli"))
                .unwrap_or_default(),
            project_dir: std::path::PathBuf::from(".envcli"),
            cache_enabled: true,
            cache_ttl_seconds: 60,
        }
    }
}
