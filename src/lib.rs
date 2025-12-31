//! EnvCLI 库
//!
//! 跨平台环境变量管理工具

// 核心模块
pub mod error;
pub mod types;
pub mod utils;

// 功能模块
pub mod config;
pub mod core;
pub mod template;

// 插件系统
pub mod plugin;

// CLI
pub mod cli;

// 重新导出主要类型
pub use error::{EnvError, Result};
pub use types::{Config, EnvSource, EnvVar, OutputFormat};

// 插件系统快捷访问
pub use plugin::{
    GlobalSettings, HookChainBuilder, HookContext, HookDispatcher, HookErrorHandler, HookExecutor,
    HookPriority, HookResult, HookStats, HookType, LoaderFactory, PLUGIN_SDK_VERSION,
    PLUGIN_SYSTEM_VERSION, Platform, Plugin, PluginConfig, PluginConfigFormatter,
    PluginConfigManager, PluginError, PluginInfo, PluginLoader, PluginManager, PluginManagerStats,
    PluginMetadata, PluginRequest, PluginResponse, PluginStatus, PluginType,
    create_default_manager,
};

// 宏导出 (使用 #[macro_export] 定义的宏在 crate 根自动可用)
