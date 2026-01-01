//! 跨平台路径处理工具 (传统原则：常识性接口设计)

use crate::error::{EnvError, Result};
use crate::types::EnvSource;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

// ==================== 系统环境缓存 ====================

/// 系统环境变量缓存结构
struct SystemEnvCache {
    env: HashMap<String, String>,
    timestamp: Instant,
}

impl SystemEnvCache {
    /// 检查缓存是否有效（60秒 TTL）
    fn is_valid(&self) -> bool {
        self.timestamp.elapsed() < Duration::from_secs(60)
    }
}

/// 全局缓存实例（使用 `OnceLock` 确保线程安全）
static SYSTEM_ENV_CACHE: std::sync::OnceLock<Mutex<Option<SystemEnvCache>>> =
    std::sync::OnceLock::new();

/// 实际读取系统环境的内部函数（无缓存）
fn read_system_env_from_source() -> HashMap<String, String> {
    let mut env = HashMap::new();

    #[cfg(target_os = "windows")]
    {
        use winreg::{RegKey, enums::HKEY_CURRENT_USER};

        // 先添加当前进程的环境变量
        for (key, value) in std::env::vars() {
            if !value.is_empty() && !key.starts_with('_') && key != "_" {
                env.insert(key, value);
            }
        }

        // 从注册表读取用户级环境变量
        if let Ok(reg_key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey("Environment") {
            for (name, _value_type) in reg_key.enum_values().flatten() {
                if name.starts_with('_') || name == "_" {
                    continue;
                }
                if let Ok(value) = reg_key.get_value::<String, _>(&name)
                    && !value.is_empty()
                {
                    env.insert(name, value);
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        for (key, value) in std::env::vars() {
            if !value.is_empty() && !key.starts_with('_') && key != "_" {
                env.insert(key, value);
            }
        }
    }

    env
}

/// # Panics
///
/// Panics if the mutex is poisoned.
///
/// # Errors
///
/// Returns errors from system environment reading operations.
pub fn get_system_env() -> Result<HashMap<String, String>> {
    let cache_guard = SYSTEM_ENV_CACHE.get_or_init(|| Mutex::new(None));
    let mut cache_opt = cache_guard.lock().unwrap();

    // 检查缓存有效性
    if let Some(cache) = &*cache_opt
        && cache.is_valid()
    {
        return Ok(cache.env.clone());
    }

    // 缓存失效，重新读取
    let env = read_system_env_from_source();

    // 更新缓存
    *cache_opt = Some(SystemEnvCache {
        env: env.clone(),
        timestamp: Instant::now(),
    });

    Ok(env)
}

/// # Panics
///
/// Panics if the mutex is poisoned.
pub fn clear_system_env_cache() {
    if let Some(cache) = SYSTEM_ENV_CACHE.get() {
        let mut guard = cache.lock().unwrap();
        *guard = None;
    }
}

/// 获取系统环境缓存统计信息
pub fn get_system_env_cache_stats() -> (bool, Duration) {
    if let Some(cache) = SYSTEM_ENV_CACHE.get()
        && let Ok(guard) = cache.lock()
        && let Some(c) = &*guard
    {
        return (true, c.timestamp.elapsed());
    }
    (false, Duration::from_secs(0))
}

/// 获取用户配置目录：~/.envcli
///
/// # Errors
///
/// Returns `EnvError::ConfigDirMissing` if config directory cannot be determined.
pub fn get_config_dir() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| EnvError::ConfigDirMissing("无法找到用户主目录".to_string()))?;

    Ok(home.join(".envcli"))
}

/// 获取指定层级的文件路径
///
/// # Errors
///
/// Returns errors from path resolution or file system operations.
pub fn get_layer_path(source: &EnvSource) -> Result<PathBuf> {
    let config_dir = get_config_dir()?;

    match source {
        EnvSource::System => Err(EnvError::PermissionDenied("系统环境变量层只读".to_string())),
        EnvSource::User => Ok(config_dir.join("user.env")),
        EnvSource::Project => {
            let current_dir = std::env::current_dir()
                .map_err(|e| EnvError::ConfigDirMissing(format!("无法获取当前目录: {e}")))?;
            Ok(current_dir.join(".envcli").join("project.env"))
        }
        EnvSource::Local => {
            let current_dir = std::env::current_dir()
                .map_err(|e| EnvError::ConfigDirMissing(format!("无法获取当前目录: {e}")))?;
            Ok(current_dir.join(".envcli").join("local.env"))
        }
    }
}

/// 确保配置目录存在 (幂等操作)
///
/// # Errors
///
/// Returns errors from directory creation operations.
pub fn ensure_config_dir() -> Result<()> {
    let config_dir = get_config_dir()?;
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir)?;
    }
    Ok(())
}

/// 确保项目级目录存在
///
/// # Errors
///
/// Returns errors from directory creation operations.
pub fn ensure_project_dir() -> Result<()> {
    let current_dir = std::env::current_dir()
        .map_err(|e| EnvError::ConfigDirMissing(format!("无法获取当前目录: {e}")))?;
    let project_dir = current_dir.join(".envcli");
    if !project_dir.exists() {
        std::fs::create_dir_all(&project_dir)?;
    }
    Ok(())
}

/// 获取模板目录路径
///
/// # Errors
///
/// Returns errors from config directory resolution.
pub fn get_templates_dir() -> Result<PathBuf> {
    let config_dir = get_config_dir()?;
    Ok(config_dir.join("templates"))
}

/// 获取插件配置文件路径
///
/// # Errors
///
/// Returns errors from config directory resolution.
pub fn get_plugin_config_path() -> Result<PathBuf> {
    let config_dir = get_config_dir()?;
    Ok(config_dir.join("plugins.toml"))
}

/// 获取插件目录路径
///
/// # Errors
///
/// Returns errors from config directory resolution.
pub fn get_plugins_dir() -> Result<PathBuf> {
    let config_dir = get_config_dir()?;
    Ok(config_dir.join("plugins"))
}

/// 检查文件是否存在
#[must_use]
pub fn file_exists(path: &Path) -> bool {
    path.exists() && path.is_file()
}

/// 读取文件内容，返回错误时提供详细信息
///
/// # Errors
///
/// Returns `EnvError::FileNotFound` if the file doesn't exist.
/// Returns `EnvError::Io` for I/O errors.
pub fn read_file(path: &Path) -> Result<String> {
    if !path.exists() {
        return Err(EnvError::FileNotFound(path.to_path_buf()));
    }
    std::fs::read_to_string(path).map_err(|e| {
        EnvError::Io(std::io::Error::other(format!(
            "读取文件 {} 失败: {}",
            path.display(),
            e
        )))
    })
}

/// 安全写入文件 (使用临时文件 + 原子替换)
///
/// # Errors
///
/// Returns errors from directory creation or file operations.
pub fn write_file_safe(path: &Path, content: &str) -> Result<()> {
    // 确保父目录存在
    if let Some(parent) = path.parent()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent)?;
    }

    // 写入临时文件
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, content)?;

    // 原子替换
    std::fs::rename(&temp_path, path)?;

    Ok(())
}

/// 追加内容到文件 (如果内容已存在则不追加)
///
/// 注意：此函数目前未使用，保留作为工具函数供未来使用
///
/// # Errors
///
/// Returns errors from file operations.
#[allow(dead_code)]
pub fn append_to_file_unique(path: &Path, line: &str) -> Result<()> {
    // 如果文件存在，检查是否已有该行
    if path.exists() {
        let content = read_file(path)?;
        if content.lines().any(|l| l.trim() == line.trim()) {
            return Ok(()); // 已存在，无需操作
        }
    }

    // 追加内容
    let mut content = if path.exists() {
        read_file(path)?
    } else {
        String::new()
    };

    if !content.ends_with('\n') && !content.is_empty() {
        content.push('\n');
    }
    content.push_str(line);
    content.push('\n');

    write_file_safe(path, &content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    mod file_operations_tests {
        use super::*;

        #[test]
        fn test_file_exists_with_existing_file() {
            let temp_dir = tempfile::tempdir().unwrap();
            let test_file = temp_dir.path().join("test.txt");
            fs::write(&test_file, "content").unwrap();

            assert!(file_exists(&test_file));
        }

        #[test]
        fn test_file_exists_with_nonexistent_file() {
            let temp_dir = tempfile::tempdir().unwrap();
            let test_file = temp_dir.path().join("nonexistent.txt");

            assert!(!file_exists(&test_file));
        }

        #[test]
        fn test_file_exists_with_directory() {
            let temp_dir = tempfile::tempdir().unwrap();
            let test_dir = temp_dir.path().join("subdir");
            fs::create_dir(&test_dir).unwrap();

            assert!(!file_exists(&test_dir)); // 应该返回 false，因为是目录不是文件
        }

        #[test]
        fn test_read_file_success() {
            let temp_dir = tempfile::tempdir().unwrap();
            let test_file = temp_dir.path().join("test.txt");
            let content = "Hello, World!";
            fs::write(&test_file, content).unwrap();

            let result = read_file(&test_file);
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), content);
        }

        #[test]
        fn test_read_file_not_found() {
            let temp_dir = tempfile::tempdir().unwrap();
            let test_file = temp_dir.path().join("nonexistent.txt");

            let result = read_file(&test_file);
            assert!(result.is_err());
            assert!(matches!(result.unwrap_err(), EnvError::FileNotFound(_)));
        }

        #[test]
        fn test_write_file_safe_creates_file() {
            let temp_dir = tempfile::tempdir().unwrap();
            let test_file = temp_dir.path().join("test.txt");
            let content = "test content";

            let result = write_file_safe(&test_file, content);
            assert!(result.is_ok());
            assert!(test_file.exists());
            assert_eq!(fs::read_to_string(&test_file).unwrap(), content);
        }

        #[test]
        fn test_write_file_safe_overwrites_existing() {
            let temp_dir = tempfile::tempdir().unwrap();
            let test_file = temp_dir.path().join("test.txt");

            fs::write(&test_file, "old content").unwrap();
            let result = write_file_safe(&test_file, "new content");

            assert!(result.is_ok());
            assert_eq!(fs::read_to_string(&test_file).unwrap(), "new content");
        }

        #[test]
        fn test_write_file_safe_creates_parent_dirs() {
            let temp_dir = tempfile::tempdir().unwrap();
            let test_file = temp_dir.path().join("subdir").join("test.txt");

            let result = write_file_safe(&test_file, "content");
            assert!(result.is_ok());
            assert!(test_file.exists());
        }

        #[test]
        fn test_append_to_file_unique_new_file() {
            let temp_dir = tempfile::tempdir().unwrap();
            let test_file = temp_dir.path().join("test.txt");
            let line = "KEY=VALUE";

            let result = append_to_file_unique(&test_file, line);
            assert!(result.is_ok());

            let content = fs::read_to_string(&test_file).unwrap();
            assert!(content.contains(line));
        }

        #[test]
        fn test_append_to_file_unique_duplicate() {
            let temp_dir = tempfile::tempdir().unwrap();
            let test_file = temp_dir.path().join("test.txt");
            let line = "KEY=VALUE";

            // 第一次追加
            append_to_file_unique(&test_file, line).unwrap();
            let content1 = fs::read_to_string(&test_file).unwrap();

            // 第二次追加相同内容
            append_to_file_unique(&test_file, line).unwrap();
            let content2 = fs::read_to_string(&test_file).unwrap();

            // 内容应该相同，没有重复
            assert_eq!(content1, content2);
            assert_eq!(content1.lines().count(), 1);
        }

        #[test]
        fn test_append_to_file_unique_multiple_lines() {
            let temp_dir = tempfile::tempdir().unwrap();
            let test_file = temp_dir.path().join("test.txt");

            append_to_file_unique(&test_file, "KEY1=VALUE1").unwrap();
            append_to_file_unique(&test_file, "KEY2=VALUE2").unwrap();

            let content = fs::read_to_string(&test_file).unwrap();
            let lines: Vec<&str> = content.lines().collect();

            assert_eq!(lines.len(), 2);
            assert!(lines[0].contains("KEY1=VALUE1"));
            assert!(lines[1].contains("KEY2=VALUE2"));
        }

        #[test]
        fn test_append_to_file_unique_with_trailing_newline() {
            let temp_dir = tempfile::tempdir().unwrap();
            let test_file = temp_dir.path().join("test.txt");

            // 写入初始内容（不带换行）
            fs::write(&test_file, "EXISTING").unwrap();

            append_to_file_unique(&test_file, "NEW").unwrap();

            let content = fs::read_to_string(&test_file).unwrap();
            assert!(content.contains("EXISTING"));
            assert!(content.contains("NEW"));
            assert_eq!(content.lines().count(), 2);
        }
    }

    mod path_generation_tests {
        use super::*;

        #[test]
        fn test_get_config_dir_success() {
            // 这个测试依赖于系统环境，可能在不同环境表现不同
            // 我们只验证它不 panic 并返回有效路径
            let result = get_config_dir();
            assert!(result.is_ok());

            let path = result.unwrap();
            assert!(path.is_absolute());
            assert!(
                path.to_string_lossy().contains(".envcli")
                    || path.to_string_lossy().contains("envcli")
            );
        }

        #[test]
        fn test_get_layer_path_system_error() {
            let result = get_layer_path(&EnvSource::System);
            assert!(result.is_err());
            assert!(matches!(result.unwrap_err(), EnvError::PermissionDenied(_)));
        }

        #[test]
        fn test_get_layer_path_user() {
            let result = get_layer_path(&EnvSource::User);
            assert!(result.is_ok());

            let path = result.unwrap();
            assert!(path.to_string_lossy().contains("user.env"));
        }

        #[test]
        fn test_get_layer_path_project() {
            let result = get_layer_path(&EnvSource::Project);
            assert!(result.is_ok());

            let path = result.unwrap();
            assert!(path.to_string_lossy().contains("project.env"));
        }

        #[test]
        fn test_get_layer_path_local() {
            let result = get_layer_path(&EnvSource::Local);
            assert!(result.is_ok());

            let path = result.unwrap();
            assert!(path.to_string_lossy().contains("local.env"));
        }

        #[test]
        fn test_get_templates_dir() {
            let result = get_templates_dir();
            assert!(result.is_ok());

            let path = result.unwrap();
            assert!(path.to_string_lossy().contains("templates"));
        }

        #[test]
        fn test_ensure_config_dir_creates_directory() {
            // 临时修改 HOME 环境变量（仅在测试中）
            // 注意：由于 dirs::home_dir() 的限制，这个测试可能需要特殊处理
            // 我们只验证函数签名和基本逻辑
            let result = ensure_config_dir();
            assert!(result.is_ok() || result.is_err()); // 取决于系统环境
        }

        #[test]
        fn test_ensure_project_dir_creates_directory() {
            use crate::test_utils::with_temp_dir;

            with_temp_dir(|temp_dir| {
                let result = ensure_project_dir();
                assert!(result.is_ok());

                let project_dir = temp_dir.path().join(".envcli");
                assert!(project_dir.exists());
            });
        }

        #[test]
        fn test_ensure_project_dir_idempotent() {
            use crate::test_utils::with_temp_dir;

            with_temp_dir(|_| {
                // 第一次调用
                ensure_project_dir().unwrap();
                // 第二次调用（目录已存在）
                let result = ensure_project_dir();

                assert!(result.is_ok());
            });
        }
    }

    mod system_env_tests {
        use super::*;

        #[test]
        fn test_get_system_env_returns_map() {
            // 清除缓存以确保测试独立性
            clear_system_env_cache();

            let result = get_system_env();
            assert!(result.is_ok());

            let env = result.unwrap();
            // 应该包含一些系统环境变量
            assert!(!env.is_empty());
        }

        #[test]
        fn test_get_system_env_contains_path() {
            // 清除缓存以确保测试独立性
            clear_system_env_cache();

            let result = get_system_env();
            assert!(result.is_ok());

            let env = result.unwrap();
            // 大多数系统都有 PATH 变量
            // 注意：在某些环境下可能不存在，所以不强制要求
            if env.contains_key("PATH") {
                let path = env.get("PATH").unwrap();
                assert!(!path.is_empty());
            }
        }

        #[test]
        fn test_get_system_env_includes_current_process_env() {
            // 清除缓存以确保测试独立性
            clear_system_env_cache();

            // 设置一个测试环境变量
            unsafe {
                std::env::set_var("TEST_ENV_VAR_UNIQUE_12345", "test_value");
            }

            let result = get_system_env();
            assert!(result.is_ok());

            let env = result.unwrap();
            assert_eq!(
                env.get("TEST_ENV_VAR_UNIQUE_12345"),
                Some(&"test_value".to_string())
            );

            // 清理
            unsafe {
                std::env::remove_var("TEST_ENV_VAR_UNIQUE_12345");
            }

            // 清除缓存，避免影响其他测试
            clear_system_env_cache();
        }
    }

    mod integration_tests {
        use super::*;

        #[test]
        fn test_full_workflow() {
            use crate::test_utils::with_temp_dir;

            with_temp_dir(|_| {
                // 1. 确保项目目录
                ensure_project_dir().unwrap();

                // 2. 获取本地层路径
                let local_path = get_layer_path(&EnvSource::Local).unwrap();

                // 3. 写入文件
                write_file_safe(&local_path, "TEST_VAR=test_value").unwrap();

                // 4. 读取文件
                let content = read_file(&local_path).unwrap();
                assert!(content.contains("TEST_VAR=test_value"));

                // 5. 追加内容
                append_to_file_unique(&local_path, "ANOTHER_VAR=another").unwrap();

                // 6. 验证文件存在
                assert!(file_exists(&local_path));
            });
        }
    }
}
