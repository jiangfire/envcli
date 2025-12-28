//! 插件签名验证
//!
//! 提供插件签名的生成和验证功能，确保插件来源可信

use crate::plugin::types::{PluginSignature, SignatureAlgorithm, PluginMetadata};
use ring::signature::{Ed25519KeyPair, ED25519, UnparsedPublicKey};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH, Instant, Duration};
use thiserror::Error;

/// 签名验证错误
#[derive(Error, Debug)]
pub enum SignatureError {
    #[error("插件未签名")]
    Unsigned,

    #[error("签名验证失败: {reason} (算法: {algorithm}, 公钥: {public_key})")]
    VerificationFailed {
        reason: String,
        algorithm: SignatureAlgorithm,
        public_key: String,
    },

    #[error("签名已过期: 签名于 {signed_at}，已超过 {max_age} 天")]
    Expired {
        signed_at: u64,
        max_age: u64,
    },

    #[error("不支持的签名算法: {0}")]
    UnsupportedAlgorithm(String),

    #[error("签名时间无效: 签名时间 {signed_at} 是未来时间 (当前: {now})")]
    InvalidTimestamp { signed_at: u64, now: u64 },

    #[error("时钟偏差过大: 签名时间 {signed_at} 与当前时间 {now} 相差 {skew} 秒，超过允许的 {max_skew} 秒")]
    ClockSkewTooLarge { signed_at: u64, now: u64, skew: u64, max_skew: u64 },

    #[error("检测到重放攻击: 签名已被使用过 (哈希: {signature_hash})")]
    ReplayAttackDetected { signature_hash: String },

    #[error("签名生成失败: {0}")]
    SigningFailed(String),

    #[error("JSON 序列化失败: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("十六进制解码失败: {0}")]
    HexDecodeError(#[from] hex::FromHexError),
}

/// 时间戳验证配置
#[derive(Debug, Clone)]
pub struct TimestampConfig {
    /// 签名最大有效期（秒），默认 365 天
    pub max_age_seconds: u64,
    /// 时钟容忍度（秒），用于处理系统时钟不同步，默认 5 分钟
    pub clock_tolerance_seconds: u64,
    /// 最大时钟偏差（秒），用于检测系统时钟被篡改，默认 24 小时
    pub max_clock_skew_seconds: u64,
    /// 是否要求签名时间在合理范围内（防止使用过于陈旧的签名）
    pub require_recent_signature: bool,
}

impl Default for TimestampConfig {
    fn default() -> Self {
        Self {
            max_age_seconds: 365 * 24 * 60 * 60,
            clock_tolerance_seconds: 5 * 60,
            max_clock_skew_seconds: 24 * 60 * 60, // 24小时
            require_recent_signature: false,
        }
    }
}

impl TimestampConfig {
    /// 创建自定义配置
    pub fn new(max_age_days: u64, clock_tolerance_minutes: u64) -> Self {
        Self {
            max_age_seconds: max_age_days * 24 * 60 * 60,
            clock_tolerance_seconds: clock_tolerance_minutes * 60,
            max_clock_skew_seconds: 24 * 60 * 60,
            require_recent_signature: false,
        }
    }

    /// 创建宽松配置（适合开发环境）
    pub fn lax() -> Self {
        Self {
            max_age_seconds: 365 * 24 * 60 * 60, // 1年
            clock_tolerance_seconds: 60 * 60,    // 1小时
            max_clock_skew_seconds: 24 * 60 * 60 * 7, // 7天
            require_recent_signature: false,
        }
    }

    /// 创建标准配置（适合生产环境）
    pub fn standard() -> Self {
        Self::default()
    }

    /// 创建严格配置（适合高安全环境）
    pub fn strict() -> Self {
        Self {
            max_age_seconds: 7 * 24 * 60 * 60, // 7天
            clock_tolerance_seconds: 5 * 60,    // 5分钟
            max_clock_skew_seconds: 3600, // 1小时
            require_recent_signature: true,
        }
    }

    /// 创建高安全配置（极短有效期）
    pub fn high_security() -> Self {
        Self {
            max_age_seconds: 24 * 60 * 60, // 1天
            clock_tolerance_seconds: 2 * 60,    // 2分钟
            max_clock_skew_seconds: 300, // 5分钟
            require_recent_signature: true,
        }
    }

    /// 启用近期签名要求
    pub fn with_recent_signature_requirement(mut self) -> Self {
        self.require_recent_signature = true;
        self
    }

    /// 设置最大时钟偏差
    pub fn with_max_clock_skew(mut self, skew_seconds: u64) -> Self {
        self.max_clock_skew_seconds = skew_seconds;
        self
    }

    /// 设置最大签名年龄
    pub fn with_max_age(mut self, days: u64) -> Self {
        self.max_age_seconds = days * 24 * 60 * 60;
        self
    }

    /// 设置时钟容忍度
    pub fn with_clock_tolerance(mut self, minutes: u64) -> Self {
        self.clock_tolerance_seconds = minutes * 60;
        self
    }
}

/// 缓存条目（包含时间戳）
struct CacheEntry {
    used_at: Instant,
}

/// 签名缓存（用于重放攻击防护）
pub struct SignatureCache {
    /// 已验证的签名哈希集合（包含时间戳）
    used_signatures: HashMap<String, CacheEntry>,
    /// 缓存条目最大存活时间（秒）
    max_age_seconds: u64,
    /// 自动清理的间隔（秒）
    cleanup_interval_seconds: u64,
    /// 上次清理时间
    last_cleanup: Instant,
}

impl Default for SignatureCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SignatureCache {
    /// 创建新的签名缓存（默认配置）
    pub fn new() -> Self {
        Self {
            used_signatures: HashMap::new(),
            max_age_seconds: 3600,  // 默认1小时
            cleanup_interval_seconds: 300,  // 每5分钟自动清理一次
            last_cleanup: Instant::now(),
        }
    }

    /// 创建指定过期时间的缓存
    pub fn with_max_age(max_age_seconds: u64) -> Self {
        Self {
            used_signatures: HashMap::new(),
            max_age_seconds,
            cleanup_interval_seconds: 300,
            last_cleanup: Instant::now(),
        }
    }

    /// 创建严格模式缓存（短过期时间）
    pub fn strict() -> Self {
        Self {
            used_signatures: HashMap::new(),
            max_age_seconds: 300,  // 5分钟
            cleanup_interval_seconds: 60,  // 每分钟清理
            last_cleanup: Instant::now(),
        }
    }

    /// 检查签名是否已使用过
    pub fn is_used(&self, signature_hash: &str) -> bool {
        self.used_signatures.contains_key(signature_hash)
    }

    /// 标记签名为已使用
    pub fn mark_used(&mut self, signature_hash: String) {
        let entry = CacheEntry {
            used_at: Instant::now(),
        };
        self.used_signatures.insert(signature_hash, entry);
    }

    /// 清除缓存
    pub fn clear(&mut self) {
        self.used_signatures.clear();
        self.last_cleanup = Instant::now();
    }

    /// 获取缓存大小
    pub fn size(&self) -> usize {
        self.used_signatures.len()
    }

    /// 自动清理过期条目
    pub fn cleanup_expired(&mut self) -> usize {
        let now = Instant::now();

        // 检查是否需要清理
        if now.duration_since(self.last_cleanup) < Duration::from_secs(self.cleanup_interval_seconds) {
            return 0;
        }

        let before = self.used_signatures.len();
        let max_age = Duration::from_secs(self.max_age_seconds);

        self.used_signatures.retain(|_, entry| {
            now.duration_since(entry.used_at) < max_age
        });

        self.last_cleanup = now;
        before - self.used_signatures.len()
    }

    /// 设置清理间隔
    pub fn set_cleanup_interval(&mut self, interval_seconds: u64) {
        self.cleanup_interval_seconds = interval_seconds;
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> CacheStats {
        let now = Instant::now();
        let max_age = Duration::from_secs(self.max_age_seconds);

        let total = self.used_signatures.len();
        let mut expired = 0;
        let mut valid = 0;

        for entry in self.used_signatures.values() {
            if now.duration_since(entry.used_at) < max_age {
                valid += 1;
            } else {
                expired += 1;
            }
        }

        CacheStats {
            total,
            valid,
            expired,
            max_age_seconds: self.max_age_seconds,
            cleanup_interval_seconds: self.cleanup_interval_seconds,
        }
    }
}

/// 缓存统计信息
pub struct CacheStats {
    pub total: usize,
    pub valid: usize,
    pub expired: usize,
    pub max_age_seconds: u64,
    pub cleanup_interval_seconds: u64,
}

/// 线程安全的签名缓存包装器
#[derive(Clone, Default)]
pub struct ThreadSafeSignatureCache {
    cache: Arc<Mutex<SignatureCache>>,
}

impl ThreadSafeSignatureCache {
    /// 创建新的线程安全缓存
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(SignatureCache::new())),
        }
    }

    /// 创建指定过期时间的线程安全缓存
    pub fn with_max_age(max_age_seconds: u64) -> Self {
        Self {
            cache: Arc::new(Mutex::new(SignatureCache::with_max_age(max_age_seconds))),
        }
    }

    /// 创建严格模式线程安全缓存
    pub fn strict() -> Self {
        Self {
            cache: Arc::new(Mutex::new(SignatureCache::strict())),
        }
    }

    /// 检查签名是否已使用过（自动清理过期条目）
    pub fn is_used(&self, signature_hash: &str) -> bool {
        let mut cache = self.cache.lock().unwrap();
        cache.cleanup_expired();
        cache.is_used(signature_hash)
    }

    /// 标记签名为已使用（自动清理过期条目）
    pub fn mark_used(&self, signature_hash: String) {
        let mut cache = self.cache.lock().unwrap();
        cache.cleanup_expired();
        cache.mark_used(signature_hash);
    }

    /// 清除缓存
    pub fn clear(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
    }

    /// 获取缓存大小
    pub fn size(&self) -> usize {
        let cache = self.cache.lock().unwrap();
        cache.size()
    }

    /// 手动触发过期条目清理
    pub fn cleanup_expired(&self) -> usize {
        let mut cache = self.cache.lock().unwrap();
        cache.cleanup_expired()
    }

    /// 获取缓存统计信息
    pub fn get_stats(&self) -> CacheStats {
        let cache = self.cache.lock().unwrap();
        cache.get_stats()
    }

    /// 设置清理间隔
    pub fn set_cleanup_interval(&self, interval_seconds: u64) {
        let mut cache = self.cache.lock().unwrap();
        cache.set_cleanup_interval(interval_seconds);
    }
}

/// 签名验证器
pub struct SignatureVerifier {
    /// 签名缓存（用于重放攻击防护）
    cache: Option<ThreadSafeSignatureCache>,
    /// 是否启用重放防护
    enable_replay_protection: bool,
    /// 验证配置
    timestamp_config: TimestampConfig,
}

impl Default for SignatureVerifier {
    fn default() -> Self {
        Self::with_replay_protection()  // 默认启用重放防护
    }
}

impl SignatureVerifier {
    /// 创建签名验证器（不启用重放防护）
    ///
    /// # 注意
    /// 此方法创建的验证器不提供重放攻击防护
    /// 建议使用 `with_replay_protection()` 或 `default()`
    pub fn new() -> Self {
        Self {
            cache: None,
            enable_replay_protection: false,
            timestamp_config: TimestampConfig::default(),
        }
    }

    /// 创建启用重放防护的签名验证器（推荐）
    pub fn with_replay_protection() -> Self {
        Self {
            cache: Some(ThreadSafeSignatureCache::new()),
            enable_replay_protection: true,
            timestamp_config: TimestampConfig::default(),
        }
    }

    /// 创建启用重放防护且指定过期时间的签名验证器
    pub fn with_replay_protection_and_max_age(max_age_seconds: u64) -> Self {
        Self {
            cache: Some(ThreadSafeSignatureCache::with_max_age(max_age_seconds)),
            enable_replay_protection: true,
            timestamp_config: TimestampConfig::default(),
        }
    }

    /// 创建严格模式签名验证器（短过期时间）
    pub fn with_strict_mode() -> Self {
        Self {
            cache: Some(ThreadSafeSignatureCache::strict()),
            enable_replay_protection: true,
            timestamp_config: TimestampConfig::strict(),
        }
    }

    /// 创建指定缓存的签名验证器
    pub fn with_cache(cache: ThreadSafeSignatureCache) -> Self {
        Self {
            cache: Some(cache),
            enable_replay_protection: true,
            timestamp_config: TimestampConfig::default(),
        }
    }

    /// 创建指定配置的签名验证器
    pub fn with_config(config: TimestampConfig) -> Self {
        Self {
            cache: Some(ThreadSafeSignatureCache::new()),
            enable_replay_protection: true,
            timestamp_config: config,
        }
    }

    /// 创建指定缓存和配置的签名验证器
    pub fn with_cache_and_config(cache: ThreadSafeSignatureCache, config: TimestampConfig) -> Self {
        Self {
            cache: Some(cache),
            enable_replay_protection: true,
            timestamp_config: config,
        }
    }

    /// 更新验证配置
    pub fn set_config(&mut self, config: TimestampConfig) {
        self.timestamp_config = config;
    }

    /// 获取当前验证配置
    pub fn get_config(&self) -> &TimestampConfig {
        &self.timestamp_config
    }

    /// 获取缓存引用
    pub fn cache(&self) -> Option<&ThreadSafeSignatureCache> {
        self.cache.as_ref()
    }

    /// 检查是否启用重放防护
    pub fn is_replay_protection_enabled(&self) -> bool {
        self.enable_replay_protection
    }

    /// 清除签名缓存
    pub fn clear_cache(&self) {
        if let Some(cache) = &self.cache {
            cache.clear();
        }
    }

    /// 计算签名哈希（用于重放检测）
    fn calculate_signature_hash(signature: &PluginSignature) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(&signature.signature);
        hasher.update(&signature.public_key);
        hasher.update(signature.algorithm.to_string());
        hasher.update(signature.signed_at.to_le_bytes());
        hex::encode(hasher.finalize())
    }

    /// 验证插件签名（使用验证器配置）
    ///
    /// # 参数
    /// * `metadata` - 插件元数据
    /// * `trust_unsigned` - 是否信任未签名的插件
    ///
    /// # 返回
    /// 验证成功返回 Ok(())，失败返回错误
    pub fn verify_metadata(
        &self,
        metadata: &PluginMetadata,
        trust_unsigned: bool,
    ) -> Result<(), SignatureError> {
        self.verify_metadata_with_config(metadata, trust_unsigned, &self.timestamp_config)
    }

    /// 验证插件签名（使用自定义配置）
    ///
    /// # 参数
    /// * `metadata` - 插件元数据
    /// * `trust_unsigned` - 是否信任未签名的插件
    /// * `config` - 时间戳验证配置
    ///
    /// # 返回
    /// 验证成功返回 Ok(())，失败返回错误
    pub fn verify_metadata_with_config(
        &self,
        metadata: &PluginMetadata,
        trust_unsigned: bool,
        config: &TimestampConfig,
    ) -> Result<(), SignatureError> {
        match &metadata.signature {
            None => {
                if trust_unsigned {
                    Ok(())
                } else {
                    Err(SignatureError::Unsigned)
                }
            }
            Some(sig) => {
                // 创建不包含签名字段的元数据副本，用于验证
                let mut metadata_without_signature = metadata.clone();
                metadata_without_signature.signature = None;
                let metadata_json = serde_json::to_string(&metadata_without_signature)?;
                self.verify_with_config(&metadata_json, sig, config)
            }
        }
    }

    /// 验证签名（使用验证器配置）
    ///
    /// # 参数
    /// * `metadata_json` - 元数据的 JSON 字符串
    /// * `signature` - 签名信息
    ///
    /// # 返回
    /// 验证成功返回 Ok(())，失败返回错误
    pub fn verify(
        &self,
        metadata_json: &str,
        signature: &PluginSignature,
    ) -> Result<(), SignatureError> {
        self.verify_with_config(metadata_json, signature, &self.timestamp_config)
    }

    /// 验证签名（使用自定义配置）
    ///
    /// # 参数
    /// * `metadata_json` - 元数据的 JSON 字符串（用于签名验证）
    /// * `signature` - 签名信息
    /// * `config` - 时间戳验证配置
    ///
    /// # 返回
    /// 验证成功返回 Ok(())，失败返回错误
    ///
    /// # 注意
    /// 此方法假设签名存在且已通过 `verify_metadata_with_config` 的预检查。
    /// `trust_unsigned` 参数在此处不适用，因为调用此方法时签名必然存在。
    pub fn verify_with_config(
        &self,
        metadata_json: &str,
        signature: &PluginSignature,
        config: &TimestampConfig,
    ) -> Result<(), SignatureError> {
        // 检查时间戳
        Self::verify_timestamp(signature, config)?;

        // 检查重放攻击（如果启用）
        if self.enable_replay_protection && let Some(cache) = &self.cache {
            let signature_hash = Self::calculate_signature_hash(signature);
            if cache.is_used(&signature_hash) {
                return Err(SignatureError::ReplayAttackDetected { signature_hash });
            }
            // 标记为已使用
            cache.mark_used(signature_hash);
        }

        // 根据算法验证
        match signature.algorithm {
            SignatureAlgorithm::Ed25519 => {
                Self::verify_ed25519(metadata_json, signature)
            }
        }
    }

    /// 验证时间戳（防止重放攻击）
    ///
    /// # 验证规则
    /// 1. 检查时钟偏差（签名时间与当前时间的绝对差值）
    /// 2. 签名时间不能是未来（超出时钟容忍度）
    /// 3. 签名不能超过有效期
    /// 4. 可选：要求签名在合理时间范围内
    fn verify_timestamp(sig: &PluginSignature, config: &TimestampConfig) -> Result<(), SignatureError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // 1. 检查时钟偏差（绝对差值，防止时钟回滚或篡改）
        let skew = sig.signed_at.abs_diff(now);

        if skew > config.max_clock_skew_seconds {
            return Err(SignatureError::ClockSkewTooLarge {
                signed_at: sig.signed_at,
                now,
                skew,
                max_skew: config.max_clock_skew_seconds,
            });
        }

        // 2. 检查未来时间（带时钟容忍度）
        if sig.signed_at > now + config.clock_tolerance_seconds {
            return Err(SignatureError::InvalidTimestamp {
                signed_at: sig.signed_at,
                now,
            });
        }

        // 3. 检查是否过于陈旧（可选）
        if config.require_recent_signature {
            let one_year_ago = now.saturating_sub(365 * 24 * 60 * 60);
            if sig.signed_at < one_year_ago {
                return Err(SignatureError::Expired {
                    signed_at: sig.signed_at,
                    max_age: 365,
                });
            }
        }

        // 4. 检查有效期
        let age = now.saturating_sub(sig.signed_at);
        if age > config.max_age_seconds {
            let max_age_days = config.max_age_seconds / (24 * 60 * 60);
            return Err(SignatureError::Expired {
                signed_at: sig.signed_at,
                max_age: max_age_days,
            });
        }

        Ok(())
    }

    /// Ed25519 签名验证
    fn verify_ed25519(
        metadata_json: &str,
        sig: &PluginSignature,
    ) -> Result<(), SignatureError> {
        // 解码十六进制
        let public_key_bytes = hex::decode(&sig.public_key)?;
        let signature_bytes = hex::decode(&sig.signature)?;

        // 创建公钥
        let public_key = UnparsedPublicKey::new(&ED25519, public_key_bytes);

        // 验证签名
        public_key
            .verify(metadata_json.as_bytes(), &signature_bytes)
            .map_err(|e| SignatureError::VerificationFailed {
                reason: e.to_string(),
                algorithm: sig.algorithm.clone(),
                public_key: sig.public_key.clone(),
            })
    }

    /// 生成签名（开发者工具）
    ///
    /// # 参数
    /// * `metadata` - 插件元数据
    /// * `private_key_hex` - 私钥（十六进制编码）
    /// * `algorithm` - 签名算法
    ///
    /// # 返回
    /// 签名信息
    pub fn sign_metadata(
        metadata: &PluginMetadata,
        private_key_hex: &str,
        algorithm: SignatureAlgorithm,
    ) -> Result<PluginSignature, SignatureError> {
        // 创建不包含签名字段的元数据副本，用于签名
        let mut metadata_without_signature = metadata.clone();
        metadata_without_signature.signature = None;
        let metadata_json = serde_json::to_string(&metadata_without_signature)?;
        Self::sign(&metadata_json, private_key_hex, algorithm)
    }

    /// 生成签名
    pub fn sign(
        metadata_json: &str,
        private_key_hex: &str,
        algorithm: SignatureAlgorithm,
    ) -> Result<PluginSignature, SignatureError> {
        let signed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        match algorithm {
            SignatureAlgorithm::Ed25519 => {
                Self::sign_ed25519(metadata_json, private_key_hex, signed_at)
            }
        }
    }

    /// Ed25519 签名生成
    fn sign_ed25519(
        metadata_json: &str,
        private_key_hex: &str,
        signed_at: u64,
    ) -> Result<PluginSignature, SignatureError> {
        // 解码私钥（32字节种子）
        let private_key_bytes = hex::decode(private_key_hex)?;

        if private_key_bytes.len() != 32 {
            return Err(SignatureError::SigningFailed(
                "Ed25519 私钥种子必须是 32 字节".to_string()
            ));
        }

        // 生成密钥对
        let key_pair = Ed25519KeyPair::from_seed_unchecked(&private_key_bytes)
            .map_err(|e| SignatureError::SigningFailed(e.to_string()))?;

        // 签名
        let signature = key_pair.sign(metadata_json.as_bytes());
        let signature_hex = hex::encode(signature.as_ref());

        // 获取公钥（使用 trait 方法）
        use ring::signature::KeyPair;
        let public_key_hex = hex::encode(key_pair.public_key().as_ref());

        Ok(PluginSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: public_key_hex,
            signature: signature_hex,
            signed_at,
        })
    }

    /// 生成密钥对（用于测试或开发者首次使用）
    ///
    /// # 返回
    /// (私钥, 公钥) 的十六进制字符串
    pub fn generate_key_pair() -> Result<(String, String), SignatureError> {
        use ring::rand::{SecureRandom, SystemRandom};
        use ring::signature::KeyPair;

        let rng = SystemRandom::new();
        let mut seed = [0u8; 32];
        rng.fill(&mut seed)
            .map_err(|e| SignatureError::SigningFailed(e.to_string()))?;

        let key_pair = Ed25519KeyPair::from_seed_unchecked(&seed)
            .map_err(|e| SignatureError::SigningFailed(e.to_string()))?;

        let private_key_hex = hex::encode(seed);
        let public_key_hex = hex::encode(key_pair.public_key().as_ref());

        Ok((private_key_hex, public_key_hex))
    }

    /// 计算公钥指纹（用于显示）
    pub fn fingerprint(public_key_hex: &str) -> String {
        let bytes = match hex::decode(public_key_hex) {
            Ok(b) => b,
            Err(_) => return "INVALID".to_string(),
        };

        // 取前8字节作为指纹
        hex::encode(&bytes[..8.min(bytes.len())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::types::{PluginType, Platform};

    #[test]
    fn test_generate_key_pair() {
        let result = SignatureVerifier::generate_key_pair();
        assert!(result.is_ok());

        let (private_key, public_key) = result.unwrap();
        assert_eq!(private_key.len(), 64);  // 32字节 = 64 hex chars (种子)
        assert_eq!(public_key.len(), 64);   // 32字节 = 64 hex chars
    }

    #[test]
    fn test_sign_and_verify() {
        // 生成密钥对
        let (private_key, public_key) = SignatureVerifier::generate_key_pair().unwrap();

        // 创建测试元数据
        let metadata = PluginMetadata {
            id: "test-plugin".to_string(),
            name: "Test Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: Some("A test plugin".to_string()),
            author: Some("Test Author".to_string()),
            plugin_type: PluginType::DynamicLibrary,
            hooks: vec![],
            extensions: vec![],
            config_schema: None,
            enabled: true,
            dependencies: vec![],
            platforms: vec![Platform::current()],
            envcli_version: None,
            signature: None, // 签名前为空
        };

        // 生成签名
        let metadata_json = serde_json::to_string(&metadata).unwrap();
        let signature = SignatureVerifier::sign(&metadata_json, &private_key, SignatureAlgorithm::Ed25519).unwrap();

        // 验证签名（使用实例方法）
        let verifier = SignatureVerifier::new();
        let verify_result = verifier.verify(&metadata_json, &signature);
        assert!(verify_result.is_ok());

        // 验证公钥匹配
        assert_eq!(signature.public_key, public_key);
    }

    #[test]
    fn test_verify_timestamp() {
        let sig = PluginSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: "abcd".to_string(),
            signature: "1234".to_string(),
            signed_at: 0, // 1970年，已过期
        };

        let config = TimestampConfig::default();
        let result = SignatureVerifier::verify_timestamp(&sig, &config);
        assert!(result.is_err());
        // 由于时钟偏差检查（默认24小时），时间戳0会先触发 ClockSkewTooLarge
        let err = result.unwrap_err();
        assert!(
            matches!(err, SignatureError::ClockSkewTooLarge { .. }) ||
            matches!(err, SignatureError::Expired { .. }),
            "Expected ClockSkewTooLarge or Expired, got: {:?}", err
        );
    }

    #[test]
    fn test_clock_skew_detection() {
        // 测试时钟偏差检测
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // 签名时间在未来2小时（超过1小时的最大偏差）
        let sig_future = PluginSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: "abcd".to_string(),
            signature: "1234".to_string(),
            signed_at: now + 7200, // 2小时
        };

        // 严格配置：最大时钟偏差1小时
        let config = TimestampConfig::strict().with_max_clock_skew(3600);
        let result = SignatureVerifier::verify_timestamp(&sig_future, &config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, SignatureError::ClockSkewTooLarge { .. }));

        // 测试过去时间（时钟回滚）
        let sig_past = PluginSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: "abcd".to_string(),
            signature: "1234".to_string(),
            signed_at: now - 7200, // 2小时前
        };

        let result = SignatureVerifier::verify_timestamp(&sig_past, &config);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, SignatureError::ClockSkewTooLarge { .. }));
    }

    #[test]
    fn test_strict_config() {
        // 测试严格模式配置
        let strict_config = TimestampConfig::strict();
        assert_eq!(strict_config.max_age_seconds, 7 * 24 * 60 * 60); // 7天
        assert_eq!(strict_config.max_clock_skew_seconds, 3600); // 1小时
        assert!(strict_config.require_recent_signature);
    }

    #[test]
    fn test_fingerprint() {
        // 测试正常情况 - 32字节公钥（64个十六进制字符），取前8字节
        let fp = SignatureVerifier::fingerprint("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef");
        assert_eq!(fp, "0123456789abcdef"); // 前8字节 = 16个十六进制字符

        // 测试短输入（8字节）- 返回全部
        let fp = SignatureVerifier::fingerprint("0123456789abcdef");
        assert_eq!(fp, "0123456789abcdef");

        // 测试无效输入
        let fp = SignatureVerifier::fingerprint("invalid");
        assert_eq!(fp, "INVALID");
    }

    #[test]
    fn test_replay_protection() {
        // 生成密钥对
        let (private_key, _) = SignatureVerifier::generate_key_pair().unwrap();

        // 创建测试元数据
        let metadata = PluginMetadata {
            id: "test-plugin".to_string(),
            name: "Test Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: Some("A test plugin".to_string()),
            author: Some("Test Author".to_string()),
            plugin_type: PluginType::DynamicLibrary,
            hooks: vec![],
            extensions: vec![],
            config_schema: None,
            enabled: true,
            dependencies: vec![],
            platforms: vec![Platform::current()],
            envcli_version: None,
            signature: None,
        };

        // 生成签名
        let metadata_json = serde_json::to_string(&metadata).unwrap();
        let signature = SignatureVerifier::sign(&metadata_json, &private_key, SignatureAlgorithm::Ed25519).unwrap();

        // 创建启用重放防护的验证器
        let verifier = SignatureVerifier::with_replay_protection();

        // 第一次验证应该成功
        let result = verifier.verify(&metadata_json, &signature);
        assert!(result.is_ok(), "第一次验证应该通过");

        // 第二次验证相同的签名应该失败（重放攻击）
        let result = verifier.verify(&metadata_json, &signature);
        assert!(result.is_err(), "重放签名应该被拒绝");
        let err = result.unwrap_err();
        assert!(matches!(err, SignatureError::ReplayAttackDetected { .. }));

        // 清除缓存后应该可以再次验证
        verifier.clear_cache();
        let result = verifier.verify(&metadata_json, &signature);
        assert!(result.is_ok(), "清除缓存后应该可以再次验证");
    }

    #[test]
    fn test_no_replay_protection() {
        // 生成密钥对
        let (private_key, _) = SignatureVerifier::generate_key_pair().unwrap();

        // 创建测试元数据
        let metadata = PluginMetadata {
            id: "test-plugin".to_string(),
            name: "Test Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: Some("A test plugin".to_string()),
            author: Some("Test Author".to_string()),
            plugin_type: PluginType::DynamicLibrary,
            hooks: vec![],
            extensions: vec![],
            config_schema: None,
            enabled: true,
            dependencies: vec![],
            platforms: vec![Platform::current()],
            envcli_version: None,
            signature: None,
        };

        // 生成签名
        let metadata_json = serde_json::to_string(&metadata).unwrap();
        let signature = SignatureVerifier::sign(&metadata_json, &private_key, SignatureAlgorithm::Ed25519).unwrap();

        // 创建禁用重放防护的验证器
        let verifier = SignatureVerifier::new();

        // 多次验证相同的签名都应该成功
        for _ in 0..5 {
            let result = verifier.verify(&metadata_json, &signature);
            assert!(result.is_ok(), "禁用重放防护时应该允许多次验证");
        }

        // 验证缓存大小应该为0
        assert_eq!(verifier.cache().map(|c| c.size()).unwrap_or(0), 0);
    }

    #[test]
    fn test_signature_cache() {
        let cache = ThreadSafeSignatureCache::new();

        // 初始状态
        assert_eq!(cache.size(), 0);
        assert!(!cache.is_used("test_hash"));

        // 标记为已使用
        cache.mark_used("test_hash".to_string());
        assert_eq!(cache.size(), 1);
        assert!(cache.is_used("test_hash"));

        // 重复标记
        cache.mark_used("test_hash".to_string());
        assert_eq!(cache.size(), 1); // 大小不变

        // 清除缓存
        cache.clear();
        assert_eq!(cache.size(), 0);
        assert!(!cache.is_used("test_hash"));
    }

    #[test]
    fn test_calculate_signature_hash() {
        let sig1 = PluginSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: "abcd".to_string(),
            signature: "1234".to_string(),
            signed_at: 1000,
        };

        let sig2 = PluginSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: "abcd".to_string(),
            signature: "1234".to_string(),
            signed_at: 1000,
        };

        let sig3 = PluginSignature {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: "abcd".to_string(),
            signature: "1235".to_string(), // 不同签名
            signed_at: 1000,
        };

        let hash1 = SignatureVerifier::calculate_signature_hash(&sig1);
        let hash2 = SignatureVerifier::calculate_signature_hash(&sig2);
        let hash3 = SignatureVerifier::calculate_signature_hash(&sig3);

        // 相同签名应该产生相同哈希
        assert_eq!(hash1, hash2);

        // 不同签名应该产生不同哈希
        assert_ne!(hash1, hash3);

        // 哈希应该是64字符的十六进制字符串（SHA-256）
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn test_with_cache() {
        let cache = ThreadSafeSignatureCache::new();
        let verifier = SignatureVerifier::with_cache(cache.clone());

        assert!(verifier.is_replay_protection_enabled());
        assert!(verifier.cache().is_some());

        // 缓存应该共享
        cache.mark_used("test".to_string());
        assert!(verifier.cache().unwrap().is_used("test"));
    }

    #[test]
    fn test_cache_expiration() {
        use std::thread;
        use std::time::Duration;

        // 创建短过期时间的缓存（1秒），设置为立即可清理
        let cache = ThreadSafeSignatureCache::with_max_age(1);
        cache.set_cleanup_interval(0); // 设置清理间隔为0，立即可清理

        // 标记签名
        cache.mark_used("test_hash".to_string());
        assert_eq!(cache.size(), 1);
        assert!(cache.is_used("test_hash"));

        // 等待过期（2秒，超过1秒的过期时间）
        thread::sleep(Duration::from_secs(2));

        // 手动清理
        let cleaned = cache.cleanup_expired();

        // 验证清理结果
        assert!(cleaned > 0, "应该清理至少1个过期条目，实际清理了: {}", cleaned);
        assert_eq!(cache.size(), 0, "清理后缓存应该为空");
        assert!(!cache.is_used("test_hash"), "过期的签名应该不再被标记为已使用");
    }

    #[test]
    fn test_strict_mode_cache() {
        let _cache = ThreadSafeSignatureCache::strict();
        let verifier = SignatureVerifier::with_strict_mode();

        assert!(verifier.is_replay_protection_enabled());
        assert!(verifier.cache().is_some());
    }

    #[test]
    fn test_cache_stats() {
        let cache = ThreadSafeSignatureCache::new();

        // 添加一些条目
        cache.mark_used("hash1".to_string());
        cache.mark_used("hash2".to_string());
        cache.mark_used("hash3".to_string());

        let stats = cache.get_stats();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.valid, 3);
        assert_eq!(stats.expired, 0);
        assert!(stats.max_age_seconds > 0);
    }

    #[test]
    fn test_default_verifier_uses_replay_protection() {
        // 测试默认构造函数使用重放防护
        let verifier = SignatureVerifier::default();
        assert!(verifier.is_replay_protection_enabled());
        assert!(verifier.cache().is_some());
    }

    #[test]
    fn test_with_replay_protection_and_max_age() {
        let verifier = SignatureVerifier::with_replay_protection_and_max_age(600);
        assert!(verifier.is_replay_protection_enabled());
        assert!(verifier.cache().is_some());

        // 验证缓存配置
        let stats = verifier.cache().unwrap().get_stats();
        assert_eq!(stats.max_age_seconds, 600);
    }
}
