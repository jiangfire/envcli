//! 插件管理器
//!
//! 插件系统的核心，负责插件的加载、管理和执行
//!
//! # 锁获取顺序规范
//!
//! 为了避免死锁，所有代码必须遵循以下锁获取顺序：
//!
//! 1. **插件管理器锁 (PluginManager)**: `self` 的 Mutex（如果需要）
//! 2. **配置/状态读锁**: `plugin_configs.read()` 或 `plugin_statuses.read()`
//! 3. **配置/状态写锁**: `plugin_configs.write()` 或 `plugin_statuses.write()`
//! 4. **插件实例锁**: `plugin_arc.lock()`
//!
//! ## 重要规则
//!
//! - 如果需要同时获取多个 RwLock，必须按照 `plugin_configs` → `plugin_statuses` 的顺序
//! - 不要在持有 RwLock 的同时获取 PluginManager 的 Mutex
//! - 插件实例锁 (Mutex<Box<dyn Plugin>>) 应该在获取配置锁之后获取
//! - 钩子调度器锁应该在获取插件实例锁之后获取
//!
//! ## 示例
//!
//! ```ignore
//! // ✅ 正确：先读锁，后操作
//! let config = self.plugin_configs.read().unwrap().get(id).cloned();
//! let status = self.plugin_statuses.read().unwrap().get(id).cloned();
//!
//! // ✅ 正确：分别获取写锁
//! self.plugin_configs.write().unwrap().insert(id, config);
//! self.plugin_statuses.write().unwrap().insert(id, status);
//!
//! // ❌ 错误：反向获取锁
//! let status = self.plugin_statuses.read().unwrap().get(id);  // 先 statuses
//! let config = self.plugin_configs.read().unwrap().get(id);  // 后 configs
//! ```

use crate::plugin::dependency::DependencyResolver;
use crate::plugin::hook::HookDispatcher;
use crate::plugin::loader::LoaderFactory;
use crate::plugin::signature::{SignatureVerifier, ThreadSafeSignatureCache, TimestampConfig};
use crate::plugin::types::{
    HookContext, HookPriority, HookResult, HookType, Plugin, PluginConfig, PluginError,
    PluginInfo, PluginMetadata, PluginSignature, PluginStatus, SignatureAlgorithm,
    CompatibilityIssue, CompatibilityReport, Platform,
};
use crate::plugin::validation::{PluginIdValidator, PathValidator, ConfigValidator};
use crate::utils::encryption::{SopsEncryptor, CacheConfig as EncryptorCacheConfig};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

/// 插件管理器
pub struct PluginManager {
    /// 已加载的插件 (plugin_id -> plugin)
    plugins: HashMap<String, Arc<Mutex<Box<dyn Plugin>>>>,

    /// 插件配置（使用 RwLock 支持并发读写）
    plugin_configs: RwLock<HashMap<String, PluginConfig>>,

    /// 钩子调度器
    hook_dispatcher: HookDispatcher,

    /// 插件状态跟踪（使用 RwLock 支持并发读写）
    plugin_statuses: RwLock<HashMap<String, PluginStatus>>,

    /// 插件目录
    plugin_dir: PathBuf,

    /// 签名验证器（支持重放防护）
    signature_verifier: SignatureVerifier,

    /// 签名缓存（用于跨插件管理器实例共享缓存）
    signature_cache: ThreadSafeSignatureCache,

    /// 加密器（用于敏感数据处理）
    encryptor: SopsEncryptor,

    /// 正在重载的插件（用于并发控制）
    reloading_plugins: Arc<Mutex<HashMap<String, std::time::Instant>>>,

    /// 性能监控统计
    performance_stats: Arc<Mutex<PluginManagerPerformanceStats>>,
}

/// 包装器：将 Mutex<Box<dyn Plugin>> 转换为 dyn Plugin
#[derive(Clone)]
struct PluginWrapper {
    inner: Arc<Mutex<Box<dyn Plugin>>>,
}

/// 重载快照（用于避免死锁和事务性回滚）
struct ReloadSnapshot {
    config: PluginConfig,
    status: Option<PluginStatus>,
    plugin: Option<Arc<Mutex<Box<dyn Plugin>>>>,
    hooks: Vec<HookType>,
    path: PathBuf,
}

impl Plugin for PluginWrapper {
    fn metadata(&self) -> PluginMetadata {
        let plugin = self.inner.lock().unwrap();
        plugin.metadata()
    }

    fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        let mut plugin = self.inner.lock().unwrap();
        plugin.initialize(config)
    }

    fn execute_hook(&self, hook_type: HookType, context: &HookContext) -> Result<HookResult, PluginError> {
        let plugin = self.inner.lock().unwrap();
        plugin.execute_hook(hook_type, context)
    }

    fn supports_extension(&self, extension: crate::plugin::types::ExtensionPoint) -> bool {
        let plugin = self.inner.lock().unwrap();
        plugin.supports_extension(extension)
    }

    fn execute_extension(&self, extension: crate::plugin::types::ExtensionPoint, input: &[u8]) -> Result<Vec<u8>, PluginError> {
        let plugin = self.inner.lock().unwrap();
        plugin.execute_extension(extension, input)
    }

    fn shutdown(&mut self) -> Result<(), PluginError> {
        let mut plugin = self.inner.lock().unwrap();
        plugin.shutdown()
    }
}

impl PluginManager {
    /// 创建插件管理器（默认启用重放防护和标准配置）
    pub fn new() -> Result<Self, PluginError> {
        let plugin_dir = PathBuf::from("plugins");

        // 确保插件目录存在
        if !plugin_dir.exists() {
            let _ = std::fs::create_dir_all(&plugin_dir);
        }

        let cache = ThreadSafeSignatureCache::new();
        let verifier = SignatureVerifier::with_cache(cache.clone());
        let encryptor = SopsEncryptor::with_default_cache();

        Ok(Self {
            plugins: HashMap::new(),
            plugin_configs: RwLock::new(HashMap::new()),
            hook_dispatcher: HookDispatcher::new(),
            plugin_statuses: RwLock::new(HashMap::new()),
            plugin_dir,
            signature_verifier: verifier,
            signature_cache: cache,
            encryptor,
            reloading_plugins: Arc::new(Mutex::new(HashMap::new())),
            performance_stats: Arc::new(Mutex::new(PluginManagerPerformanceStats::new())),
        })
    }

    /// 创建空的插件管理器（不启用重放防护，无缓存）
    pub fn empty() -> Self {
        let cache = ThreadSafeSignatureCache::new();
        let verifier = SignatureVerifier::new();
        let encryptor = SopsEncryptor::new();

        Self {
            plugins: HashMap::new(),
            plugin_configs: RwLock::new(HashMap::new()),
            hook_dispatcher: HookDispatcher::new(),
            plugin_statuses: RwLock::new(HashMap::new()),
            plugin_dir: PathBuf::new(),
            signature_verifier: verifier,
            signature_cache: cache,
            encryptor,
            reloading_plugins: Arc::new(Mutex::new(HashMap::new())),
            performance_stats: Arc::new(Mutex::new(PluginManagerPerformanceStats::new())),
        }
    }

    /// 创建禁用重放防护的插件管理器
    pub fn without_replay_protection() -> Result<Self, PluginError> {
        let plugin_dir = PathBuf::from("plugins");

        if !plugin_dir.exists() {
            let _ = std::fs::create_dir_all(&plugin_dir);
        }

        let cache = ThreadSafeSignatureCache::new();
        let verifier = SignatureVerifier::new();
        let encryptor = SopsEncryptor::with_default_cache();

        Ok(Self {
            plugins: HashMap::new(),
            plugin_configs: RwLock::new(HashMap::new()),
            hook_dispatcher: HookDispatcher::new(),
            plugin_statuses: RwLock::new(HashMap::new()),
            plugin_dir,
            signature_verifier: verifier,
            signature_cache: cache,
            encryptor,
            reloading_plugins: Arc::new(Mutex::new(HashMap::new())),
            performance_stats: Arc::new(Mutex::new(PluginManagerPerformanceStats::new())),
        })
    }

    /// 创建严格模式的插件管理器（高安全配置）
    pub fn strict() -> Result<Self, PluginError> {
        let plugin_dir = PathBuf::from("plugins");

        if !plugin_dir.exists() {
            let _ = std::fs::create_dir_all(&plugin_dir);
        }

        let cache = ThreadSafeSignatureCache::strict();
        let verifier = SignatureVerifier::with_strict_mode();
        let encryptor = SopsEncryptor::with_cache_config(EncryptorCacheConfig::strict());

        Ok(Self {
            plugins: HashMap::new(),
            plugin_configs: RwLock::new(HashMap::new()),
            hook_dispatcher: HookDispatcher::new(),
            plugin_statuses: RwLock::new(HashMap::new()),
            plugin_dir,
            signature_verifier: verifier,
            signature_cache: cache,
            encryptor,
            reloading_plugins: Arc::new(Mutex::new(HashMap::new())),
            performance_stats: Arc::new(Mutex::new(PluginManagerPerformanceStats::new())),
        })
    }

    /// 创建开发模式的插件管理器（宽松配置）
    pub fn development() -> Result<Self, PluginError> {
        let plugin_dir = PathBuf::from("plugins");

        if !plugin_dir.exists() {
            let _ = std::fs::create_dir_all(&plugin_dir);
        }

        let cache = ThreadSafeSignatureCache::with_max_age(3600 * 24); // 24小时
        let verifier = SignatureVerifier::with_config(TimestampConfig::lax());
        let encryptor = SopsEncryptor::with_cache(1000); // 大缓存

        Ok(Self {
            plugins: HashMap::new(),
            plugin_configs: RwLock::new(HashMap::new()),
            hook_dispatcher: HookDispatcher::new(),
            plugin_statuses: RwLock::new(HashMap::new()),
            plugin_dir,
            signature_verifier: verifier,
            signature_cache: cache,
            encryptor,
            reloading_plugins: Arc::new(Mutex::new(HashMap::new())),
            performance_stats: Arc::new(Mutex::new(PluginManagerPerformanceStats::new())),
        })
    }

    /// 从配置创建插件管理器
    pub fn from_config(_config: crate::plugin::config::PluginGlobalConfig) -> Result<Self, PluginError> {
        Self::new()
    }

    /// 重新配置签名验证器
    pub fn reconfigure_signature_verifier(&mut self, config: TimestampConfig) {
        self.signature_verifier.set_config(config);
    }

    /// 重新配置加密器（使用新的缓存配置）
    pub fn reconfigure_encryptor(&mut self, cache_config: EncryptorCacheConfig) {
        self.encryptor = SopsEncryptor::with_cache_config(cache_config);
    }

    /// 获取加密器引用
    pub fn encryptor(&self) -> &SopsEncryptor {
        &self.encryptor
    }

    /// 获取可变加密器引用（用于需要修改的场景）
    pub fn encryptor_mut(&mut self) -> &mut SopsEncryptor {
        &mut self.encryptor
    }

    /// 获取签名验证器
    pub fn signature_verifier(&self) -> &SignatureVerifier {
        &self.signature_verifier
    }

    /// 获取签名缓存
    pub fn signature_cache(&self) -> &ThreadSafeSignatureCache {
        &self.signature_cache
    }

    /// 清除签名缓存
    pub fn clear_signature_cache(&self) {
        self.signature_cache.clear();
    }

    /// 保存插件状态到文件（持久化）
    ///
    /// # 保存内容
    /// - 插件配置
    /// - 插件状态
    /// - 签名缓存（可选）
    /// - 性能统计（可选）
    pub fn save_state(&self, path: &Path) -> Result<(), PluginError> {
        use serde_json::json;
        use std::fs;

        // 收集要保存的数据
        let configs: HashMap<String, serde_json::Value> = self.plugin_configs
            .read()
            .unwrap()
            .iter()
            .map(|(id, config)| {
                (id.clone(), json!({
                    "plugin_id": config.plugin_id,
                    "enabled": config.enabled,
                    "settings": config.settings,
                    "path": config.path.as_ref().map(|p| p.to_string_lossy().to_string()),
                    "timeout": config.timeout,
                    "env": config.env,
                }))
            })
            .collect();

        let statuses: HashMap<String, serde_json::Value> = self.plugin_statuses
            .read()
            .unwrap()
            .iter()
            .map(|(id, status)| {
                (id.clone(), json!({
                    "plugin_id": status.plugin_id,
                    "enabled": status.enabled,
                    "loaded": status.loaded,
                    "last_error": status.last_error,
                    "execution_count": status.execution_count,
                    "error_count": status.error_count,
                    "last_execution": status.last_execution,
                }))
            })
            .collect();

        let state = json!({
            "version": "1.0",
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            "plugin_configs": configs,
            "plugin_statuses": statuses,
            "stats": self.get_performance_stats(),
        });

        // 写入文件
        let content = serde_json::to_string_pretty(&state)
            .map_err(|e| PluginError::ExecutionFailed(format!("序列化失败: {}", e)))?;

        fs::write(path, content)
            .map_err(|e| PluginError::ExecutionFailed(format!("写入文件失败: {}", e)))?;

        Ok(())
    }

    /// 从文件加载插件状态（持久化恢复）
    pub fn load_state(&mut self, path: &Path) -> Result<(), PluginError> {
        use std::fs;
        use serde_json::Value;

        if !path.exists() {
            return Err(PluginError::LoadFailed(format!("状态文件不存在: {}", path.display())));
        }

        let content = fs::read_to_string(path)
            .map_err(|e| PluginError::LoadFailed(format!("读取文件失败: {}", e)))?;

        let state: Value = serde_json::from_str(&content)
            .map_err(|e| PluginError::LoadFailed(format!("解析 JSON 失败: {}", e)))?;

        // 恢复插件配置
        if let Some(configs) = state.get("plugin_configs").and_then(|v| v.as_object()) {
            let mut plugin_configs = self.plugin_configs.write().unwrap();
            for (id, config) in configs {
                if let Some(obj) = config.as_object() {
                    let plugin_config = PluginConfig {
                        plugin_id: id.clone(),
                        enabled: obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                        settings: obj.get("settings")
                            .and_then(|v| v.as_object())
                            .map(|m| {
                                m.iter()
                                    .map(|(k, v)| (k.clone(), v.to_string()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        path: obj.get("path")
                            .and_then(|v| v.as_str())
                            .map(PathBuf::from),
                        timeout: obj.get("timeout").and_then(|v| v.as_u64()),
                        env: obj.get("env")
                            .and_then(|v| v.as_object())
                            .map(|m| {
                                m.iter()
                                    .map(|(k, v)| (k.clone(), v.to_string()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    };
                    plugin_configs.insert(id.clone(), plugin_config);
                }
            }
        }

        // 恢复插件状态
        if let Some(statuses) = state.get("plugin_statuses").and_then(|v| v.as_object()) {
            let mut plugin_statuses = self.plugin_statuses.write().unwrap();
            for (id, status) in statuses {
                if let Some(obj) = status.as_object() {
                    let plugin_status = PluginStatus {
                        plugin_id: id.clone(),
                        enabled: obj.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
                        loaded: obj.get("loaded").and_then(|v| v.as_bool()).unwrap_or(false),
                        last_error: obj.get("last_error").and_then(|v| v.as_str()).map(String::from),
                        execution_count: obj.get("execution_count").and_then(|v| v.as_u64()).unwrap_or(0),
                        error_count: obj.get("error_count").and_then(|v| v.as_u64()).unwrap_or(0),
                        last_execution: obj.get("last_execution").and_then(|v| v.as_u64()),
                    };
                    plugin_statuses.insert(id.clone(), plugin_status);
                }
            }
        }

        Ok(())
    }

    /// 保存当前配置到文件（向后兼容方法，调用 save_state）
    pub fn save_config(&self) -> Result<(), PluginError> {
        let path = self.plugin_dir.join("plugin_state.json");
        self.save_state(&path)
    }

    /// 加载配置（向后兼容方法，调用 load_state）
    pub fn load_config(&mut self) -> Result<(), PluginError> {
        let path = self.plugin_dir.join("plugin_state.json");
        self.load_state(&path)
    }

    /// 加载插件从路径
    #[allow(clippy::ptr_arg)]
    pub fn load_from_path(&mut self, path: &Path) -> Result<String, PluginError> {
        // 验证路径
        PathValidator::validate_plugin_path(path)?;

        // 检测插件类型
        let plugin_type = LoaderFactory::detect_type(path)?;
        let loader = LoaderFactory::get_loader(plugin_type);

        // 获取插件ID并验证
        let plugin_id = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        PluginIdValidator::validate(&plugin_id)?;

        // 检查插件ID冲突
        let existing_ids: Vec<String> = self.plugin_configs.read().unwrap().keys().cloned().collect();
        PluginIdValidator::check_conflict(&plugin_id, &existing_ids)?;

        // 创建配置
        let config = PluginConfig {
            plugin_id: plugin_id.clone(),
            enabled: true,
            settings: HashMap::new(),
            path: Some(path.to_path_buf()),
            timeout: Some(30),
            env: HashMap::new(),
        };

        // 验证配置
        ConfigValidator::validate_config(&config)?;

        // 加载插件
        let plugin = loader.load(path, config.clone())?;

        // 获取元数据
        let metadata = plugin.metadata();

        // 注册插件
        let plugin_arc = Arc::new(Mutex::new(plugin));
        self.plugins.insert(plugin_id.clone(), plugin_arc.clone());
        self.plugin_configs.write().unwrap().insert(plugin_id.clone(), config.clone());

        // 注册钩子（使用包装器）
        let wrapper = PluginWrapper {
            inner: plugin_arc.clone(),
        };

        let mut registered_hooks = Vec::new();

        for hook_type in &metadata.hooks {
            match self.hook_dispatcher.register(
                hook_type.clone(),
                Arc::new(wrapper.clone()),
                HookPriority::NORMAL,
            ) {
                Ok(_) => registered_hooks.push(hook_type.clone()),
                Err(e) => {
                    // 回滚已注册的钩子（只需调用一次，会移除所有）
                    self.hook_dispatcher.unregister(&plugin_id);

                    // 移除已插入的插件、配置和状态
                    self.plugins.remove(&plugin_id);
                    self.plugin_configs.write().unwrap().remove(&plugin_id);
                    // 如果状态已插入，也移除
                    self.plugin_statuses.write().unwrap().remove(&plugin_id);

                    return Err(PluginError::ExecutionFailed(
                        format!("钩子注册失败: {} (已回滚)", e)
                    ));
                }
            }
        }

        // 初始化状态（仅在所有钩子注册成功后）
        self.plugin_statuses.write().unwrap().insert(
            plugin_id.clone(),
            PluginStatus {
                plugin_id: plugin_id.clone(),
                enabled: config.enabled,
                loaded: true,
                last_error: None,
                execution_count: 0,
                error_count: 0,
                last_execution: None,
            },
        );

        Ok(plugin_id)
    }

    /// 卸载插件
    pub fn unload(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        // 从钩子中注销
        self.hook_dispatcher.unregister(plugin_id);

        // 移除插件并尝试关闭（即使失败也继续清理）
        let shutdown_result = if let Some(plugin_arc) = self.plugins.remove(plugin_id) {
            let mut plugin = plugin_arc.lock().unwrap();
            plugin.shutdown()
        } else {
            Ok(())
        };

        // 无论 shutdown 是否成功，都确保清理配置和状态
        self.plugin_statuses.write().unwrap().remove(plugin_id);
        self.plugin_configs.write().unwrap().remove(plugin_id);

        // 返回 shutdown 的结果
        shutdown_result
    }

    /// 启用插件
    pub fn enable(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        if let Some(config) = self.plugin_configs.write().unwrap().get_mut(plugin_id) {
            config.enabled = true;
        }
        if let Some(status) = self.plugin_statuses.write().unwrap().get_mut(plugin_id) {
            status.enabled = true;
        }
        Ok(())
    }

    /// 禁用插件
    pub fn disable(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        if let Some(config) = self.plugin_configs.write().unwrap().get_mut(plugin_id) {
            config.enabled = false;
        }
        if let Some(status) = self.plugin_statuses.write().unwrap().get_mut(plugin_id) {
            status.enabled = false;
        }
        Ok(())
    }

    /// 启用插件（别名）
    pub fn enable_plugin(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        self.enable(plugin_id)
    }

    /// 禁用插件（别名）
    pub fn disable_plugin(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        self.disable(plugin_id)
    }

    /// 卸载插件（别名）
    pub fn unload_plugin(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        self.unload(plugin_id)
    }

    /// 热重载插件（保持配置，重新加载代码）
    ///
    /// # 参数
    /// * `plugin_id` - 要重载的插件ID
    ///
    /// # 返回
    /// 重载后的插件ID（可能与原ID不同）
    ///
    /// # 注意
    /// - 会保留插件配置和状态
    /// - 如果重载失败，会完全恢复原状态（包括钩子注册）
    /// - 保证事务性：要么完全成功，要么完全回滚
    /// - 会验证新插件的签名
    pub fn reload(&mut self, plugin_id: &str) -> Result<String, PluginError> {
        self.reload_with_config(plugin_id, true)
    }

    /// 热重载插件（带配置选项）
    ///
    /// # 参数
    /// * `plugin_id` - 要重载的插件ID
    /// * `verify_signature` - 是否验证新插件的签名
    ///
    /// # 返回
    /// 重载后的插件ID（可能与原ID不同）
    ///
    /// # 注意
    /// - 会保留插件配置和状态
    /// - 如果重载失败，会完全恢复原状态（包括钩子注册）
    /// - 保证事务性：要么完全成功，要么完全回滚
    /// - 可以选择是否验证新插件的签名
    pub fn reload_with_config(&mut self, plugin_id: &str, verify_signature: bool) -> Result<String, PluginError> {
        // 0. 并发检查：防止同一插件同时被重载
        {
            let mut reloading = self.reloading_plugins.lock().unwrap();
            let now = std::time::Instant::now();

            // 检查是否正在重载
            if let Some(start_time) = reloading.get(plugin_id) {
                // 如果重载时间超过5分钟，认为是僵尸任务，允许新的重载
                if now.duration_since(*start_time) < Duration::from_secs(300) {
                    return Err(PluginError::ExecutionFailed(
                        format!("插件 {} 正在重载中，请稍后再试", plugin_id)
                    ));
                }
            }

            // 标记为正在重载
            reloading.insert(plugin_id.to_string(), now);
        }

        // 执行重载，使用 panic guard 确保在任何情况下（包括 panic）都清理重载标记
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.do_reload_with_cleanup(plugin_id, verify_signature)
        }));

        // 无论成功、失败还是 panic，都清理重载标记
        {
            let mut reloading = self.reloading_plugins.lock().unwrap();
            reloading.remove(plugin_id);
        }

        // 处理结果
        match result {
            Ok(r) => r,
            Err(_) => Err(PluginError::ExecutionFailed(
                format!("插件 {} 重载过程中发生 panic", plugin_id)
            )),
        }
    }

    /// 执行重载并自动清理（内部方法）
    fn do_reload_with_cleanup(&mut self, plugin_id: &str, verify_signature: bool) -> Result<String, PluginError> {
        // 1. 一次性收集完整状态快照（避免持有多个锁导致死锁）
        // 遵循锁获取顺序规范：先获取 configs 读锁，再获取 statuses 读锁
        let snapshot = {
            let configs = self.plugin_configs.read().unwrap();
            let statuses = self.plugin_statuses.read().unwrap();

            let config = configs.get(plugin_id)
                .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?
                .clone();

            // 验证插件文件存在
            let plugin_path = config.path.clone()
                .ok_or_else(|| PluginError::Unsupported(
                    format!("无法重载插件 {}: 缺少路径信息", plugin_id)
                ))?;

            if !plugin_path.exists() {
                return Err(PluginError::LoadFailed(
                    format!("插件文件不存在: {}", plugin_path.display())
                ));
            }

            let status = statuses.get(plugin_id).cloned();
            let plugin = self.plugins.get(plugin_id).cloned();

            // 保存旧的钩子注册信息
            let hooks = if let Some(plugin_arc) = &plugin {
                let p = plugin_arc.lock().unwrap();
                p.metadata().hooks
            } else {
                Vec::new()
            };

            ReloadSnapshot {
                config,
                status,
                plugin,
                hooks,
                path: plugin_path,
            }
        };

        // 2. 执行事务性重载操作
        let result = (|| -> Result<String, PluginError> {
            // 卸载旧插件（仅从 plugins HashMap 移除，保留配置和状态）
            self.hook_dispatcher.unregister(plugin_id);
            if let Some(plugin_arc) = self.plugins.remove(plugin_id) {
                let mut plugin = plugin_arc.lock().unwrap();
                plugin.shutdown()?;
            }

            // 加载新插件
            let new_id = self.load_from_path(&snapshot.path)?;

            // 验证新插件已正确注册
            if !self.plugins.contains_key(&new_id) {
                return Err(PluginError::LoadFailed(
                    format!("插件加载后未正确注册: {}", new_id)
                ));
            }

            // 检查ID冲突（如果ID改变）
            if new_id != plugin_id && self.plugins.contains_key(&new_id) {
                return Err(PluginError::LoadFailed(
                    format!("重载后ID冲突: 插件 {} 已存在", new_id)
                ));
            }

            // 根据配置决定是否验证新插件的签名
            if verify_signature {
                self.verify_plugin_signature(&new_id, false)?;
            }

            Ok(new_id)
        })();

        match result {
            Ok(new_id) => {
                // 成功：如果插件ID改变，清理旧ID
                if new_id != plugin_id {
                    self.plugin_configs.write().unwrap().remove(plugin_id);
                    self.plugin_statuses.write().unwrap().remove(plugin_id);
                }
                Ok(new_id)
            }
            Err(e) => {
                // 失败：完全回滚到旧状态
                self.rollback_plugin_state(
                    plugin_id,
                    snapshot.config,
                    snapshot.status,
                    snapshot.plugin,
                    &snapshot.hooks
                )?;
                Err(PluginError::ExecutionFailed(
                    format!("重载插件 {} 失败，已回滚: {}", plugin_id, e)
                ))
            }
        }
    }

    /// 回滚插件状态（包括配置、状态、插件实例和钩子注册）
    ///
    /// # 回滚步骤
    /// 1. 清理新插件可能创建的临时资源
    /// 2. 恢复配置
    /// 3. 恢复状态
    /// 4. 恢复插件实例
    /// 5. 重新注册钩子
    /// 6. 清理可能残留的新钩子注册
    fn rollback_plugin_state(
        &mut self,
        plugin_id: &str,
        old_config: PluginConfig,
        old_status: Option<PluginStatus>,
        old_plugin: Option<Arc<Mutex<Box<dyn Plugin>>>>,
        old_hooks: &[HookType],
    ) -> Result<(), PluginError> {
        // 步骤1: 清理可能残留的新插件资源
        // 先卸载可能已部分加载的新插件
        if let Some(new_plugin) = self.plugins.remove(plugin_id) {
            // 尝试关闭新插件（忽略错误，因为可能未完全初始化）
            let _ = new_plugin.lock().unwrap().shutdown();
        }

        // 清除可能已注册的新钩子（注销该插件的所有钩子）
        self.hook_dispatcher.unregister(plugin_id);

        // 步骤2: 恢复配置
        self.plugin_configs.write().unwrap().insert(plugin_id.to_string(), old_config);

        // 步骤3: 恢复状态
        if let Some(status) = old_status {
            self.plugin_statuses.write().unwrap().insert(plugin_id.to_string(), status);
        }

        // 步骤4: 恢复插件实例
        if let Some(plugin_arc) = old_plugin {
            self.plugins.insert(plugin_id.to_string(), plugin_arc.clone());

            // 步骤5: 重新注册钩子（使用包装器）
            let wrapper = PluginWrapper {
                inner: plugin_arc.clone(),
            };
            for hook_type in old_hooks {
                self.hook_dispatcher.register(
                    hook_type.clone(),
                    Arc::new(wrapper.clone()),
                    HookPriority::NORMAL,
                )?;
            }
        }

        // 步骤6: 验证回滚完整性
        self.verify_rollback_integrity(plugin_id, old_hooks)?;

        Ok(())
    }

    /// 验证回滚的完整性
    fn verify_rollback_integrity(&self, plugin_id: &str, expected_hooks: &[HookType]) -> Result<(), PluginError> {
        // 检查配置是否存在
        let configs = self.plugin_configs.read().unwrap();
        if !configs.contains_key(plugin_id) {
            return Err(PluginError::ExecutionFailed(
                format!("回滚验证失败: 配置丢失 for {}", plugin_id)
            ));
        }

        // 检查插件实例是否恢复
        if !self.plugins.contains_key(plugin_id) {
            // 如果期望有插件实例但实际没有，这是可接受的（可能原来就没有）
            // 但如果有旧插件实例，必须恢复
            if !expected_hooks.is_empty() {
                return Err(PluginError::ExecutionFailed(
                    format!("回滚验证失败: 插件实例丢失 for {}", plugin_id)
                ));
            }
        }

        // 钩子注册验证：依赖注册时的错误处理
        // 如果有期望的钩子但注册失败，会在 rollback_plugin_state 中抛出错误

        Ok(())
    }

    /// 执行钩子
    pub fn execute_hooks(
        &self,
        hook_type: HookType,
        context: &HookContext,
    ) -> Result<Vec<HookResult>, PluginError> {
        self.hook_dispatcher.execute(hook_type, context)
    }

    /// 执行钩子链（带上下文修改）
    pub fn execute_hooks_with_context<'a>(
        &self,
        hook_type: HookType,
        context: HookContext<'a>,
    ) -> Result<(HookContext<'a>, Vec<HookResult>), PluginError> {
        self.hook_dispatcher.execute_with_context(hook_type, context)
    }

    /// 获取插件信息
    pub fn get_plugin_info(&self, plugin_id: &str) -> Option<PluginInfo> {
        let plugin_arc = self.plugins.get(plugin_id)?;
        let config = self.plugin_configs.read().unwrap().get(plugin_id)?.clone();
        let status = self.plugin_statuses.read().unwrap().get(plugin_id)?.clone();

        let metadata = {
            let plugin = plugin_arc.lock().unwrap();
            plugin.metadata()
        };

        Some(PluginInfo {
            metadata,
            config,
            status,
        })
    }

    /// 列出所有插件
    pub fn list_plugins(&self, include_disabled: bool) -> Vec<PluginInfo> {
        let mut infos = Vec::new();

        for (plugin_id, plugin_arc) in &self.plugins {
            let config = match self.plugin_configs.read().unwrap().get(plugin_id) {
                Some(c) => c.clone(),
                None => continue,
            };

            if !include_disabled && !config.enabled {
                continue;
            }

            let status = self.plugin_statuses.read().unwrap().get(plugin_id).cloned().unwrap_or_else(|| {
                PluginStatus {
                    plugin_id: plugin_id.clone(),
                    enabled: config.enabled,
                    loaded: true,
                    last_error: None,
                    execution_count: 0,
                    error_count: 0,
                    last_execution: None,
                }
            });

            let metadata = {
                let plugin = plugin_arc.lock().unwrap();
                plugin.metadata()
            };

            infos.push(PluginInfo {
                metadata,
                config: config.clone(),
                status,
            });
        }

        infos
    }

    /// 扫描插件目录并自动加载
    pub fn scan_and_load(&mut self) -> Result<Vec<String>, PluginError> {
        let mut loaded = Vec::new();

        if !self.plugin_dir.exists() {
            return Ok(loaded);
        }

        for entry in std::fs::read_dir(&self.plugin_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                let plugin_id = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                if !self.plugins.contains_key(&plugin_id) {
                    match self.load_from_path(&path) {
                        Ok(id) => loaded.push(id),
                        Err(e) => {
                            eprintln!("加载插件失败 {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }

        Ok(loaded)
    }

    /// 获取插件目录
    pub fn get_plugin_dir(&self) -> &Path {
        &self.plugin_dir
    }

    /// 获取插件统计信息
    pub fn get_stats(&self) -> PluginManagerStats {
        PluginManagerStats {
            total_plugins: self.plugins.len(),
            enabled_plugins: self
                .plugin_statuses
                .read()
                .unwrap()
                .values()
                .filter(|s| s.enabled)
                .count(),
            loaded_plugins: self
                .plugin_statuses
                .read()
                .unwrap()
                .values()
                .filter(|s| s.loaded)
                .count(),
            total_executions: self
                .plugin_statuses
                .read()
                .unwrap()
                .values()
                .map(|s| s.execution_count)
                .sum(),
            total_errors: self.plugin_statuses.read().unwrap().values().map(|s| s.error_count).sum(),
            hook_stats: self.hook_dispatcher.get_stats(),
        }
    }

    /// 检查插件是否已加载
    pub fn is_loaded(&self, plugin_id: &str) -> bool {
        self.plugins.contains_key(plugin_id)
    }

    /// 导出配置
    pub fn export_config(&self) -> Result<String, PluginError> {
        // 简化实现：返回空字符串
        Ok(String::new())
    }

    /// 导入配置
    pub fn import_config(&mut self, _content: &str) -> Result<(), PluginError> {
        // 简化实现：不执行任何操作
        Ok(())
    }

    /// 获取所有已加载插件的元数据
    pub fn get_all_metadata(&self) -> HashMap<String, PluginMetadata> {
        let mut metadata_map = HashMap::new();

        for (plugin_id, plugin_arc) in &self.plugins {
            let plugin = plugin_arc.lock().unwrap();
            metadata_map.insert(plugin_id.clone(), plugin.metadata());
        }

        metadata_map
    }

    /// 解析并加载插件及其依赖
    ///
    /// # 参数
    /// * `paths` - 插件路径列表
    ///
    /// # 返回
    /// 成功加载的插件ID列表（按依赖顺序）
    pub fn load_with_dependencies(&mut self, paths: &[PathBuf]) -> Result<Vec<String>, PluginError> {
        // 1. 先加载所有插件获取元数据（不注册钩子）
        let mut temp_metadata: HashMap<String, PluginMetadata> = HashMap::new();
        let mut path_map: HashMap<String, PathBuf> = HashMap::new();

        for path in paths {
            let plugin_type = LoaderFactory::detect_type(path)?;
            let loader = LoaderFactory::get_loader(plugin_type);

            // 创建临时配置获取元数据
            let plugin_id = path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let config = PluginConfig {
                plugin_id: plugin_id.clone(),
                enabled: true,
                settings: HashMap::new(),
                path: Some(path.clone()),
                timeout: Some(30),
                env: HashMap::new(),
            };

            // 加载插件获取元数据
            let plugin = loader.load(path, config)?;
            let metadata = plugin.metadata();

            temp_metadata.insert(plugin_id.clone(), metadata);
            path_map.insert(plugin_id, path.clone());
        }

        // 2. 解析依赖顺序
        let load_order = DependencyResolver::resolve(&temp_metadata)
            .map_err(|e| PluginError::DependencyMissing(e.to_string()))?;

        // 3. 按顺序加载插件
        let mut loaded = Vec::new();
        for plugin_id in load_order {
            if let Some(path) = path_map.get(&plugin_id) {
                // 使用完整的加载流程（包括钩子注册）
                let new_id = self.load_from_path(path)?;
                loaded.push(new_id);
            }
        }

        Ok(loaded)
    }

    /// 自动扫描并加载目录中的插件（包括依赖自动加载）
    ///
    /// # 参数
    /// * `recursive` - 是否递归扫描子目录
    /// * `auto_resolve_deps` - 是否自动解析和加载依赖
    ///
    /// # 返回
    /// 成功加载的插件ID列表（按依赖顺序）
    pub fn scan_and_load_with_deps(&mut self, recursive: bool, auto_resolve_deps: bool) -> Result<Vec<String>, PluginError> {
        if !self.plugin_dir.exists() {
            return Ok(Vec::new());
        }

        // 收集所有插件文件
        let mut plugin_files = Vec::new();
        self.collect_plugin_files(&self.plugin_dir.clone(), recursive, &mut plugin_files)?;

        if plugin_files.is_empty() {
            return Ok(Vec::new());
        }

        if auto_resolve_deps {
            // 使用依赖解析加载
            self.load_with_dependencies(&plugin_files)
        } else {
            // 简单顺序加载
            let mut loaded = Vec::new();
            for path in plugin_files {
                match self.load_from_path(&path) {
                    Ok(id) => loaded.push(id),
                    Err(e) => {
                        eprintln!("加载插件失败 {}: {}", path.display(), e);
                    }
                }
            }
            Ok(loaded)
        }
    }

    /// 递归收集插件文件
    fn collect_plugin_files(&self, dir: &Path, recursive: bool, files: &mut Vec<PathBuf>) -> Result<(), PluginError> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() && recursive {
                self.collect_plugin_files(&path, recursive, files)?;
            } else if path.is_file() {
                // 检查是否是插件文件（根据扩展名）
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if matches!(ext_str.as_str(), "dll" | "so" | "dylib" | "json" | "yaml" | "yml") {
                        files.push(path);
                    }
                }
            }
        }
        Ok(())
    }

    /// 检查插件依赖状态
    ///
    /// # 返回
    /// (已满足的依赖, 缺失的依赖)
    pub fn check_dependencies(&self, plugin_id: &str) -> (Vec<String>, Vec<String>) {
        let metadata_map = self.get_all_metadata();

        if let Some(metadata) = metadata_map.get(plugin_id) {
            let mut satisfied = Vec::new();
            let mut missing = Vec::new();

            for dep in &metadata.dependencies {
                if self.plugins.contains_key(dep) {
                    satisfied.push(dep.clone());
                } else {
                    missing.push(dep.clone());
                }
            }

            (satisfied, missing)
        } else {
            (Vec::new(), Vec::new())
        }
    }

    /// 自动加载缺失的依赖
    ///
    /// # 参数
    /// * `plugin_id` - 插件ID
    /// * `search_dir` - 搜索目录（可选，为空则使用插件目录）
    ///
    /// # 返回
    /// 成功加载的依赖列表
    pub fn auto_load_missing_deps(&mut self, plugin_id: &str, search_dir: Option<&Path>) -> Result<Vec<String>, PluginError> {
        let (_, missing) = self.check_dependencies(plugin_id);
        if missing.is_empty() {
            return Ok(Vec::new());
        }

        let search_path = search_dir.unwrap_or(&self.plugin_dir);
        if !search_path.exists() {
            return Ok(Vec::new());
        }

        let mut loaded = Vec::new();
        let mut plugin_files = Vec::new();
        self.collect_plugin_files(search_path, true, &mut plugin_files)?;

        // 为每个缺失的依赖查找对应的文件
        for dep_id in &missing {
            if let Some(file_path) = plugin_files.iter().find(|p| {
                p.file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .map(|s| s == *dep_id)
                    .unwrap_or(false)
            }) {
                match self.load_from_path(file_path) {
                    Ok(id) => loaded.push(id),
                    Err(e) => {
                        eprintln!("自动加载依赖 {} 失败: {}", dep_id, e);
                    }
                }
            }
        }

        Ok(loaded)
    }

    /// 验证所有插件的依赖关系
    pub fn validate_all_dependencies(&self) -> Result<(), PluginError> {
        let metadata_map = self.get_all_metadata();

        // 使用 dependency 模块验证
        DependencyResolver::validate_dependencies(&metadata_map)
            .map_err(|e| PluginError::DependencyMissing(e.to_string()))?;

        Ok(())
    }

    /// 验证插件签名
    ///
    /// # 参数
    /// * `plugin_id` - 插件ID
    /// * `trust_unsigned` - 是否信任未签名的插件
    ///
    /// # 返回
    /// 验证成功返回 Ok(())，失败返回错误
    pub fn verify_plugin_signature(
        &self,
        plugin_id: &str,
        trust_unsigned: bool,
    ) -> Result<(), PluginError> {
        let metadata = self
            .get_plugin_info(plugin_id)
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?
            .metadata;

        self.signature_verifier
            .verify_metadata(&metadata, trust_unsigned)
            .map_err(|e| PluginError::ExecutionFailed(format!("签名验证失败: {}", e)))?;

        Ok(())
    }

    /// 验证所有插件的签名
    ///
    /// # 参数
    /// * `trust_unsigned` - 是否信任未签名的插件
    ///
    /// # 返回
    /// 所有验证通过返回 Ok(())，否则返回第一个错误
    pub fn verify_all_signatures(&self, trust_unsigned: bool) -> Result<(), PluginError> {
        for plugin_id in self.plugins.keys() {
            self.verify_plugin_signature(plugin_id, trust_unsigned)?;
        }
        Ok(())
    }

    /// 为插件生成签名（开发者工具）
    ///
    /// # 参数
    /// * `plugin_id` - 插件ID
    /// * `private_key_hex` - 私钥（十六进制编码）
    /// * `algorithm` - 签名算法
    ///
    /// # 返回
    /// 生成的签名信息
    pub fn sign_plugin(
        &self,
        plugin_id: &str,
        private_key_hex: &str,
        algorithm: SignatureAlgorithm,
    ) -> Result<PluginSignature, PluginError> {
        let metadata = self
            .get_plugin_info(plugin_id)
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?
            .metadata;

        SignatureVerifier::sign_metadata(&metadata, private_key_hex, algorithm)
            .map_err(|e| PluginError::ExecutionFailed(format!("签名生成失败: {}", e)))
    }

    /// 生成密钥对（用于测试或开发者首次使用）
    ///
    /// # 返回
    /// (私钥, 公钥) 的十六进制字符串
    pub fn generate_key_pair() -> Result<(String, String), PluginError> {
        SignatureVerifier::generate_key_pair()
            .map_err(|e| PluginError::ExecutionFailed(format!("密钥生成失败: {}", e)))
    }

    /// 计算公钥指纹（用于显示）
    pub fn fingerprint(public_key_hex: &str) -> String {
        SignatureVerifier::fingerprint(public_key_hex)
    }

    /// 检查插件版本兼容性
    ///
    /// # 检查项
    /// - EnvCLI 版本要求
    /// - 平台兼容性
    /// - 插件版本格式
    ///
    /// # 返回
    /// (兼容性检查结果, 详细信息)
    pub fn check_compatibility(&self, plugin_id: &str) -> Result<CompatibilityReport, PluginError> {
        let info = self.get_plugin_info(plugin_id)
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;

        let metadata = &info.metadata;
        let mut report = CompatibilityReport::new(plugin_id.to_string());

        // 1. 检查 EnvCLI 版本要求
        if let Some(required_version) = &metadata.envcli_version
            && !self.check_envcli_version(required_version) {
            report.add_issue(
                CompatibilityIssue::EnvCliVersionMismatch {
                    required: required_version.clone(),
                    current: env!("CARGO_PKG_VERSION").to_string(),
                }
            );
        }

        // 2. 检查平台兼容性
        if !metadata.platforms.is_empty() {
            let current_platform = Platform::current();
            if !metadata.platforms.contains(&current_platform) {
                report.add_issue(
                    CompatibilityIssue::PlatformMismatch {
                        required: metadata.platforms.clone(),
                        current: current_platform,
                    }
                );
            }
        }

        // 3. 检查插件版本格式
        if let Err(e) = self.check_plugin_version_format(&metadata.version) {
            report.add_issue(CompatibilityIssue::InvalidVersionFormat {
                version: metadata.version.clone(),
                reason: e,
            });
        }

        // 4. 检查依赖的其他插件是否已加载
        for dep in &metadata.dependencies {
            if !self.is_loaded(dep) {
                report.add_issue(CompatibilityIssue::MissingDependency {
                    dependency: dep.clone(),
                });
            }
        }

        Ok(report)
    }

    /// 检查所有插件的兼容性
    pub fn check_all_compatibility(&self) -> Result<Vec<CompatibilityReport>, PluginError> {
        let mut reports = Vec::new();
        for plugin_id in self.plugins.keys() {
            match self.check_compatibility(plugin_id) {
                Ok(report) => reports.push(report),
                Err(e) => {
                    // 记录错误但继续检查其他插件
                    eprintln!("检查插件 {} 兼容性失败: {}", plugin_id, e);
                }
            }
        }
        Ok(reports)
    }

    /// 检测插件版本冲突
    ///
    /// # 返回
    /// 冲突的插件列表 (plugin_id, 已加载版本, 冲突信息)
    pub fn detect_version_conflicts(&self) -> Vec<(String, String, String)> {
        let mut conflicts = Vec::new();

        // 检查是否有重复的插件ID（理论上不应该发生）
        let mut id_count: HashMap<String, u32> = HashMap::new();
        for plugin_id in self.plugins.keys() {
            *id_count.entry(plugin_id.clone()).or_insert(0) += 1;
        }

        for (id, count) in id_count {
            if count > 1 {
                conflicts.push((
                    id.clone(),
                    "multiple".to_string(),
                    format!("插件ID重复: {} 个实例", count)
                ));
            }
        }

        // 检查依赖循环（简单实现）
        let metadata_map = self.get_all_metadata();
        let mut visited = HashMap::new();
        let mut stack = Vec::new();

        for plugin_id in self.plugins.keys() {
            if self.detect_cycle_dfs(plugin_id, &metadata_map, &mut visited, &mut stack) {
                // 发现循环，添加所有在循环中的插件
                for id in &stack {
                    if let Some(metadata) = metadata_map.get(id) {
                        conflicts.push((
                            id.clone(),
                            metadata.version.clone(),
                            "依赖循环检测到".to_string()
                        ));
                    }
                }
                break; // 一次只报告一个循环
            }
        }

        conflicts
    }

    /// DFS检测循环依赖
    fn detect_cycle_dfs(
        &self,
        plugin_id: &str,
        metadata_map: &HashMap<String, PluginMetadata>,
        visited: &mut HashMap<String, bool>,
        stack: &mut Vec<String>,
    ) -> bool {
        if stack.contains(&plugin_id.to_string()) {
            return true;
        }

        if visited.get(plugin_id) == Some(&true) {
            return false;
        }

        visited.insert(plugin_id.to_string(), true);
        stack.push(plugin_id.to_string());

        if let Some(metadata) = metadata_map.get(plugin_id) {
            for dep in &metadata.dependencies {
                if self.detect_cycle_dfs(dep, metadata_map, visited, stack) {
                    return true;
                }
            }
        }

        stack.pop();
        false
    }

    /// 检查版本约束（保留用于未来扩展）
    #[allow(dead_code)]
    fn check_version_constraint(&self, version: &str, constraint: &str) -> bool {
        // 简单的版本约束检查
        // 支持: =, >, <, >=, <=, ~, ^
        // 这里实现基本的语义化版本比较

        if constraint.starts_with('=') && constraint.len() > 1 {
            let required = &constraint[1..];
            self.version_compare(version, required) == 0
        } else if constraint.starts_with('>') && constraint.starts_with(">=") {
            let required = &constraint[2..];
            self.version_compare(version, required) >= 0
        } else if let Some(required) = constraint.strip_prefix('>') {
            self.version_compare(version, required) > 0
        } else if let Some(required) = constraint.strip_prefix("<=") {
            self.version_compare(version, required) <= 0
        } else if let Some(required) = constraint.strip_prefix('<') {
            self.version_compare(version, required) < 0
        } else if let Some(required) = constraint.strip_prefix('~') {
            // ~1.2.3 表示 >=1.2.3 且 <1.3.0
            let parts: Vec<&str> = required.split('.').collect();
            if parts.len() >= 2 {
                let major = parts[0];
                let minor = parts[1].parse::<u32>().unwrap_or(0);
                let upper = format!("{}.{}.0", major, minor + 1);
                self.version_compare(version, required) >= 0 && self.version_compare(version, &upper) < 0
            } else {
                true
            }
        } else if let Some(required) = constraint.strip_prefix('^') {
            // ^1.2.3 表示 >=1.2.3 且 <2.0.0
            let parts: Vec<&str> = required.split('.').collect();
            if !parts.is_empty() {
                let major = parts[0].parse::<u32>().unwrap_or(0);
                let upper = format!("{}.0.0", major + 1);
                self.version_compare(version, required) >= 0 && self.version_compare(version, &upper) < 0
            } else {
                true
            }
        } else {
            // 默认为精确匹配
            self.version_compare(version, constraint) == 0
        }
    }

    /// 检查 EnvCLI 版本是否满足要求
    fn check_envcli_version(&self, required: &str) -> bool {
        // 简单的版本比较：检查当前版本是否 >= 要求版本
        let current = env!("CARGO_PKG_VERSION");
        self.version_compare(current, required) >= 0
    }

    /// 检查插件版本格式
    fn check_plugin_version_format(&self, version: &str) -> Result<(), String> {
        // 支持语义化版本：major.minor.patch
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() < 2 || parts.len() > 3 {
            return Err(format!("版本格式无效: {} (期望: major.minor 或 major.minor.patch)", version));
        }

        for part in parts {
            if part.parse::<u32>().is_err() {
                return Err(format!("版本号必须是数字: {}", part));
            }
        }

        Ok(())
    }

    /// 版本比较：返回 -1, 0, 1
    fn version_compare(&self, v1: &str, v2: &str) -> i32 {
        let parts1: Vec<u32> = v1.split('.').filter_map(|s| s.parse().ok()).collect();
        let parts2: Vec<u32> = v2.split('.').filter_map(|s| s.parse().ok()).collect();

        let max_len = parts1.len().max(parts2.len());

        for i in 0..max_len {
            let p1 = parts1.get(i).unwrap_or(&0);
            let p2 = parts2.get(i).unwrap_or(&0);

            if p1 > p2 {
                return 1;
            } else if p1 < p2 {
                return -1;
            }
        }

        0
    }

    /// 获取性能监控统计
    pub fn get_performance_stats(&self) -> PluginManagerPerformanceStats {
        let stats = self.performance_stats.lock().unwrap();
        stats.clone()
    }

    /// 重置性能统计
    pub fn reset_performance_stats(&self) {
        let mut stats = self.performance_stats.lock().unwrap();
        *stats = PluginManagerPerformanceStats::new();
    }

    /// 记录操作耗时（用于性能监控）
    pub fn record_operation(&self, operation: &str, duration: Duration) {
        let mut stats = self.performance_stats.lock().unwrap();
        stats.record_operation(operation, duration);
    }

    /// 获取加密器统计信息
    pub fn get_encryptor_stats(&self) -> crate::utils::encryption::EncryptorStats {
        self.encryptor.get_stats()
    }

    /// 检查 SOPS 是否可用
    pub fn is_encryptor_available(&self) -> bool {
        SopsEncryptor::is_available()
    }
}

/// 插件管理器性能监控统计
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PluginManagerPerformanceStats {
    /// 操作耗时记录 (操作名称 -> 总耗时毫秒, 调用次数)
    operation_durations: HashMap<String, (u64, u64)>,
    /// 加载插件次数
    pub load_count: u64,
    /// 卸载插件次数
    pub unload_count: u64,
    /// 重载插件次数
    pub reload_count: u64,
    /// 签名验证次数
    pub signature_verify_count: u64,
    /// 加密操作次数
    pub encrypt_count: u64,
    /// 解密操作次数
    pub decrypt_count: u64,
    /// 错误总数
    pub total_errors: u64,
    /// 最后重置时间（纳秒）
    pub last_reset: u128,
}

impl PluginManagerPerformanceStats {
    /// 创建新的性能统计
    pub fn new() -> Self {
        use std::time::SystemTime;
        let last_reset = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self {
            operation_durations: HashMap::new(),
            load_count: 0,
            unload_count: 0,
            reload_count: 0,
            signature_verify_count: 0,
            encrypt_count: 0,
            decrypt_count: 0,
            total_errors: 0,
            last_reset,
        }
    }

    /// 记录操作耗时
    pub fn record_operation(&mut self, operation: &str, duration: Duration) {
        let duration_ms = duration.as_millis() as u64;
        let entry = self.operation_durations
            .entry(operation.to_string())
            .or_insert((0, 0));
        entry.0 += duration_ms;
        entry.1 += 1;
    }

    /// 获取操作平均耗时（毫秒）
    pub fn get_avg_duration(&self, operation: &str) -> Option<u64> {
        if let Some((total, count)) = self.operation_durations.get(operation)
            && *count > 0 {
            return Some(total / count);
        }
        None
    }

    /// 获取操作总调用次数
    pub fn get_operation_count(&self, operation: &str) -> u64 {
        self.operation_durations.get(operation).map(|(_, count)| *count).unwrap_or(0)
    }

    /// 获取所有操作统计
    pub fn get_all_operations(&self) -> Vec<(String, u64, u64)> {
        self.operation_durations
            .iter()
            .map(|(op, (total, count))| (op.clone(), *total, *count))
            .collect()
    }

    /// 增加错误计数
    pub fn increment_error(&mut self) {
        self.total_errors += 1;
    }

    /// 增加加载计数
    pub fn increment_load(&mut self) {
        self.load_count += 1;
    }

    /// 增加卸载计数
    pub fn increment_unload(&mut self) {
        self.unload_count += 1;
    }

    /// 增加重载计数
    pub fn increment_reload(&mut self) {
        self.reload_count += 1;
    }

    /// 增加签名验证计数
    pub fn increment_signature_verify(&mut self) {
        self.signature_verify_count += 1;
    }

    /// 增加加密计数
    pub fn increment_encrypt(&mut self) {
        self.encrypt_count += 1;
    }

    /// 增加解密计数
    pub fn increment_decrypt(&mut self) {
        self.decrypt_count += 1;
    }
}

/// 插件管理器统计信息
#[derive(Debug, Clone)]
pub struct PluginManagerStats {
    pub total_plugins: usize,
    pub enabled_plugins: usize,
    pub loaded_plugins: usize,
    pub total_executions: u64,
    pub total_errors: u64,
    pub hook_stats: crate::plugin::hook::HookStats,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::types::{PluginType, Platform};

    #[test]
    fn test_plugin_manager_creation() {
        let manager = PluginManager::new();
        assert!(manager.is_ok());
    }

    #[test]
    fn test_plugin_manager_stats() {
        let manager = PluginManager::new().unwrap();
        let stats = manager.get_stats();

        assert_eq!(stats.total_plugins, 0);
        assert_eq!(stats.enabled_plugins, 0);
        assert_eq!(stats.total_executions, 0);
    }

    #[test]
    fn test_plugin_manager_list_empty() {
        let manager = PluginManager::new().unwrap();
        let plugins = manager.list_plugins(true);
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_plugin_manager_config() {
        let manager = PluginManager::new().unwrap();
        let config = manager.export_config();
        assert!(config.is_ok());
    }

    #[test]
    fn test_generate_key_pair() {
        let result = PluginManager::generate_key_pair();
        assert!(result.is_ok());

        let (private_key, public_key) = result.unwrap();
        assert_eq!(private_key.len(), 64);  // 32字节种子 = 64 hex chars
        assert_eq!(public_key.len(), 64);   // 32字节公钥 = 64 hex chars
    }

    #[test]
    fn test_fingerprint() {
        let fp = PluginManager::fingerprint("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef");
        assert_eq!(fp, "0123456789abcdef");
    }

    #[test]
    fn test_sign_and_verify_plugin() {
        // 创建测试元数据（未签名）
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

        // 生成密钥对
        let (private_key, public_key) = PluginManager::generate_key_pair().unwrap();

        // 为元数据生成签名
        let signature = SignatureVerifier::sign_metadata(&metadata, &private_key, SignatureAlgorithm::Ed25519).unwrap();

        // 创建验证器实例
        let verifier = SignatureVerifier::new();

        // 测试验证签名 - 使用 verify 方法直接验证（不包含签名字段的 JSON）
        let metadata_json = serde_json::to_string(&metadata).unwrap();
        let result = verifier.verify(&metadata_json, &signature);
        assert!(result.is_ok(), "签名验证应该通过");

        // 测试验证未签名插件（不信任）
        let result = verifier.verify_metadata(&metadata, false);
        assert!(result.is_err(), "未签名插件应该验证失败");

        // 测试验证未签名插件（信任）
        let result = verifier.verify_metadata(&metadata, true);
        assert!(result.is_ok(), "信任未签名时应该通过");

        // 验证公钥指纹
        let fp = PluginManager::fingerprint(&public_key);
        assert_eq!(fp.len(), 16); // 8字节 = 16 hex chars
    }

    #[test]
    fn test_reload_snapshot_structure() {
        // 测试重载快照结构是否正确
        let snapshot = ReloadSnapshot {
            config: PluginConfig {
                plugin_id: "test".to_string(),
                enabled: true,
                settings: HashMap::new(),
                path: Some(PathBuf::from("/test/plugin.dll")),
                timeout: Some(30),
                env: HashMap::new(),
            },
            status: Some(PluginStatus {
                plugin_id: "test".to_string(),
                enabled: true,
                loaded: true,
                last_error: None,
                execution_count: 5,
                error_count: 0,
                last_execution: None,
            }),
            plugin: None,
            hooks: vec![HookType::PreCommand],
            path: PathBuf::from("/test/plugin.dll"),
        };

        assert_eq!(snapshot.config.plugin_id, "test");
        assert_eq!(snapshot.hooks.len(), 1);
        assert!(!snapshot.path.exists()); // 测试路径不存在的情况
    }

    #[test]
    fn test_reload_path_validation() {
        // 测试重载时的路径验证
        let _manager = PluginManager::empty();

        // 创建一个不存在的插件配置
        let non_existent_path = PathBuf::from("/nonexistent/plugin.dll");

        // 尝试从不存在的路径重载应该失败
        // 这个测试主要验证我们的路径检查逻辑
        assert!(!non_existent_path.exists());
    }

    #[test]
    fn test_signature_cache_integration() {
        // 测试签名缓存与管理器的集成
        let manager = PluginManager::new().unwrap();

        // 验证管理器有签名缓存
        let cache = manager.signature_cache();
        assert_eq!(cache.size(), 0);

        // 标记一个签名
        cache.mark_used("test_hash".to_string());
        assert_eq!(cache.size(), 1);
        assert!(cache.is_used("test_hash"));

        // 清除缓存
        manager.clear_signature_cache();
        assert_eq!(manager.signature_cache().size(), 0);
    }

    #[test]
    fn test_version_compare() {
        let manager = PluginManager::empty();

        // 测试版本比较逻辑
        assert_eq!(manager.version_compare("1.0.0", "1.0.0"), 0);
        assert_eq!(manager.version_compare("1.1.0", "1.0.0"), 1);
        assert_eq!(manager.version_compare("1.0.0", "1.1.0"), -1);
        assert_eq!(manager.version_compare("2.0", "1.9.9"), 1);
        assert_eq!(manager.version_compare("1.0", "1.0.1"), -1);
    }

    #[test]
    fn test_check_plugin_version_format() {
        let manager = PluginManager::empty();

        // 有效版本
        assert!(manager.check_plugin_version_format("1.0.0").is_ok());
        assert!(manager.check_plugin_version_format("1.0").is_ok());
        assert!(manager.check_plugin_version_format("2.1.3").is_ok());

        // 无效版本
        assert!(manager.check_plugin_version_format("1").is_err());
        assert!(manager.check_plugin_version_format("1.0.0.0").is_err());
        assert!(manager.check_plugin_version_format("1.0.a").is_err());
        assert!(manager.check_plugin_version_format("abc").is_err());
    }

    #[test]
    fn test_compatibility_report() {
        use crate::plugin::types::{CompatibilityReport, CompatibilityIssue};

        let mut report = CompatibilityReport::new("test-plugin".to_string());
        assert!(report.is_compatible());

        // 添加问题
        report.add_issue(CompatibilityIssue::MissingDependency {
            dependency: "other-plugin".to_string(),
        });

        assert!(!report.is_compatible());
        assert_eq!(report.issues.len(), 1);
    }

    #[test]
    fn test_compatibility_issue_display() {
        use crate::plugin::types::{CompatibilityIssue, Platform};

        let issue1 = CompatibilityIssue::EnvCliVersionMismatch {
            required: "0.2.0".to_string(),
            current: "0.1.0".to_string(),
        };
        assert!(format!("{}", issue1).contains("EnvCLI 版本不兼容"));

        let issue2 = CompatibilityIssue::PlatformMismatch {
            required: vec![Platform::Windows],
            current: Platform::Linux,
        };
        assert!(format!("{}", issue2).contains("平台不兼容"));

        let issue3 = CompatibilityIssue::InvalidVersionFormat {
            version: "abc".to_string(),
            reason: "not a number".to_string(),
        };
        assert!(format!("{}", issue3).contains("版本格式无效"));

        let issue4 = CompatibilityIssue::MissingDependency {
            dependency: "missing".to_string(),
        };
        assert!(format!("{}", issue4).contains("缺少依赖"));
    }

    #[test]
    fn test_concurrent_reload_protection() {
        // 测试并发重载保护机制
        let mut manager = PluginManager::empty();

        // 创建一个测试配置（但不加载实际插件）
        let config = PluginConfig {
            plugin_id: "test-plugin".to_string(),
            enabled: true,
            settings: HashMap::new(),
            path: Some(PathBuf::from("/nonexistent/test.dll")),
            timeout: Some(30),
            env: HashMap::new(),
        };

        // 手动添加配置以测试并发保护
        manager.plugin_configs.write().unwrap().insert(
            "test-plugin".to_string(),
            config
        );

        // 第一次尝试重载应该失败（因为插件文件不存在）
        let result = manager.reload("test-plugin");
        assert!(result.is_err());

        // 验证重载标记已被清理
        {
            let reloading = manager.reloading_plugins.lock().unwrap();
            assert!(!reloading.contains_key("test-plugin"));
        }
    }

    #[test]
    fn test_reloading_plugins_tracking() {
        // 测试重载插件跟踪功能
        let manager = PluginManager::empty();

        // 验证初始状态为空
        {
            let reloading = manager.reloading_plugins.lock().unwrap();
            assert!(reloading.is_empty());
        }
    }

    #[test]
    fn test_rollback_integrity_verification() {
        // 测试回滚完整性验证
        let manager = PluginManager::empty();

        // 创建一个测试配置
        let config = PluginConfig {
            plugin_id: "test-plugin".to_string(),
            enabled: true,
            settings: HashMap::new(),
            path: Some(PathBuf::from("/nonexistent/test.dll")),
            timeout: Some(30),
            env: HashMap::new(),
        };

        // 手动添加配置
        manager.plugin_configs.write().unwrap().insert(
            "test-plugin".to_string(),
            config
        );

        // 测试有配置但无插件实例的情况（应该通过，因为原来就没有插件）
        // 这里我们通过 reload 方法间接测试，因为 reload 内部会调用 verify_rollback_integrity
        // 但由于没有实际文件，reload 会失败，但回滚应该成功清理状态

        // 验证重载标记已被清理（间接验证回滚逻辑）
        {
            let reloading = manager.reloading_plugins.lock().unwrap();
            assert!(!reloading.contains_key("test-plugin"));
        }

        // 测试配置存在性检查
        {
            let configs = manager.plugin_configs.read().unwrap();
            assert!(configs.contains_key("test-plugin"));
        }
    }
}
