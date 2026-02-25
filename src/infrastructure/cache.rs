//! 缓存实现

use crate::domain::error::Result;
use crate::domain::models::EnvVar;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::time::{Duration, SystemTime};

/// 文件缓存条目
#[derive(Clone)]
pub struct FileCacheEntry {
    pub vars: Vec<EnvVar>,
    pub last_modified: SystemTime,
}

/// 文件内容缓存
pub struct FileCache {
    inner: RwLock<HashMap<PathBuf, FileCacheEntry>>,
    #[allow(dead_code)]
    ttl: Option<Duration>,
}

impl FileCache {
    /// 创建新的缓存
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
            ttl: None,
        }
    }

    /// 创建带 TTL 的缓存
    pub fn with_ttl(seconds: u64) -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
            ttl: Some(Duration::from_secs(seconds)),
        }
    }

    /// 获取缓存
    pub fn get(&self, path: &Path) -> Result<Option<Vec<EnvVar>>> {
        let cache = self
            .inner
            .read()
            .map_err(|_| crate::domain::error::DomainError::Storage("缓存锁错误".to_string()))?;

        if let Some(entry) = cache.get(path) {
            // 检查文件是否被修改
            if let Ok(metadata) = std::fs::metadata(path)
                && let Ok(modified) = metadata.modified()
                && entry.last_modified == modified
            {
                return Ok(Some(entry.vars.clone()));
            }
        }

        Ok(None)
    }

    /// 设置缓存
    pub fn set(&self, path: &Path, vars: Vec<EnvVar>) -> Result<()> {
        let modified = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| SystemTime::now());

        let mut cache = self
            .inner
            .write()
            .map_err(|_| crate::domain::error::DomainError::Storage("缓存锁错误".to_string()))?;

        cache.insert(
            path.to_path_buf(),
            FileCacheEntry {
                vars,
                last_modified: modified,
            },
        );

        Ok(())
    }

    /// 清除指定路径的缓存
    pub fn invalidate(&self, path: &Path) {
        if let Ok(mut cache) = self.inner.write() {
            cache.remove(path);
        }
    }

    /// 清除所有缓存
    pub fn clear(&self) {
        if let Ok(mut cache) = self.inner.write() {
            cache.clear();
        }
    }

    /// 获取缓存大小
    pub fn len(&self) -> usize {
        self.inner.read().map(|c| c.len()).unwrap_or(0)
    }

    /// 缓存是否为空
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for FileCache {
    fn default() -> Self {
        Self::new()
    }
}

/// 无缓存（空实现）
pub struct NoCache;

impl NoCache {
    pub fn new() -> Self {
        Self
    }

    pub fn get(&self, _path: &Path) -> Result<Option<Vec<EnvVar>>> {
        Ok(None)
    }

    pub fn set(&self, _path: &Path, _vars: Vec<EnvVar>) -> Result<()> {
        Ok(())
    }

    pub fn invalidate(&self, _path: &Path) {}

    pub fn clear(&self) {}
}

impl Default for NoCache {
    fn default() -> Self {
        Self::new()
    }
}
