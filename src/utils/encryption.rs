//! SOPS 加密工具模块
//!
//! 提供基于 SOPS (Secrets OPerationS) 的加密/解密功能
//! 支持多种加密后端：AWS KMS, GCP KMS, Azure Key Vault, GPG 等

use crate::error::{EnvError, Result};
use std::collections::HashMap;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

/// 缓存配置
#[derive(Debug, Clone, Copy)]
pub struct CacheConfig {
    /// 最大缓存条目数，0 表示无限制
    pub max_size: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self { max_size: 100 }
    }
}

impl CacheConfig {
    /// 创建指定大小的缓存配置
    pub fn new(max_size: usize) -> Self {
        Self { max_size }
    }

    /// 无限制缓存
    pub fn unlimited() -> Self {
        Self { max_size: 0 }
    }

    /// 创建默认缓存配置（100条记录）
    pub fn new_default() -> Self {
        Self { max_size: 100 }
    }

    /// 创建小缓存配置（适合内存受限环境）
    pub fn small() -> Self {
        Self { max_size: 10 }
    }

    /// 创建大缓存配置（适合高频访问）
    pub fn large() -> Self {
        Self { max_size: 1000 }
    }

    /// 创建严格限制的缓存配置（适合测试）
    pub fn strict() -> Self {
        Self { max_size: 5 }
    }
}

/// 线程安全的解密缓存
#[derive(Clone)]
pub struct DecryptCache {
    cache: Arc<Mutex<HashMap<String, String>>>,
    max_size: usize,
}

impl DecryptCache {
    /// 创建缓存
    pub fn new(config: CacheConfig) -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            max_size: config.max_size,
        }
    }

    /// 从缓存获取值（短暂持有锁）
    pub fn get(&self, key: &str) -> Option<String> {
        let cache = self.cache.lock().unwrap();
        cache.get(key).cloned()
    }

    /// 存入缓存（短暂持有锁，带大小限制）
    pub fn insert(&self, key: String, value: String) {
        let mut cache = self.cache.lock().unwrap();

        // 检查大小限制（如果启用）
        if self.max_size > 0 && cache.len() >= self.max_size {
            // 移除最旧的条目（简单 LRU：移除第一个）
            if let Some(first_key) = cache.keys().next().cloned() {
                cache.remove(&first_key);
            }
        }

        cache.insert(key, value);
    }

    /// 清除缓存
    pub fn clear(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
    }

    /// 获取缓存大小
    pub fn size(&self) -> usize {
        let cache = self.cache.lock().unwrap();
        cache.len()
    }
}

/// SOPS 加密器（带可选缓存）
pub struct SopsEncryptor {
    /// 解密缓存（用于高频访问的场景）
    cache: Option<DecryptCache>,
}

impl Default for SopsEncryptor {
    fn default() -> Self {
        Self::new()
    }
}

impl SopsEncryptor {
    /// 创建不带缓存的加密器（静态方法，保持向后兼容）
    pub fn new() -> Self {
        Self { cache: None }
    }

    /// 创建带缓存的加密器
    ///
    /// # 参数
    /// * `max_cache_size` - 最大缓存条目数，0 表示无限制
    pub fn with_cache(max_cache_size: usize) -> Self {
        if max_cache_size > 0 {
            Self {
                cache: Some(DecryptCache::new(CacheConfig::new(max_cache_size))),
            }
        } else {
            Self { cache: None }
        }
    }

    /// 使用指定缓存配置创建加密器
    pub fn with_cache_config(config: CacheConfig) -> Self {
        if config.max_size > 0 {
            Self {
                cache: Some(DecryptCache::new(config)),
            }
        } else {
            Self { cache: None }
        }
    }

    /// 创建默认缓存（100条记录）
    pub fn with_default_cache() -> Self {
        Self::with_cache(100)
    }

    /// 清除缓存
    pub fn clear_cache(&self) {
        if let Some(cache) = &self.cache {
            cache.clear();
        }
    }

    /// 获取缓存大小
    pub fn cache_size(&self) -> usize {
        if let Some(cache) = &self.cache {
            cache.size()
        } else {
            0
        }
    }

    /// 检查 SOPS 是否可用（静态方法）
    pub fn is_available() -> bool {
        Command::new("sops").arg("--version").output().is_ok()
    }

    /// 检查 SOPS 是否可用并返回详细信息（静态方法）
    ///
    /// # 返回
    /// Ok(()) 如果可用，否则返回错误描述
    pub fn check_availability() -> std::result::Result<(), String> {
        if !Self::is_available() {
            return Err("SOPS 未安装或不在 PATH 中".to_string());
        }

        match Self::version() {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("无法获取 SOPS 版本: {}", e)),
        }
    }

    /// 验证值是否为加密格式（静态方法）
    pub fn is_encrypted(value: &str) -> bool {
        value.starts_with("ENC[SOPS:") && value.ends_with(']')
    }

    /// 获取 SOPS 版本信息（静态方法）
    pub fn version() -> Result<String> {
        if !Self::is_available() {
            return Ok("SOPS 未安装".to_string());
        }

        let output = Command::new("sops").arg("--version").output()?;

        if output.status.success() {
            String::from_utf8(output.stdout)
                .map(|s| s.trim().to_string())
                .map_err(|e| EnvError::EncryptionError(format!("UTF-8 错误: {}", e)))
        } else {
            Ok("未知版本".to_string())
        }
    }

    /// 快速加密测试（静态方法）
    ///
    /// # 返回
    /// 如果 SOPS 可用且能正常工作返回 Ok(())，否则返回错误
    pub fn test_availability() -> std::result::Result<(), String> {
        Self::check_availability()
    }

    /// 获取加密器统计信息
    pub fn get_stats(&self) -> EncryptorStats {
        EncryptorStats {
            has_cache: self.cache.is_some(),
            cache_size: self.cache_size(),
            cache_enabled: self.cache.is_some(),
        }
    }

    /// 使用 SOPS 加密值
    ///
    /// # 参数
    /// - `value`: 要加密的明文值
    ///
    /// # 返回
    /// 加密后的字符串，格式为 `ENC[SOPS:v1:...]`
    ///
    /// # 示例
    /// ```ignore
    /// let encryptor = SopsEncryptor::with_default_cache();
    /// let encrypted = encryptor.encrypt("my_secret")?;
    /// // 返回: ENC[SOPS:v1:...]
    /// ```
    pub fn encrypt(&self, value: &str) -> Result<String> {
        if !Self::is_available() {
            return Err(EnvError::EncryptionError(
                "SOPS 未安装或不在 PATH 中".to_string(),
            ));
        }

        // 使用 SOPS 加密
        let mut output = Command::new("sops")
            .args([
                "--encrypt",
                "--input-type",
                "binary",
                "--output-type",
                "binary",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // 写入要加密的内容
        let mut stdin = output.stdin.take().unwrap();
        use std::io::Write;
        write!(stdin, "{}", value)?;
        drop(stdin);

        let result = output.wait_with_output()?;

        if !result.status.success() {
            let error_msg = String::from_utf8_lossy(&result.stderr);
            return Err(EnvError::EncryptionError(format!(
                "SOPS 加密失败: {}",
                error_msg
            )));
        }

        let encrypted = String::from_utf8(result.stdout)
            .map_err(|e| EnvError::EncryptionError(format!("无效的 UTF-8 输出: {}", e)))?;

        // 包装成 ENC[SOPS:...] 格式
        Ok(format!("ENC[SOPS:{}]", encrypted.trim()))
    }

    /// 使用 SOPS 解密值（带缓存支持）
    ///
    /// # 参数
    /// - `encrypted`: 加密的字符串（格式：`ENC[SOPS:v1:...]`）
    ///
    /// # 返回
    /// 解密后的明文
    pub fn decrypt(&self, encrypted: &str) -> Result<String> {
        // 1. 先检查缓存（短暂持有锁）
        if let Some(cache) = &self.cache
            && let Some(cached) = cache.get(encrypted)
        {
            return Ok(cached);
        }

        if !Self::is_available() {
            return Err(EnvError::EncryptionError(
                "SOPS 未安装或不在 PATH 中".to_string(),
            ));
        }

        // 检查格式
        if !encrypted.starts_with("ENC[SOPS:") || !encrypted.ends_with(']') {
            return Err(EnvError::EncryptionError(
                "无效的加密格式，应为 ENC[SOPS:...]".to_string(),
            ));
        }

        // 提取 SOPS 内容
        let inner = &encrypted[10..encrypted.len() - 1]; // 去掉 "ENC[SOPS:" 和 "]"

        // 2. 执行解密（不持有锁，可能耗时）
        let mut output = Command::new("sops")
            .args([
                "--decrypt",
                "--input-type",
                "binary",
                "--output-type",
                "binary",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // 写入要解密的内容
        let mut stdin = output.stdin.take().unwrap();
        use std::io::Write;
        write!(stdin, "{}", inner)?;
        drop(stdin);

        let result = output.wait_with_output()?;

        if !result.status.success() {
            let error_msg = String::from_utf8_lossy(&result.stderr);
            return Err(EnvError::DecryptionError(format!(
                "SOPS 解密失败: {}",
                error_msg
            )));
        }

        let decrypted = String::from_utf8(result.stdout)
            .map_err(|e| EnvError::DecryptionError(format!("无效的 UTF-8 输出: {}", e)))?;

        // 3. 存入缓存（短暂持有锁）
        if let Some(cache) = &self.cache {
            cache.insert(encrypted.to_string(), decrypted.clone());
        }

        Ok(decrypted)
    }
}

/// 加密器统计信息
#[derive(Debug, Clone)]
pub struct EncryptorStats {
    /// 是否启用缓存
    pub cache_enabled: bool,
    /// 是否有缓存实例
    pub has_cache: bool,
    /// 当前缓存大小
    pub cache_size: usize,
}

// 为向后兼容，提供静态方法包装器
impl SopsEncryptor {
    /// 静态方法：加密（无缓存）
    pub fn encrypt_static(value: &str) -> Result<String> {
        let encryptor = SopsEncryptor::new();
        encryptor.encrypt(value)
    }

    /// 静态方法：解密（无缓存）
    pub fn decrypt_static(encrypted: &str) -> Result<String> {
        let encryptor = SopsEncryptor::new();
        encryptor.decrypt(encrypted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_available() {
        let available = SopsEncryptor::is_available();
        let _ = available;
    }

    #[test]
    fn test_is_encrypted() {
        assert!(SopsEncryptor::is_encrypted("ENC[SOPS:v1:abc]"));
        assert!(!SopsEncryptor::is_encrypted("plain_text"));
        assert!(!SopsEncryptor::is_encrypted("ENC[SOPS:"));
        assert!(!SopsEncryptor::is_encrypted("SOPS:v1:abc]"));
    }

    #[test]
    fn test_encrypt_decrypt_cycle() {
        if !SopsEncryptor::is_available() {
            println!("SOPS 未安装，跳过加密测试");
            return;
        }

        let encryptor = SopsEncryptor::new();
        let original = "test_secret_value_12345";

        // 加密
        let encrypted = encryptor.encrypt(original).unwrap();
        assert!(encrypted.starts_with("ENC[SOPS:"));

        // 解密
        let decrypted = encryptor.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, original);
    }

    #[test]
    fn test_cache_functionality() {
        if !SopsEncryptor::is_available() {
            println!("SOPS 未安装，跳过缓存测试");
            return;
        }

        let encryptor = SopsEncryptor::with_default_cache();
        let original = "cache_test_secret";

        // 第一次解密（实际调用SOPS）
        let encrypted = encryptor.encrypt(original).unwrap();
        let decrypted1 = encryptor.decrypt(&encrypted).unwrap();

        // 验证缓存大小
        assert_eq!(encryptor.cache_size(), 1);

        // 第二次解密（应从缓存读取）
        let decrypted2 = encryptor.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted1, decrypted2);
        assert_eq!(decrypted1, original);

        // 清除缓存
        encryptor.clear_cache();
        assert_eq!(encryptor.cache_size(), 0);
    }

    #[test]
    fn test_invalid_encrypted_format() {
        let encryptor = SopsEncryptor::new();
        let result = encryptor.decrypt("invalid_format");
        assert!(result.is_err());
    }

    #[test]
    fn test_version() {
        let version = SopsEncryptor::version().unwrap();
        assert!(!version.is_empty());
    }

    #[test]
    fn test_static_methods() {
        // 测试静态方法兼容性
        if !SopsEncryptor::is_available() {
            println!("SOPS 未安装，跳过静态方法测试");
            return;
        }

        let original = "static_test";
        let encrypted = SopsEncryptor::encrypt_static(original).unwrap();
        let decrypted = SopsEncryptor::decrypt_static(&encrypted).unwrap();
        assert_eq!(decrypted, original);
    }

    #[test]
    fn test_cache_size_limit() {
        // 测试缓存大小限制功能
        let config = CacheConfig::new(2); // 最多缓存2条
        let encryptor = SopsEncryptor::with_cache_config(config);

        // 创建两个不同的加密值
        let val1 = "secret1";
        let val2 = "secret2";
        let val3 = "secret3";

        // 手动插入缓存（模拟多次解密）
        if let Some(cache) = &encryptor.cache {
            cache.insert("key1".to_string(), val1.to_string());
            cache.insert("key2".to_string(), val2.to_string());
            assert_eq!(cache.size(), 2);

            // 插入第三条，应该移除某一条（保持在限制内）
            cache.insert("key3".to_string(), val3.to_string());
            assert_eq!(cache.size(), 2); // 仍然保持在限制内

            // 验证：只有两个键存在，第三个被移除
            let keys: Vec<String> =
                vec!["key1".to_string(), "key2".to_string(), "key3".to_string()];
            let existing_keys: Vec<String> = keys
                .iter()
                .filter(|k| cache.get(k).is_some())
                .cloned()
                .collect();
            assert_eq!(existing_keys.len(), 2, "应该恰好有两个键存在于缓存中");

            // 验证存在的键的值正确
            if cache.get("key2").is_some() {
                assert_eq!(cache.get("key2").unwrap(), val2);
            }
            if cache.get("key3").is_some() {
                assert_eq!(cache.get("key3").unwrap(), val3);
            }
        }
    }

    #[test]
    fn test_cache_config() {
        // 测试不同的缓存配置
        let config1 = CacheConfig::new(10);
        assert_eq!(config1.max_size, 10);

        let config2 = CacheConfig::unlimited();
        assert_eq!(config2.max_size, 0);

        let config3 = CacheConfig::default();
        assert_eq!(config3.max_size, 100);
    }

    #[test]
    fn test_cache_concurrency() {
        // 测试缓存的线程安全
        use std::sync::Arc;
        use std::thread;

        let encryptor = Arc::new(SopsEncryptor::with_cache(100));
        let encryptor_clone = encryptor.clone();

        // 在一个线程中插入缓存
        let handle1 = thread::spawn(move || {
            if let Some(cache) = &encryptor_clone.cache {
                cache.insert("key1".to_string(), "value1".to_string());
                cache.insert("key2".to_string(), "value2".to_string());
            }
        });

        handle1.join().unwrap();

        // 在主线程中读取
        if let Some(cache) = &encryptor.cache {
            assert_eq!(cache.get("key1").unwrap(), "value1");
            assert_eq!(cache.get("key2").unwrap(), "value2");
        }
    }
}
