//! 核心存储引擎 (模块原则：清晰分离的存储逻辑)

use crate::config::format::dotenv::DotenvParser;
use crate::config::format::encrypted_dotenv::EncryptedDotenvParser;
use crate::error::{EnvError, Result};
use crate::types::{Config, EncryptedEnvVar, EncryptionType, EnvSource, EnvVar};
use crate::utils::encryption::SopsEncryptor;
use crate::utils::paths::{self, file_exists, get_system_env, read_file, write_file_safe};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;
use std::time::SystemTime;

// ==================== 文件内容缓存 ====================

/// 文件缓存条目
#[derive(Clone)]
struct FileCacheEntry {
    vars: Vec<EnvVar>,
    last_modified: SystemTime,
}

/// 全局文件缓存（使用 RwLock 优化读多写少场景）
static FILE_CACHE: std::sync::OnceLock<RwLock<HashMap<PathBuf, FileCacheEntry>>> =
    std::sync::OnceLock::new();

/// 获取文件缓存引用
fn get_file_cache() -> &'static RwLock<HashMap<PathBuf, FileCacheEntry>> {
    FILE_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// 核心存储引擎 (遵循分离原则：接口与实现分离)
#[derive(Clone)]
pub struct Store {
    config: Config,
}

impl Store {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// 获取单个变量（按优先级合并）
    /// 优先级：Local > Project > User > System
    pub fn get(&self, key: &str) -> Result<Option<String>> {
        // 从高优先级向低优先级查找
        let sources = [
            EnvSource::Local,
            EnvSource::Project,
            EnvSource::User,
            EnvSource::System,
        ];

        for source in sources {
            if let Some(value) = self.get_from_source(key, &source)? {
                return Ok(Some(value));
            }
        }

        Ok(None)
    }

    /// 从指定源获取变量（带缓存）
    fn get_from_source(&self, key: &str, source: &EnvSource) -> Result<Option<String>> {
        // 系统层特殊处理
        if *source == EnvSource::System {
            let system_env = get_system_env()?;
            return Ok(system_env.get(key).cloned());
        }

        // 获取文件路径
        let path = paths::get_layer_path(source)?;

        // 文件不存在
        if !file_exists(&path) {
            return Ok(None);
        }

        // 尝试从缓存获取
        if let Some(cached_vars) = self.get_cached_vars(&path)? {
            return Ok(cached_vars
                .iter()
                .find(|v| v.key == key)
                .map(|v| v.value.clone()));
        }

        // 缓存未命中，读取并解析
        let content = read_file(&path)?;
        let vars = DotenvParser::parse(&content, source)?;

        // 更新缓存
        self.update_cache(&path, vars.clone())?;

        // 查找目标变量
        Ok(vars.iter().find(|v| v.key == key).map(|v| v.value.clone()))
    }

    /// 从缓存获取变量列表
    fn get_cached_vars(&self, path: &PathBuf) -> Result<Option<Vec<EnvVar>>> {
        if !file_exists(path) {
            return Ok(None);
        }

        let cache = get_file_cache().read().unwrap();

        if let Some(entry) = cache.get(path) {
            // 检查文件是否被修改
            let current_modified = std::fs::metadata(path)?.modified()?;
            if entry.last_modified == current_modified {
                return Ok(Some(entry.vars.clone()));
            }
        }

        Ok(None)
    }

    /// 更新缓存
    fn update_cache(&self, path: &PathBuf, vars: Vec<EnvVar>) -> Result<()> {
        let current_modified = std::fs::metadata(path)?.modified()?;

        let mut cache = get_file_cache().write().unwrap();

        cache.insert(
            path.clone(),
            FileCacheEntry {
                vars,
                last_modified: current_modified,
            },
        );

        Ok(())
    }

    /// 清除指定路径的缓存
    pub fn invalidate_cache(&self, path: &PathBuf) {
        if let Ok(mut cache) = get_file_cache().write() {
            cache.remove(path);
        }
    }

    /// 清除所有文件缓存
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = get_file_cache().write() {
            cache.clear();
        }
    }

    /// 设置变量（写入最高优先级可写层：Local）
    pub fn set(&self, key: String, value: String) -> Result<()> {
        // 确保本地目录存在
        paths::ensure_project_dir()?;

        // 获取本地层路径
        let path = paths::get_layer_path(&EnvSource::Local)?;

        // 读取现有变量（如果文件存在）
        let mut vars = if file_exists(&path) {
            let content = read_file(&path)?;
            DotenvParser::parse(&content, &EnvSource::Local)?
        } else {
            Vec::new()
        };

        // 更新或添加变量
        if let Some(existing) = vars.iter_mut().find(|v| v.key == key) {
            existing.value = value.clone();
        } else {
            vars.push(EnvVar::new(key.clone(), value.clone(), EnvSource::Local));
        }

        // 序列化并写回
        let new_content = DotenvParser::serialize(&vars);
        write_file_safe(&path, &new_content)?;

        // 清除文件缓存，确保下一次读取获取最新内容
        self.invalidate_cache(&path);

        if self.config.verbose {
            println!("✓ 设置变量 {} = {}", key, value);
        }

        Ok(())
    }

    /// 删除变量（从 Local 层中移除）
    pub fn unset(&self, key: &str) -> Result<bool> {
        let path = paths::get_layer_path(&EnvSource::Local)?;

        if !file_exists(&path) {
            return Ok(false);
        }

        let content = read_file(&path)?;
        let vars = DotenvParser::parse(&content, &EnvSource::Local)?;

        // 过滤掉目标变量
        let remaining: Vec<_> = vars.iter().filter(|v| v.key != key).cloned().collect();

        if remaining.len() == vars.len() {
            // 变量不存在
            return Ok(false);
        }

        // 重写文件
        if remaining.is_empty() {
            // 如果没有变量了，删除文件
            std::fs::remove_file(&path)?;
        } else {
            let new_content = DotenvParser::serialize(&remaining);
            write_file_safe(&path, &new_content)?;
        }

        // 清除文件缓存，确保下一次读取获取最新内容
        self.invalidate_cache(&path);

        if self.config.verbose {
            println!("✓ 删除变量 {}", key);
        }

        Ok(true)
    }

    /// 列出变量
    pub fn list(&self, source_filter: Option<EnvSource>) -> Result<Vec<EnvVar>> {
        match source_filter {
            Some(source) => self.list_from_source(&source),
            None => self.list_merged(),
        }
    }

    /// 列出指定源的变量（带缓存）
    fn list_from_source(&self, source: &EnvSource) -> Result<Vec<EnvVar>> {
        match source {
            EnvSource::System => {
                let env = get_system_env()?;

                Ok(env
                    .into_iter()
                    .map(|(k, v)| EnvVar::new(k, v, EnvSource::System))
                    .collect())
            }
            _ => {
                let path = paths::get_layer_path(source)?;

                if !file_exists(&path) {
                    return Ok(vec![]);
                }

                // 使用缓存
                if let Some(cached) = self.get_cached_vars(&path)? {
                    return Ok(cached);
                }

                let content = read_file(&path)?;
                let vars = DotenvParser::parse(&content, source)?;
                self.update_cache(&path, vars.clone())?;
                Ok(vars)
            }
        }
    }

    /// 合并所有层级（应用优先级规则）
    fn list_merged(&self) -> Result<Vec<EnvVar>> {
        let mut map = HashMap::new();

        // 按优先级从低到高覆盖
        for source in [
            EnvSource::System,
            EnvSource::User,
            EnvSource::Project,
            EnvSource::Local,
        ] {
            let vars = self.list_from_source(&source)?;
            for var in vars {
                map.insert(var.key.clone(), var); // 高优先级覆盖低优先级
            }
        }

        Ok(map.into_values().collect())
    }

    /// 导入 .env 文件到指定层级
    pub fn import_file(&self, file_path: &str, target_source: &EnvSource) -> Result<usize> {
        if !target_source.is_writable() {
            return Err(EnvError::PermissionDenied("目标层级不可写".to_string()));
        }

        let path = std::path::Path::new(file_path);

        if !file_exists(path) {
            return Err(EnvError::FileNotFound(path.to_path_buf()));
        }

        // 确保目录存在
        match target_source {
            EnvSource::User => paths::ensure_config_dir()?,
            EnvSource::Project | EnvSource::Local => paths::ensure_project_dir()?,
            _ => {}
        }

        // 读取并解析文件
        let content = read_file(path)?;
        let vars = DotenvParser::parse(&content, &EnvSource::System)?; // 临时标记为 System

        // 写入目标层级
        let target_path = paths::get_layer_path(target_source)?;
        let existing = if file_exists(&target_path) {
            let existing_content = read_file(&target_path)?;
            DotenvParser::parse(&existing_content, target_source)?
        } else {
            vec![]
        };

        // 去重合并
        let mut final_vars = existing;
        let mut count = 0;

        for var in vars {
            if !final_vars.iter().any(|v| v.key == var.key) {
                final_vars.push(EnvVar::new(var.key, var.value, target_source.clone()));
                count += 1;
            }
        }

        // 写回文件
        let new_content = DotenvParser::serialize(&final_vars);
        write_file_safe(&target_path, &new_content)?;

        if self.config.verbose {
            println!(
                "✓ 从 {} 导入了 {} 个变量到 {:?}",
                file_path, count, target_source
            );
        }

        Ok(count)
    }

    /// 寄出变量到 .env 格式
    pub fn export(&self, source_filter: Option<EnvSource>) -> Result<String> {
        let vars = match source_filter {
            Some(ref source) => self.list_from_source(source)?,
            None => self.list_merged()?, // 默认合并所有
        };

        Ok(DotenvParser::serialize(&vars))
    }

    // ==================== 加密相关方法 ====================

    /// 设置加密变量（使用 SOPS）
    pub fn set_encrypted(&self, key: String, value: String) -> Result<()> {
        // 确保本地目录存在
        paths::ensure_project_dir()?;

        // 获取本地层路径
        let path = paths::get_layer_path(&EnvSource::Local)?;

        // 加密值
        let encryptor = SopsEncryptor::new();
        let encrypted_value = encryptor.encrypt(&value)?;

        // 读取现有变量（如果文件存在）
        let mut vars = if file_exists(&path) {
            let content = read_file(&path)?;
            EncryptedDotenvParser::parse(&content, &EnvSource::Local)?
        } else {
            Vec::new()
        };

        // 更新或添加加密变量
        if let Some(existing) = vars.iter_mut().find(|v| v.key == key) {
            existing.value = encrypted_value.clone();
            existing.encryption_type = EncryptionType::Sops;
        } else {
            vars.push(EncryptedEnvVar::new(
                key.clone(),
                encrypted_value.clone(),
                EnvSource::Local,
                EncryptionType::Sops,
            ));
        }

        // 序列化并写回
        let new_content = EncryptedDotenvParser::serialize(&vars);
        write_file_safe(&path, &new_content)?;

        if self.config.verbose {
            println!("✓ 设置加密变量 {} = {}", key, encrypted_value);
        }

        Ok(())
    }

    /// 获取变量（自动解密）
    pub fn get_decrypted(&self, key: &str) -> Result<Option<String>> {
        // 从高优先级向低优先级查找
        let sources = [
            EnvSource::Local,
            EnvSource::Project,
            EnvSource::User,
            EnvSource::System,
        ];

        for source in sources {
            if let Some(value) = self.get_decrypted_from_source(key, &source)? {
                return Ok(Some(value));
            }
        }

        Ok(None)
    }

    /// 从指定源获取变量（自动解密）
    fn get_decrypted_from_source(&self, key: &str, source: &EnvSource) -> Result<Option<String>> {
        // 系统层特殊处理（系统层不加密）
        if *source == EnvSource::System {
            let system_env = get_system_env()?;
            return Ok(system_env.get(key).cloned());
        }

        // 获取文件路径
        let path = paths::get_layer_path(source)?;

        // 文件不存在
        if !file_exists(&path) {
            return Ok(None);
        }

        // 读取并解析文件（支持加密格式）
        let content = read_file(&path)?;

        // 检查是否包含加密变量
        if EncryptedDotenvParser::has_encrypted(&content) {
            let vars = EncryptedDotenvParser::parse(&content, source)?;

            for var in vars {
                if var.key == key {
                    // 如果是加密的，解密后返回
                    if var.is_encrypted() {
                        let encryptor = SopsEncryptor::new();
                        let decrypted = encryptor.decrypt(&var.value)?;
                        return Ok(Some(decrypted));
                    }
                    // 明文直接返回
                    return Ok(Some(var.value));
                }
            }
        } else {
            // 没有加密变量，使用普通解析器
            let vars = DotenvParser::parse(&content, source)?;

            for var in vars {
                if var.key == key {
                    return Ok(Some(var.value));
                }
            }
        }

        Ok(None)
    }

    /// 列出加密变量（保留加密状态）
    pub fn list_encrypted(&self, source_filter: Option<EnvSource>) -> Result<Vec<EncryptedEnvVar>> {
        match source_filter {
            Some(source) => self.list_encrypted_from_source(&source),
            None => self.list_merged_encrypted(),
        }
    }

    /// 列出指定源的加密变量
    fn list_encrypted_from_source(&self, source: &EnvSource) -> Result<Vec<EncryptedEnvVar>> {
        match source {
            EnvSource::System => {
                // 系统环境变量不加密
                let env = get_system_env()?;
                Ok(env
                    .into_iter()
                    .map(|(k, v)| {
                        EncryptedEnvVar::new(k, v, EnvSource::System, EncryptionType::None)
                    })
                    .collect())
            }
            _ => {
                let path = paths::get_layer_path(source)?;

                if !file_exists(&path) {
                    return Ok(vec![]);
                }

                let content = read_file(&path)?;
                EncryptedDotenvParser::parse(&content, source)
            }
        }
    }

    /// 合并所有层级的加密变量
    fn list_merged_encrypted(&self) -> Result<Vec<EncryptedEnvVar>> {
        let mut map = HashMap::new();

        // 按优先级从低到高覆盖
        for source in [
            EnvSource::System,
            EnvSource::User,
            EnvSource::Project,
            EnvSource::Local,
        ] {
            let vars = self.list_encrypted_from_source(&source)?;
            for var in vars {
                map.insert(var.key.clone(), var); // 高优先级覆盖低优先级
            }
        }

        Ok(map.into_values().collect())
    }

    /// 导出加密变量（保持加密状态）
    pub fn export_encrypted(&self, source_filter: Option<EnvSource>) -> Result<String> {
        let vars = match source_filter {
            Some(ref source) => self.list_encrypted_from_source(source)?,
            None => self.list_merged_encrypted()?,
        };

        Ok(EncryptedDotenvParser::serialize(&vars))
    }

    /// 解密并导出变量
    pub fn export_decrypted(&self, source_filter: Option<EnvSource>) -> Result<String> {
        let vars = match source_filter {
            Some(ref source) => self.list_encrypted_from_source(source)?,
            None => self.list_merged_encrypted()?,
        };

        let encryptor = SopsEncryptor::new();
        let mut lines = Vec::new();

        for var in vars {
            let value = if var.is_encrypted() {
                encryptor.decrypt(&var.value)?
            } else {
                var.value
            };
            lines.push(format!("{}={}", var.key, value));
        }

        Ok(lines.join("\n"))
    }

    /// 解密指定变量
    pub fn decrypt_var(&self, key: &str) -> Result<Option<String>> {
        self.get_decrypted(key)
    }

    /// 检查 SOPS 是否可用
    pub fn check_sops(&self) -> Result<()> {
        if !SopsEncryptor::is_available() {
            return Err(EnvError::EncryptionError(
                "SOPS 未安装或不在 PATH 中\n请安装 SOPS: https://github.com/mozilla/sops"
                    .to_string(),
            ));
        }
        Ok(())
    }

    // ==================== 系统环境变量操作方法 ====================

    /// 设置系统环境变量（永久生效）
    ///
    /// # 参数
    /// * `key` - 变量名称
    /// * `value` - 变量值
    /// * `scope` - 作用域: "global" (用户级) 或 "machine" (系统级)
    ///
    /// # 平台差异
    /// - **Windows**: 支持 global 和 machine
    /// - **Unix/Linux/macOS**: 仅支持 global (写入 shell 配置文件)
    pub fn set_system(&self, key: String, value: String, scope: &str) -> Result<()> {
        use crate::utils::system_env::SystemEnvWriter;

        // 验证作用域
        if scope != "global" && scope != "machine" {
            return Err(EnvError::InvalidArgument(
                "scope 必须是 'global' 或 'machine'".to_string(),
            ));
        }

        #[cfg(target_os = "windows")]
        {
            match scope {
                "machine" => SystemEnvWriter::set_machine_var(&key, &value),
                _ => SystemEnvWriter::set_user_var(&key, &value),
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if scope == "machine" {
                return Err(EnvError::PermissionDenied(
                    "Unix 系统不支持机器级环境变量，仅支持用户级 (global)".to_string(),
                ));
            }

            SystemEnvWriter::set_user_var(&key, &value)
        }
    }

    /// 删除系统环境变量
    ///
    /// # 参数
    /// * `key` - 变量名称
    /// * `scope` - 作用域: "global" (用户级) 或 "machine" (系统级)
    pub fn unset_system(&self, key: String, scope: &str) -> Result<()> {
        use crate::utils::system_env::SystemEnvWriter;

        // 验证作用域
        if scope != "global" && scope != "machine" {
            return Err(EnvError::InvalidArgument(
                "scope 必须是 'global' 或 'machine'".to_string(),
            ));
        }

        #[cfg(target_os = "windows")]
        {
            SystemEnvWriter::unset_var(&key, scope)
        }

        #[cfg(not(target_os = "windows"))]
        {
            if scope == "machine" {
                return Err(EnvError::PermissionDenied(
                    "Unix 系统不支持机器级环境变量操作".to_string(),
                ));
            }

            SystemEnvWriter::unset_var(&key, scope)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;
    use tempfile::TempDir;

    // 辅助函数：创建临时测试环境（未使用，保留供未来使用）
    #[allow(dead_code)]
    fn create_test_environment() -> (TempDir, Store) {
        let temp_dir = tempfile::tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();

        // 切换到临时目录
        std::env::set_current_dir(&temp_dir).unwrap();

        let config = Config { verbose: false };
        let store = Store::new(config);

        // 恢复原目录
        std::env::set_current_dir(original_dir).unwrap();

        (temp_dir, store)
    }

    // 辅助函数：设置用户级配置
    fn setup_user_config(content: &str) {
        let home_dir = dirs::home_dir().unwrap();
        let user_dir = home_dir.join(".envcli");
        fs::create_dir_all(&user_dir).unwrap();
        let user_file = user_dir.join("user.env");
        fs::write(&user_file, content).unwrap();
    }

    // 辅助函数：清理用户级配置
    fn cleanup_user_config() {
        let home_dir = dirs::home_dir().unwrap();
        let user_dir = home_dir.join(".envcli");
        let user_file = user_dir.join("user.env");
        if user_file.exists() {
            fs::remove_file(&user_file).unwrap();
        }
    }

    mod store_construction_tests {
        use super::*;

        #[test]
        fn test_store_new() {
            let config = Config { verbose: true };
            let store = Store::new(config.clone());

            // Store 应该包含配置
            assert!(store.config.verbose);
        }

        #[test]
        fn test_store_clone() {
            let config = Config { verbose: true };
            let store1 = Store::new(config);
            let store2 = store1.clone();

            // 克隆的 store 应该独立
            assert!(store2.config.verbose);
        }
    }

    #[serial]
    mod get_tests {
        use super::*;

        #[test]
        fn test_get_from_local() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            // 创建本地变量
            store
                .set("LOCAL_VAR".to_string(), "local_value".to_string())
                .unwrap();

            // 获取变量
            let result = store.get("LOCAL_VAR").unwrap();
            assert_eq!(result, Some("local_value".to_string()));

            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_get_from_project() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            // 创建项目级文件
            let project_dir = temp_dir.path().join(".envcli");
            fs::create_dir_all(&project_dir).unwrap();
            let project_file = project_dir.join("project.env");
            fs::write(&project_file, "PROJECT_VAR=project_value").unwrap();

            let result = store.get("PROJECT_VAR").unwrap();
            assert_eq!(result, Some("project_value".to_string()));

            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_get_from_user() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            // 设置用户级配置
            setup_user_config("USER_VAR=user_value");

            let result = store.get("USER_VAR").unwrap();
            assert_eq!(result, Some("user_value".to_string()));

            cleanup_user_config();
            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_get_priority_order() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            // 设置系统变量
            unsafe {
                std::env::set_var("PRIORITY_VAR", "system");
            }

            // 设置用户级
            setup_user_config("PRIORITY_VAR=user");

            // 设置项目级
            let project_dir = temp_dir.path().join(".envcli");
            fs::create_dir_all(&project_dir).unwrap();
            let project_file = project_dir.join("project.env");
            fs::write(&project_file, "PRIORITY_VAR=project").unwrap();

            // 设置本地级
            store
                .set("PRIORITY_VAR".to_string(), "local".to_string())
                .unwrap();

            // 应该返回本地级（最高优先级）
            let result = store.get("PRIORITY_VAR").unwrap();
            assert_eq!(result, Some("local".to_string()));

            // 清理
            unsafe {
                std::env::remove_var("PRIORITY_VAR");
            }
            cleanup_user_config();
            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_get_not_found() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            let result = store.get("NONEXISTENT_VAR").unwrap();
            assert!(result.is_none());

            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_get_system_env() {
            // 清除系统环境缓存以确保测试独立性
            crate::utils::paths::clear_system_env_cache();

            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            // 设置系统变量
            unsafe {
                std::env::set_var("TEST_SYSTEM_VAR_999", "system_value");
            }

            let result = store.get("TEST_SYSTEM_VAR_999").unwrap();
            assert_eq!(result, Some("system_value".to_string()));

            unsafe {
                std::env::remove_var("TEST_SYSTEM_VAR_999");
            }
            std::env::set_current_dir(original_dir).unwrap();

            // 清除缓存，避免影响其他测试
            crate::utils::paths::clear_system_env_cache();
        }
    }

    #[serial]
    mod set_tests {
        use super::*;

        #[test]
        fn test_set_creates_file() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            store.set("KEY".to_string(), "VALUE".to_string()).unwrap();

            let local_path = paths::get_layer_path(&EnvSource::Local).unwrap();
            assert!(local_path.exists());

            let content = fs::read_to_string(&local_path).unwrap();
            assert!(content.contains("KEY=VALUE"));

            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_set_multiple_vars() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            store.set("VAR1".to_string(), "value1".to_string()).unwrap();
            store.set("VAR2".to_string(), "value2".to_string()).unwrap();

            let local_path = paths::get_layer_path(&EnvSource::Local).unwrap();
            let content = fs::read_to_string(&local_path).unwrap();

            assert!(content.contains("VAR1=value1"));
            assert!(content.contains("VAR2=value2"));

            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_set_duplicate_key() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            store.set("KEY".to_string(), "value1".to_string()).unwrap();
            store.set("KEY".to_string(), "value2".to_string()).unwrap();

            let local_path = paths::get_layer_path(&EnvSource::Local).unwrap();
            let content = fs::read_to_string(&local_path).unwrap();

            // 应该只有一行，值为最新的
            let lines: Vec<&str> = content.lines().collect();
            assert_eq!(lines.len(), 1);
            assert!(content.contains("KEY=value2"));

            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_set_with_special_chars() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            store
                .set("SPECIAL".to_string(), "value with spaces".to_string())
                .unwrap();

            let result = store.get("SPECIAL").unwrap();
            assert_eq!(result, Some("value with spaces".to_string()));

            std::env::set_current_dir(original_dir).unwrap();
        }
    }

    #[serial]
    mod unset_tests {
        use super::*;

        #[test]
        fn test_unset_existing_var() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            store
                .set("TO_DELETE".to_string(), "value".to_string())
                .unwrap();
            let result = store.unset("TO_DELETE").unwrap();

            assert!(result);
            assert!(store.get("TO_DELETE").unwrap().is_none());

            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_unset_nonexistent_var() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            let result = store.unset("NONEXISTENT").unwrap();

            assert!(!result);

            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_unset_deletes_file_when_empty() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            store
                .set("ONLY_VAR".to_string(), "value".to_string())
                .unwrap();
            store.unset("ONLY_VAR").unwrap();

            let local_path = paths::get_layer_path(&EnvSource::Local).unwrap();
            assert!(!local_path.exists());

            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_unset_keeps_other_vars() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            store
                .set("KEEP".to_string(), "keep_value".to_string())
                .unwrap();
            store
                .set("DELETE".to_string(), "delete_value".to_string())
                .unwrap();

            store.unset("DELETE").unwrap();

            assert_eq!(store.get("KEEP").unwrap(), Some("keep_value".to_string()));
            assert!(store.get("DELETE").unwrap().is_none());

            std::env::set_current_dir(original_dir).unwrap();
        }
    }

    #[serial]
    mod list_tests {
        use super::*;

        #[test]
        fn test_list_from_local() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            store.set("VAR1".to_string(), "value1".to_string()).unwrap();
            store.set("VAR2".to_string(), "value2".to_string()).unwrap();

            let vars = store.list(Some(EnvSource::Local)).unwrap();
            assert_eq!(vars.len(), 2);

            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_list_from_user() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            setup_user_config("USER_VAR=user_value");

            let vars = store.list(Some(EnvSource::User)).unwrap();
            assert!(!vars.is_empty());

            cleanup_user_config();
            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_list_from_system() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            unsafe {
                std::env::set_var("TEST_LIST_SYSTEM", "value");
            }

            let vars = store.list(Some(EnvSource::System)).unwrap();
            assert!(!vars.is_empty());

            unsafe {
                std::env::remove_var("TEST_LIST_SYSTEM");
            }
            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_list_merged() {
            // 清除文件缓存以确保测试独立性
            let config_init = Config { verbose: false };
            let store_init = Store::new(config_init);
            store_init.clear_cache();

            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            // 设置不同层级的变量
            unsafe {
                std::env::set_var("MERGE_VAR", "system");
            }
            setup_user_config("MERGE_VAR=user\nUSER_ONLY=user_only");

            let project_dir = temp_dir.path().join(".envcli");
            fs::create_dir_all(&project_dir).unwrap();
            let project_file = project_dir.join("project.env");
            fs::write(
                &project_file,
                "MERGE_VAR=project\nPROJECT_ONLY=project_only",
            )
            .unwrap();

            store
                .set("MERGE_VAR".to_string(), "local".to_string())
                .unwrap();
            store
                .set("LOCAL_ONLY".to_string(), "local_only".to_string())
                .unwrap();

            let vars = store.list(None).unwrap();

            // 应该合并所有，本地覆盖
            let merge_var = vars.iter().find(|v| v.key == "MERGE_VAR").unwrap();
            assert_eq!(merge_var.value, "local");

            // 检查其他变量
            assert!(vars.iter().any(|v| v.key == "USER_ONLY"));
            assert!(vars.iter().any(|v| v.key == "PROJECT_ONLY"));
            assert!(vars.iter().any(|v| v.key == "LOCAL_ONLY"));

            unsafe {
                std::env::remove_var("MERGE_VAR");
            }
            cleanup_user_config();
            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_list_empty_source() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            let vars = store.list(Some(EnvSource::Local)).unwrap();
            assert!(vars.is_empty());

            std::env::set_current_dir(original_dir).unwrap();
        }
    }

    #[serial]
    mod import_export_tests {
        use super::*;

        #[test]
        fn test_import_file() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            // 创建源文件
            let source_file = temp_dir.path().join("source.env");
            fs::write(
                &source_file,
                "IMPORT_VAR=import_value\nANOTHER=another_value",
            )
            .unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            let count = store
                .import_file(source_file.to_str().unwrap(), &EnvSource::Local)
                .unwrap();
            assert_eq!(count, 2);

            // 验证导入
            assert_eq!(
                store.get("IMPORT_VAR").unwrap(),
                Some("import_value".to_string())
            );
            assert_eq!(
                store.get("ANOTHER").unwrap(),
                Some("another_value".to_string())
            );

            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_import_file_duplicate() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            // 先设置一个变量
            store
                .set("EXISTING".to_string(), "old".to_string())
                .unwrap();

            // 导入包含相同键的文件
            let source_file = temp_dir.path().join("source.env");
            fs::write(&source_file, "EXISTING=new\nNEW_VAR=new_value").unwrap();

            let count = store
                .import_file(source_file.to_str().unwrap(), &EnvSource::Local)
                .unwrap();

            // 应该只导入新的，不覆盖已存在的
            assert_eq!(count, 1);
            assert_eq!(store.get("EXISTING").unwrap(), Some("old".to_string()));
            assert_eq!(store.get("NEW_VAR").unwrap(), Some("new_value".to_string()));

            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_import_file_not_found() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            let result = store.import_file("/nonexistent/file.env", &EnvSource::Local);
            assert!(result.is_err());

            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_import_to_system_error() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            let source_file = temp_dir.path().join("source.env");
            fs::write(&source_file, "VAR=value").unwrap();

            let result = store.import_file(source_file.to_str().unwrap(), &EnvSource::System);
            assert!(result.is_err());

            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_export_all() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            store
                .set("EXPORT_VAR".to_string(), "export_value".to_string())
                .unwrap();

            let result = store.export(None).unwrap();
            assert!(result.contains("EXPORT_VAR=export_value"));

            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_export_specific_source() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            store
                .set("LOCAL_VAR".to_string(), "local_value".to_string())
                .unwrap();

            let result = store.export(Some(EnvSource::Local)).unwrap();
            assert!(result.contains("LOCAL_VAR=local_value"));

            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_export_empty() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            let result = store.export(None).unwrap();
            // 应该包含系统环境变量
            assert!(!result.is_empty());

            std::env::set_current_dir(original_dir).unwrap();
        }
    }

    #[serial]
    mod encryption_tests {
        use super::*;

        #[test]
        fn test_set_encrypted_variable() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            // 检查 SOPS 是否可用，如果不可用则跳过测试
            if !SopsEncryptor::is_available() {
                println!("SOPS 未安装，跳过加密测试");
                let _ = std::env::set_current_dir(original_dir);
                return;
            }

            // 设置加密变量
            let result =
                store.set_encrypted("TEST_SECRET".to_string(), "my_secret_value".to_string());
            assert!(result.is_ok());

            // 验证文件存在并包含加密内容
            let path = paths::get_layer_path(&EnvSource::Local).unwrap();
            assert!(path.exists());

            let content = paths::read_file(&path).unwrap();
            assert!(content.contains("TEST_SECRET=ENC[SOPS:"));
            assert!(!content.contains("my_secret_value")); // 不应包含明文

            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_get_decrypted_variable() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            if !SopsEncryptor::is_available() {
                println!("SOPS 未安装，跳过加密测试");
                let _ = std::env::set_current_dir(original_dir);
                return;
            }

            // 设置加密变量
            store
                .set_encrypted("SECRET_KEY".to_string(), "secret_value_123".to_string())
                .unwrap();

            // 获取并解密
            let result = store.get_decrypted("SECRET_KEY").unwrap();
            assert_eq!(result, Some("secret_value_123".to_string()));

            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_list_encrypted_variables() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            if !SopsEncryptor::is_available() {
                println!("SOPS 未安装，跳过加密测试");
                let _ = std::env::set_current_dir(original_dir);
                return;
            }

            // 设置混合变量（明文和加密）
            store
                .set("PLAIN_VAR".to_string(), "plain_value".to_string())
                .unwrap();
            store
                .set_encrypted("ENCRYPTED_VAR".to_string(), "encrypted_value".to_string())
                .unwrap();

            // 列出加密变量
            let encrypted_vars = store.list_encrypted(Some(EnvSource::Local)).unwrap();
            assert_eq!(encrypted_vars.len(), 2);

            // 检查加密变量
            let encrypted_var = encrypted_vars
                .iter()
                .find(|v| v.key == "ENCRYPTED_VAR")
                .unwrap();
            assert!(encrypted_var.is_encrypted());
            assert_eq!(encrypted_var.encryption_type, EncryptionType::Sops);

            // 检查明文变量
            let plain_var = encrypted_vars
                .iter()
                .find(|v| v.key == "PLAIN_VAR")
                .unwrap();
            assert!(!plain_var.is_encrypted());
            assert_eq!(plain_var.encryption_type, EncryptionType::None);

            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_mixed_encryption_layers() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            if !SopsEncryptor::is_available() {
                println!("SOPS 未安装，跳过加密测试");
                let _ = std::env::set_current_dir(original_dir);
                return;
            }

            // 设置本地加密变量
            store
                .set_encrypted("LOCAL_SECRET".to_string(), "local_value".to_string())
                .unwrap();

            // 设置本地明文变量
            store
                .set("LOCAL_PLAIN".to_string(), "local_plain_value".to_string())
                .unwrap();

            // 获取加密变量（应该自动解密）
            let result = store.get_decrypted("LOCAL_SECRET").unwrap();
            assert_eq!(result, Some("local_value".to_string()));

            // 获取明文变量
            let result = store.get("LOCAL_PLAIN").unwrap();
            assert_eq!(result, Some("local_plain_value".to_string()));

            std::env::set_current_dir(original_dir).unwrap();
        }
    }

    #[serial]
    mod integration_tests {
        use super::*;

        #[test]
        fn test_full_workflow() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            // 1. 设置变量
            store
                .set("DB_HOST".to_string(), "localhost".to_string())
                .unwrap();
            store
                .set("DB_PORT".to_string(), "5432".to_string())
                .unwrap();

            // 2. 获取变量
            assert_eq!(store.get("DB_HOST").unwrap(), Some("localhost".to_string()));

            // 3. 列出变量
            let vars = store.list(Some(EnvSource::Local)).unwrap();
            assert_eq!(vars.len(), 2);

            // 4. 导出
            let exported = store.export(None).unwrap();
            assert!(exported.contains("DB_HOST=localhost"));
            assert!(exported.contains("DB_PORT=5432"));

            // 5. 删除变量
            store.unset("DB_HOST").unwrap();
            assert!(store.get("DB_HOST").unwrap().is_none());

            std::env::set_current_dir(original_dir).unwrap();
        }
    }
}
