//! 测试工具模块
//!
//! 提供统一的测试环境管理，避免环境变量污染和临时目录管理问题

use std::collections::HashMap;
use std::env;
use tempfile::TempDir;

/// 环境变量守卫 - 自动清理
pub struct EnvGuard {
    original_vars: HashMap<String, String>,
}

impl Default for EnvGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl EnvGuard {
    /// 创建一个新的环境守卫，记录当前环境变量
    pub fn new() -> Self {
        let original_vars: HashMap<String, String> = env::vars().collect();
        Self { original_vars }
    }

    /// 设置测试环境变量（自动包装为 unsafe）
    pub fn set_var(&self, key: &str, value: &str) {
        unsafe {
            env::set_var(key, value);
        }
    }

    /// 移除环境变量（自动包装为 unsafe）
    pub fn remove_var(&self, key: &str) {
        unsafe {
            env::remove_var(key);
        }
    }

    /// 检查环境变量是否存在（工具函数，供测试使用）
    #[allow(dead_code)]
    pub fn contains_var(&self, key: &str) -> bool {
        env::var(key).is_ok()
    }

    /// 获取环境变量值（工具函数，供测试使用）
    #[allow(dead_code)]
    pub fn get_var(&self, key: &str) -> Option<String> {
        env::var(key).ok()
    }
}

impl Drop for EnvGuard {
    /// 释放时恢复原始环境变量
    fn drop(&mut self) {
        // 首先移除所有不在原始环境中的变量
        let current_vars: Vec<String> = env::vars().map(|(k, _)| k).collect();
        for key in current_vars {
            if !self.original_vars.contains_key(&key) {
                self.remove_var(&key);
            }
        }

        // 然后恢复所有原始变量的值
        for (key, value) in &self.original_vars {
            match env::var(key) {
                Ok(current) => {
                    if current != *value {
                        self.set_var(key, value);
                    }
                }
                Err(_) => {
                    self.set_var(key, value);
                }
            }
        }
    }
}

/// 临时目录测试包装器
pub struct TempDirGuard {
    temp_dir: TempDir,
    original_dir: std::path::PathBuf,
}

impl Default for TempDirGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl TempDirGuard {
    /// 创建临时目录并切换到该目录
    pub fn new() -> Self {
        let temp_dir = tempfile::tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        Self {
            temp_dir,
            original_dir,
        }
    }

    /// 获取临时目录路径
    pub fn path(&self) -> &std::path::Path {
        self.temp_dir.path()
    }
}

impl Drop for TempDirGuard {
    /// 释放时恢复原始目录
    fn drop(&mut self) {
        std::env::set_current_dir(&self.original_dir).unwrap();
    }
}

/// 安全的测试环境包装器 - 自动管理环境和目录
pub fn with_isolated_test_env<F, R>(f: F) -> R
where
    F: FnOnce(&TempDirGuard, &EnvGuard) -> R,
{
    let _env_guard = EnvGuard::new();
    let temp_guard = TempDirGuard::new();
    f(&temp_guard, &_env_guard)
}

/// 简化的临时目录测试包装器
pub fn with_temp_dir<F, R>(f: F) -> R
where
    F: Fn(&TempDir) -> R,
{
    let temp_dir = tempfile::tempdir().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&temp_dir).unwrap();

    let result = f(&temp_dir);

    std::env::set_current_dir(original_dir).unwrap();
    result
}

/// 简化的环境变量测试包装器
pub fn with_env_vars<F, R>(vars: &[(&str, &str)], f: F) -> R
where
    F: FnOnce() -> R,
{
    let guard = EnvGuard::new();

    // 设置测试变量
    for (key, value) in vars {
        guard.set_var(key, value);
    }

    // 清理工作由 EnvGuard::drop 自动完成
    f()
}

/// 断言环境变量存在且值正确
pub fn assert_env_var(key: &str, expected_value: &str) {
    match env::var(key) {
        Ok(actual) => assert_eq!(actual, expected_value, "环境变量 {} 值不匹配", key),
        Err(_) => panic!("环境变量 {} 不存在", key),
    }
}

/// 断言环境变量不存在
pub fn assert_env_var_not_exists(key: &str) {
    assert!(env::var(key).is_err(), "环境变量 {} 应该不存在", key);
}

/// 清理指定的环境变量（在测试结束时调用）
///
/// 注意：EnvGuard 的 Drop 实现会自动清理，此函数保留供特殊场景使用
#[allow(dead_code)]
pub fn cleanup_env_var(key: &str) {
    unsafe {
        env::remove_var(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_guard_basic() {
        let guard = EnvGuard::new();
        guard.set_var("TEST_GUARD_VAR", "test_value");
        assert_env_var("TEST_GUARD_VAR", "test_value");
    }

    #[test]
    fn test_env_guard_cleanup() {
        {
            let guard = EnvGuard::new();
            guard.set_var("TEST_CLEANUP_VAR", "cleanup_test");
            assert_env_var("TEST_CLEANUP_VAR", "cleanup_test");
        }
        // guard 被释放后，变量应该被清理
        assert!(env::var("TEST_CLEANUP_VAR").is_err());
    }

    #[test]
    fn test_temp_dir_guard() {
        let guard = TempDirGuard::new();
        let test_file = guard.path().join("test.txt");
        std::fs::write(&test_file, "test").unwrap();
        assert!(test_file.exists());
    }

    #[test]
    fn test_with_isolated_test_env() {
        with_isolated_test_env(|temp_guard, env_guard| {
            env_guard.set_var("TEST_ISOLATED", "isolated_value");
            let test_file = temp_guard.path().join("test.txt");
            std::fs::write(&test_file, "content").unwrap();

            assert_env_var("TEST_ISOLATED", "isolated_value");
            assert!(test_file.exists());
        });
    }

    #[test]
    fn test_with_temp_dir() {
        with_temp_dir(|temp_dir| {
            let test_file = temp_dir.path().join("test.txt");
            std::fs::write(&test_file, "content").unwrap();
            assert!(test_file.exists());
        });
    }

    #[test]
    fn test_with_env_vars() {
        with_env_vars(&[("TEST_VAR1", "value1"), ("TEST_VAR2", "value2")], || {
            assert_env_var("TEST_VAR1", "value1");
            assert_env_var("TEST_VAR2", "value2");
        });
        // 清理后应该不存在
        assert_env_var_not_exists("TEST_VAR1");
        assert_env_var_not_exists("TEST_VAR2");
    }
}
