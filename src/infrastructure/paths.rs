//! 路径管理工具

use crate::domain::error::{DomainError, Result};
use crate::domain::models::EnvSource;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

/// 获取配置目录
pub fn get_config_dir() -> Result<PathBuf> {
    dirs::home_dir()
        .map(|h| h.join(".envcli"))
        .ok_or_else(|| DomainError::Config("无法确定主目录".to_string()))
}

/// 获取特定层级的文件路径
pub fn get_layer_path(source: &EnvSource) -> Result<PathBuf> {
    match source {
        EnvSource::System => Err(DomainError::InvalidSource(
            "System 层没有文件路径".to_string(),
        )),
        EnvSource::User => {
            let config_dir = get_config_dir()?;
            Ok(config_dir.join("user.env"))
        }
        EnvSource::Project => {
            let cwd = std::env::current_dir().map_err(|e| DomainError::Io(e.to_string()))?;
            Ok(cwd.join(".envcli").join("project.env"))
        }
        EnvSource::Local => {
            let cwd = std::env::current_dir().map_err(|e| DomainError::Io(e.to_string()))?;
            Ok(cwd.join(".envcli").join("local.env"))
        }
    }
}

/// 确保配置目录存在
pub fn ensure_config_dir() -> Result<PathBuf> {
    let dir = get_config_dir()?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| DomainError::Io(e.to_string()))?;
    }
    Ok(dir)
}

/// 确保项目目录存在
pub fn ensure_project_dir() -> Result<PathBuf> {
    let cwd = std::env::current_dir().map_err(|e| DomainError::Io(e.to_string()))?;
    let project_dir = cwd.join(".envcli");
    if !project_dir.exists() {
        std::fs::create_dir_all(&project_dir).map_err(|e| DomainError::Io(e.to_string()))?;
    }
    Ok(project_dir)
}

// ============ 系统环境变量缓存 ============

static SYSTEM_ENV_CACHE: RwLock<Option<(HashMap<String, String>, std::time::Instant)>> =
    RwLock::new(None);

const CACHE_TTL_SECONDS: u64 = 60;

/// 获取系统环境变量（带缓存）
pub fn get_system_env() -> Result<HashMap<String, String>> {
    // 尝试从缓存读取
    {
        let cache = SYSTEM_ENV_CACHE
            .read()
            .map_err(|_| DomainError::Storage("缓存锁获取失败".to_string()))?;

        if let Some((vars, timestamp)) = cache.as_ref() {
            let elapsed = timestamp.elapsed().as_secs();
            if elapsed < CACHE_TTL_SECONDS {
                return Ok(vars.clone());
            }
        }
    }

    // 缓存未命中或已过期，重新加载
    let vars: HashMap<String, String> = std::env::vars().collect();

    {
        let mut cache = SYSTEM_ENV_CACHE
            .write()
            .map_err(|_| DomainError::Storage("缓存锁获取失败".to_string()))?;
        *cache = Some((vars.clone(), std::time::Instant::now()));
    }

    Ok(vars)
}

/// 清除系统环境变量缓存
pub fn clear_system_env_cache() {
    if let Ok(mut cache) = SYSTEM_ENV_CACHE.write() {
        *cache = None;
    }
}

/// 获取缓存统计信息
pub fn get_system_env_cache_stats() -> (bool, std::time::Duration) {
    if let Ok(cache) = SYSTEM_ENV_CACHE.read()
        && let Some((_, timestamp)) = cache.as_ref()
    {
        let age = timestamp.elapsed();
        let valid = age.as_secs() < CACHE_TTL_SECONDS;
        return (valid, age);
    }
    (false, std::time::Duration::MAX)
}
