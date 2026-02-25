//! 文件存储实现

use crate::domain::error::{DomainError, Result};
use crate::domain::models::{EnvSource, EnvVar};
use crate::domain::repositories::{EnvRepository, StorageConfig};
use crate::infrastructure::cache::FileCache;
use crate::infrastructure::paths;
use async_trait::async_trait;
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// 文件环境变量存储
pub struct FileEnvRepository {
    #[allow(dead_code)]
    config: StorageConfig,
    cache: Arc<FileCache>,
}

impl FileEnvRepository {
    pub fn new(config: StorageConfig) -> Self {
        let cache = if config.cache_enabled {
            Arc::new(FileCache::with_ttl(config.cache_ttl_seconds))
        } else {
            Arc::new(FileCache::new())
        };

        Self { config, cache }
    }

    /// 解析 .env 文件内容
    fn parse_dotenv(content: &str, source: &EnvSource) -> Vec<EnvVar> {
        let mut vars = Vec::new();
        let re = Regex::new(r"^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*?)\s*$").unwrap();

        for line in content.lines() {
            let trimmed = line.trim();

            // 跳过空行和注释
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            if let Some(caps) = re.captures(trimmed) {
                let key = caps[1].to_string();
                let value = caps[2].to_string();
                vars.push(EnvVar::new(key, value, *source));
            }
        }

        vars
    }

    /// 序列化为 .env 格式
    fn serialize_dotenv(vars: &[EnvVar]) -> String {
        let mut lines = vec![
            "# EnvCLI 配置文件".to_string(),
            "# 格式: KEY=VALUE".to_string(),
            String::new(),
        ];

        for var in vars {
            lines.push(format!("{}={}", var.key, var.value));
        }

        lines.join("\n")
    }

    /// 获取文件路径
    fn get_path(&self, source: &EnvSource) -> Result<PathBuf> {
        paths::get_layer_path(source)
    }

    /// 确保目录存在
    async fn ensure_dir(&self, source: &EnvSource) -> Result<()> {
        match source {
            EnvSource::User => {
                paths::ensure_config_dir()?;
            }
            EnvSource::Project | EnvSource::Local => {
                paths::ensure_project_dir()?;
            }
            _ => {}
        }
        Ok(())
    }

    /// 读取变量列表（带缓存）
    async fn read_vars(&self, source: &EnvSource) -> Result<Vec<EnvVar>> {
        if *source == EnvSource::System {
            let env = paths::get_system_env()?;
            return Ok(env.into_iter().map(|(k, v)| EnvVar::system(k, v)).collect());
        }

        let path = self.get_path(source)?;

        if !path.exists() {
            return Ok(Vec::new());
        }

        // 尝试从缓存获取
        if let Some(cached) = self.cache.get(&path)? {
            return Ok(cached);
        }

        // 读取文件
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| DomainError::Io(e.to_string()))?;
        let vars = Self::parse_dotenv(&content, source);

        // 更新缓存
        self.cache.set(&path, vars.clone())?;

        Ok(vars)
    }

    /// 写入变量列表
    async fn write_vars(&self, source: &EnvSource, vars: &[EnvVar]) -> Result<()> {
        self.ensure_dir(source).await?;
        let path = self.get_path(source)?;

        if vars.is_empty() {
            // 如果没有变量，删除文件
            if path.exists() {
                tokio::fs::remove_file(&path)
                    .await
                    .map_err(|e| DomainError::Io(e.to_string()))?;
            }
        } else {
            let content = Self::serialize_dotenv(vars);
            tokio::fs::write(&path, content)
                .await
                .map_err(|e| DomainError::Io(e.to_string()))?;
        }

        // 清除缓存
        self.cache.invalidate(&path);

        Ok(())
    }
}

#[async_trait]
impl EnvRepository for FileEnvRepository {
    async fn get(&self, key: &str) -> Result<Option<String>> {
        // 按优先级查找
        for source in [
            EnvSource::Local,
            EnvSource::Project,
            EnvSource::User,
            EnvSource::System,
        ] {
            if let Some(value) = self.get_from_source(key, &source).await? {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    async fn get_from_source(&self, key: &str, source: &EnvSource) -> Result<Option<String>> {
        let vars = self.read_vars(source).await?;
        Ok(vars.into_iter().find(|v| v.key == key).map(|v| v.value))
    }

    async fn set(&self, key: &str, value: &str, source: EnvSource) -> Result<()> {
        if !source.is_writable() {
            return Err(DomainError::PermissionDenied(format!(
                "{} 层不可写",
                source
            )));
        }

        let mut vars = self.read_vars(&source).await?;

        // 更新或添加
        if let Some(existing) = vars.iter_mut().find(|v| v.key == key) {
            existing.value = value.to_string();
            existing.timestamp = chrono::Utc::now();
        } else {
            vars.push(EnvVar::new(key.to_string(), value.to_string(), source));
        }

        self.write_vars(&source, &vars).await
    }

    async fn delete(&self, key: &str, source: &EnvSource) -> Result<bool> {
        if !source.is_writable() {
            return Err(DomainError::PermissionDenied(format!(
                "{} 层不可写",
                source
            )));
        }

        let mut vars = self.read_vars(source).await?;
        let original_len = vars.len();
        vars.retain(|v| v.key != key);

        if vars.len() == original_len {
            return Ok(false);
        }

        self.write_vars(source, &vars).await?;
        Ok(true)
    }

    async fn list(&self, source_filter: Option<EnvSource>) -> Result<Vec<EnvVar>> {
        match source_filter {
            Some(source) => self.read_vars(&source).await,
            None => {
                // 合并所有层级
                let mut map = HashMap::new();

                for source in [
                    EnvSource::System,
                    EnvSource::User,
                    EnvSource::Project,
                    EnvSource::Local,
                ] {
                    let vars = self.read_vars(&source).await?;
                    for var in vars {
                        map.insert(var.key.clone(), var);
                    }
                }

                Ok(map.into_values().collect())
            }
        }
    }

    async fn import(&self, file_path: &Path, target_source: EnvSource) -> Result<usize> {
        if !target_source.is_writable() {
            return Err(DomainError::PermissionDenied("目标层级不可写".to_string()));
        }

        if !file_path.exists() {
            return Err(DomainError::FileNotFound(file_path.to_path_buf()));
        }

        let content = tokio::fs::read_to_string(file_path)
            .await
            .map_err(|e| DomainError::Io(e.to_string()))?;

        let imported_vars = Self::parse_dotenv(&content, &EnvSource::System);
        let mut existing_vars = self.read_vars(&target_source).await?;

        let mut count = 0;
        for var in imported_vars {
            if !existing_vars.iter().any(|v| v.key == var.key) {
                existing_vars.push(EnvVar::new(var.key, var.value, target_source));
                count += 1;
            }
        }

        self.write_vars(&target_source, &existing_vars).await?;
        Ok(count)
    }

    async fn export(&self, source_filter: Option<EnvSource>) -> Result<String> {
        let vars = self.list(source_filter).await?;
        Ok(Self::serialize_dotenv(&vars))
    }

    async fn clear_cache(&self) {
        self.cache.clear();
    }
}
