//! 插件系统
//!
//! 提供可扩展的插件架构，支持动态库和外部可执行文件插件。
//!
//! # 模块结构
//!
//! - `types.rs` - 核心类型定义
//! - `manager.rs` - 插件管理器
//! - `config.rs` - 配置管理
//! - `hook.rs` - 钩子系统
//! - `dependency.rs` - 依赖管理
//! - `signature.rs` - 签名验证
//! - `watcher.rs` - 文件监控和自动热重载
//! - `loader/` - 插件加载器
//! - `api.rs` - 插件 API 和辅助宏

// 子模块定义
pub mod types;
pub mod manager;
pub mod config;
pub mod hook;
pub mod dependency;
pub mod signature;
pub mod watcher;
pub mod loader;
pub mod api;
pub mod validation;

// 重新导出核心类型
pub use types::{
    ExtensionPoint, HookContext, HookPriority, HookResult, HookType, Plugin, PluginConfig,
    PluginError, PluginInfo, PluginMetadata, PluginResponse, PluginStatus, PluginType, Platform,
    CreatePluginFn, PluginRequest, PluginSignature, SignatureAlgorithm,
    CompatibilityIssue, CompatibilityReport,
};

// 重新导出管理器
pub use manager::{PluginManager, PluginManagerStats};

// 重新导出配置管理器
pub use config::{PluginConfigManager, PluginGlobalConfig, GlobalSettings, PluginConfigFormatter};

// 重新导出钩子系统
pub use hook::{HookDispatcher, HookExecutor, HookErrorHandler, HookChainBuilder, HookStats};

// 重新导出加载器
pub use loader::{LoaderFactory, PluginLoader};

// 重新导出 API
pub use api::{helpers, PLUGIN_SDK_VERSION};

// 重新导出签名验证相关
pub use signature::{SignatureVerifier, SignatureCache, ThreadSafeSignatureCache, TimestampConfig, SignatureError};

// 重新导出监控器相关
pub use watcher::{PluginWatcher, AutoReloadConfig, ReloadResult, FileChangeEvent};

/// 插件系统版本
pub const PLUGIN_SYSTEM_VERSION: &str = "0.3.0";

/// 创建默认插件管理器
///
/// # 示例
///
/// ```
/// use envcli::plugin::create_default_manager;
///
/// let manager = create_default_manager().unwrap();
/// ```
pub fn create_default_manager() -> Result<PluginManager, PluginError> {
    PluginManager::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_system_version() {
        assert_eq!(PLUGIN_SYSTEM_VERSION, "0.3.0");
    }

    #[test]
    fn test_create_default_manager() {
        let result = create_default_manager();
        assert!(result.is_ok());
    }

    #[test]
    fn test_module_exports() {
        // 验证所有关键类型都可访问
        use std::collections::HashMap;

        let _metadata = PluginMetadata {
            id: "test".to_string(),
            name: "Test".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            plugin_type: PluginType::DynamicLibrary,
            hooks: vec![HookType::PreCommand],
            extensions: vec![],
            config_schema: None,
            enabled: true,
            dependencies: vec![],
            platforms: vec![Platform::current()],
            envcli_version: None,
            signature: None,
        };

        let _config = PluginConfig {
            plugin_id: "test".to_string(),
            enabled: true,
            settings: HashMap::new(),
            path: None,
            timeout: Some(30),
            env: HashMap::new(),
        };

        let _context = HookContext {
            command: "test",
            args: &[],
            env: HashMap::new(),
            plugin_data: HashMap::new(),
            continue_execution: true,
            error: None,
        };

        let _result = HookResult::default();
        let _priority = HookPriority::NORMAL;
    }
}
