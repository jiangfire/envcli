//! 应用程序容器
//!
//! 负责依赖注入和生命周期管理

use crate::application::services::EnvService;
use crate::domain::repositories::{EnvRepository, RepositoryFactory, StorageConfig};
use crate::infrastructure::storage::FileEnvRepository;
use anyhow::Result;
use std::sync::Arc;

/// 应用程序配置
#[derive(Debug, Clone, Default)]
pub struct AppConfig {
    pub verbose: bool,
    pub storage: StorageConfig,
}

/// 应用程序容器
pub struct Application {
    /// 环境变量服务
    pub env_service: Arc<EnvService>,
}

impl Application {
    /// 创建应用程序实例
    pub async fn new(config: AppConfig) -> Result<Self> {
        // 创建 Repository
        let env_repo: Arc<dyn EnvRepository> =
            Arc::new(FileEnvRepository::new(config.storage.clone()));

        // 创建服务
        let env_service = Arc::new(EnvService::new(env_repo));
        Ok(Self { env_service })
    }
}

/// 简单 Repository 工厂
pub struct SimpleRepositoryFactory;

impl RepositoryFactory for SimpleRepositoryFactory {
    fn create_env_repository(&self) -> Box<dyn EnvRepository> {
        Box::new(FileEnvRepository::new(StorageConfig::default()))
    }
}
