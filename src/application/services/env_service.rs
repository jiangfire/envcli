//! 环境变量应用服务

use crate::domain::error::Result;
use crate::domain::models::{EnvSource, EnvVar, OutputFormat};
use crate::domain::repositories::EnvRepository;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// 环境变量服务
pub struct EnvService {
    repository: Arc<dyn EnvRepository>,
}

impl EnvService {
    pub fn new(repository: Arc<dyn EnvRepository>) -> Self {
        Self { repository }
    }

    /// 获取变量
    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        self.repository.get(key).await
    }

    /// 设置变量
    pub async fn set(&self, key: &str, value: &str, source: EnvSource) -> Result<()> {
        self.repository.set(key, value, source).await
    }

    /// 删除变量
    pub async fn unset(&self, key: &str, source: &EnvSource) -> Result<bool> {
        self.repository.delete(key, source).await
    }

    /// 列出变量
    pub async fn list(&self, source_filter: Option<EnvSource>) -> Result<Vec<EnvVar>> {
        self.repository.list(source_filter).await
    }

    /// 导出变量
    pub async fn export(
        &self,
        source_filter: Option<EnvSource>,
        format: OutputFormat,
    ) -> Result<String> {
        match format {
            OutputFormat::Env => self.repository.export(source_filter).await,
            OutputFormat::Json => {
                let vars = self.repository.list(source_filter).await?;
                Ok(serde_json::to_string_pretty(&vars)?)
            }
        }
    }

    /// 导入 .env 文件
    pub async fn import(&self, file_path: &Path, target: EnvSource) -> Result<usize> {
        self.repository.import(file_path, target).await
    }

    /// 清除缓存
    pub async fn clear_cache(&self) {
        self.repository.clear_cache().await;
    }

    /// 获取变量来源信息
    pub async fn get_variable_info(&self, key: &str) -> Result<Vec<(EnvSource, String)>> {
        let mut results = Vec::new();

        for source in [
            EnvSource::System,
            EnvSource::User,
            EnvSource::Project,
            EnvSource::Local,
        ] {
            if let Some(value) = self.repository.get_from_source(key, &source).await? {
                results.push((source, value));
            }
        }

        Ok(results)
    }

    /// 检查变量冲突（多层级定义）
    pub async fn check_conflicts(&self) -> Result<Vec<(String, Vec<EnvSource>)>> {
        let mut conflicts = Vec::new();
        let mut key_sources: HashMap<String, Vec<EnvSource>> = HashMap::new();

        for source in [
            EnvSource::System,
            EnvSource::User,
            EnvSource::Project,
            EnvSource::Local,
        ] {
            let vars = self.repository.list(Some(source)).await?;
            for var in vars {
                key_sources.entry(var.key).or_default().push(source);
            }
        }

        for (key, sources) in key_sources {
            if sources.len() > 1 {
                conflicts.push((key, sources));
            }
        }

        Ok(conflicts)
    }
}
