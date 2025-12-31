//! 插件系统类型定义
//!
//! 定义插件系统的核心数据结构和枚举类型

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// 插件类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginType {
    /// Rust 动态库插件 (.so/.dll/.dylib)
    DynamicLibrary,
    /// 外部可执行文件插件
    ExternalExecutable,
    /// WASM 插件 (未来扩展)
    Wasm,
}

/// 钩子类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookType {
    /// 命令执行前
    PreCommand,
    /// 命令执行后
    PostCommand,
    /// 错误处理
    Error,
    /// 运行前 (run 命令专用)
    PreRun,
    /// 运行后 (run 命令专用)
    PostRun,
    /// 配置加载
    ConfigLoad,
    /// 配置保存
    ConfigSave,
}

/// 扩展点
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtensionPoint {
    /// 自定义命令
    CustomCommand,
    /// 自定义格式化器
    CustomFormatter,
    /// 自定义存储后端
    CustomStorage,
    /// 自定义加密器
    CustomEncryptor,
}

/// 平台支持
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Platform {
    Windows,
    Linux,
    MacOS,
}

impl Platform {
    /// 获取当前平台
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::MacOS
        } else {
            Platform::Linux
        }
    }

    /// 检查是否兼容
    pub fn is_compatible(&self) -> bool {
        *self == Platform::current()
    }
}

/// 配置字段类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldType {
    String,
    Number,
    Boolean,
    Path,
}

/// 配置字段定义
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigField {
    pub name: String,
    pub field_type: FieldType,
    pub required: bool,
    pub default: Option<String>,
    pub description: Option<String>,
}

/// 配置模式
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigSchema {
    pub fields: Vec<ConfigField>,
}

/// 签名算法类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignatureAlgorithm {
    /// Ed25519 算法（推荐，快速且安全）
    Ed25519,
}

impl std::fmt::Display for SignatureAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignatureAlgorithm::Ed25519 => write!(f, "Ed25519"),
        }
    }
}

/// 插件签名信息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginSignature {
    /// 签名算法
    pub algorithm: SignatureAlgorithm,
    /// 公钥（十六进制编码）
    pub public_key: String,
    /// 签名（十六进制编码）
    pub signature: String,
    /// 签名时间戳（Unix timestamp）
    pub signed_at: u64,
}

/// 插件元数据
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// 插件标识符 (唯一)
    pub id: String,
    /// 插件名称
    pub name: String,
    /// 插件版本
    pub version: String,
    /// 插件描述
    pub description: Option<String>,
    /// 作者信息
    pub author: Option<String>,
    /// 插件类型
    pub plugin_type: PluginType,
    /// 支持的钩子类型
    pub hooks: Vec<HookType>,
    /// 扩展点
    pub extensions: Vec<ExtensionPoint>,
    /// 配置项模式
    pub config_schema: Option<ConfigSchema>,
    /// 是否启用
    pub enabled: bool,
    /// 依赖的其他插件
    pub dependencies: Vec<String>,
    /// 平台兼容性
    pub platforms: Vec<Platform>,
    /// EnvCLI 版本要求
    pub envcli_version: Option<String>,
    /// 插件签名（可选，未签名为 None）
    pub signature: Option<PluginSignature>,
}

/// 插件配置
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginConfig {
    /// 插件 ID
    pub plugin_id: String,
    /// 是否启用
    pub enabled: bool,
    /// 插件特定配置
    pub settings: HashMap<String, String>,
    /// 插件路径 (对于外部插件)
    pub path: Option<PathBuf>,
    /// 超时设置 (秒)
    pub timeout: Option<u64>,
    /// 环境变量
    pub env: HashMap<String, String>,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            plugin_id: String::new(),
            enabled: true,
            settings: HashMap::new(),
            path: None,
            timeout: Some(30),
            env: HashMap::new(),
        }
    }
}

/// 钩子上下文 (生命周期参数)
#[derive(Debug)]
pub struct HookContext<'a> {
    /// 命令名称
    pub command: &'a str,
    /// 命令参数
    pub args: &'a [String],
    /// 当前环境变量
    pub env: HashMap<String, String>,
    /// 插件数据 (插件间共享)
    pub plugin_data: HashMap<String, String>,
    /// 是否继续执行
    pub continue_execution: bool,
    /// 错误信息 (仅在 Error hook 中)
    pub error: Option<String>,
}

impl<'a> Clone for HookContext<'a> {
    fn clone(&self) -> Self {
        Self {
            command: self.command,
            args: self.args,
            env: self.env.clone(),
            plugin_data: self.plugin_data.clone(),
            continue_execution: self.continue_execution,
            error: self.error.clone(),
        }
    }
}

/// 钩子返回结果
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookResult {
    /// 修改后的环境变量
    pub modified_env: HashMap<String, String>,
    /// 插件数据更新
    pub plugin_data: HashMap<String, String>,
    /// 是否继续执行
    pub continue_execution: bool,
    /// 消息 (用于日志)
    pub message: Option<String>,
}

/// 钩子优先级
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HookPriority(u8);

impl HookPriority {
    pub const CRITICAL: HookPriority = HookPriority(10);
    pub const HIGH: HookPriority = HookPriority(50);
    pub const NORMAL: HookPriority = HookPriority(100);
    pub const LOW: HookPriority = HookPriority(150);
    pub const BACKGROUND: HookPriority = HookPriority(200);

    pub fn new(value: u8) -> Self {
        HookPriority(value)
    }

    pub fn value(&self) -> u8 {
        self.0
    }
}

impl Default for HookPriority {
    fn default() -> Self {
        HookPriority::NORMAL
    }
}

/// 插件错误类型
#[derive(Error, Debug)]
pub enum PluginError {
    #[error("插件未找到: {0}")]
    NotFound(String),

    #[error("插件加载失败: {0}")]
    LoadFailed(String),

    #[error("插件执行失败: {0}")]
    ExecutionFailed(String),

    #[error("插件配置错误: {0}")]
    ConfigError(String),

    #[error("插件依赖缺失: {0}")]
    DependencyMissing(String),

    #[error("插件不兼容: {0}")]
    Incompatible(String),

    #[error("超时错误: {0}")]
    Timeout(String),

    #[error("插件已存在: {0}")]
    AlreadyExists(String),

    #[error("不支持的操作: {0}")]
    Unsupported(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON 序列化错误: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML 解析错误: {0}")]
    Toml(#[from] toml::de::Error),
}

/// 插件状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStatus {
    pub plugin_id: String,
    pub enabled: bool,
    pub loaded: bool,
    pub last_error: Option<String>,
    pub execution_count: u64,
    pub error_count: u64,
    pub last_execution: Option<u64>,
}

/// 插件注册项
pub struct PluginRegistration {
    pub metadata: PluginMetadata,
    pub config: PluginConfig,
    pub status: PluginStatus,
}

/// 插件查询结果
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub metadata: PluginMetadata,
    pub config: PluginConfig,
    pub status: PluginStatus,
}

/// 插件响应格式 (用于外部插件)
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginResponse {
    pub success: bool,
    pub result: Option<HookResult>,
    pub metadata: Option<PluginMetadata>,
    pub error: Option<String>,
}

/// 插件请求格式 (用于外部插件)
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginRequest {
    pub action: String,
    pub hook_type: Option<HookType>,
    pub context: Option<HookContextStatic>,
    pub config: Option<PluginConfig>,
}

/// 静态版本的 HookContext (用于序列化)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookContextStatic {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub plugin_data: HashMap<String, String>,
    pub continue_execution: bool,
    pub error: Option<String>,
}

impl<'a> From<&'a HookContext<'a>> for HookContextStatic {
    fn from(ctx: &'a HookContext<'a>) -> Self {
        Self {
            command: ctx.command.to_string(),
            args: ctx.args.to_vec(),
            env: ctx.env.clone(),
            plugin_data: ctx.plugin_data.clone(),
            continue_execution: ctx.continue_execution,
            error: ctx.error.clone(),
        }
    }
}

/// 插件工厂函数签名 (动态库)
///
/// 注意: 这个类型定义在 FFI 中使用 trait objects，这不是完全 FFI-safe 的，
/// 但在实际使用中，我们通过裸指针传递，所以是安全的
#[allow(improper_ctypes_definitions)]
pub type CreatePluginFn = extern "C" fn() -> *mut dyn Plugin;

/// 插件 Trait (动态库插件实现)
pub trait Plugin: Send + Sync {
    /// 获取插件元数据
    fn metadata(&self) -> PluginMetadata;

    /// 初始化插件
    fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError>;

    /// 执行钩子
    fn execute_hook(
        &self,
        hook_type: HookType,
        context: &HookContext,
    ) -> Result<HookResult, PluginError>;

    /// 检查是否支持扩展点
    fn supports_extension(&self, extension: ExtensionPoint) -> bool;

    /// 执行扩展功能
    fn execute_extension(
        &self,
        extension: ExtensionPoint,
        input: &[u8],
    ) -> Result<Vec<u8>, PluginError>;

    /// 清理资源
    fn shutdown(&mut self) -> Result<(), PluginError>;
}

/// 钩子处理器 Trait
pub trait HookHandler: Send + Sync {
    fn handle_pre_command(&self, context: &HookContext) -> Result<HookResult, PluginError>;
    fn handle_post_command(&self, context: &HookContext) -> Result<HookResult, PluginError>;
    fn handle_error(&self, context: &HookContext) -> Result<HookResult, PluginError>;
}

/// 插件加载器 Trait
pub trait PluginLoader: Send + Sync {
    /// 加载插件
    fn load(&self, path: &Path, config: PluginConfig) -> Result<Box<dyn Plugin>, PluginError>;

    /// 卸载插件
    fn unload(&self, plugin: &mut dyn Plugin) -> Result<(), PluginError>;

    /// 获取支持的插件类型
    fn supported_types(&self) -> Vec<PluginType>;
}

/// 插件管理器 Trait (用于扩展)
pub trait PluginManagerExt {
    /// 注册插件
    fn register(
        &mut self,
        plugin: Box<dyn Plugin>,
        config: PluginConfig,
    ) -> Result<(), PluginError>;

    /// 注销插件
    fn unregister(&mut self, plugin_id: &str) -> Result<(), PluginError>;

    /// 执行钩子链
    fn execute_hooks(
        &self,
        hook_type: HookType,
        context: &HookContext,
    ) -> Result<Vec<HookResult>, PluginError>;

    /// 获取插件信息
    fn get_plugin_info(&self, plugin_id: &str) -> Option<PluginInfo>;

    /// 列出所有插件
    fn list_plugins(&self, include_disabled: bool) -> Vec<PluginInfo>;
}

/// 兼容性问题类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompatibilityIssue {
    /// EnvCLI 版本不匹配
    EnvCliVersionMismatch { required: String, current: String },
    /// 平台不兼容
    PlatformMismatch {
        required: Vec<Platform>,
        current: Platform,
    },
    /// 无效的版本格式
    InvalidVersionFormat { version: String, reason: String },
    /// 缺少依赖
    MissingDependency { dependency: String },
}

impl std::fmt::Display for CompatibilityIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompatibilityIssue::EnvCliVersionMismatch { required, current } => {
                write!(f, "EnvCLI 版本不兼容: 需要 {}, 当前 {}", required, current)
            }
            CompatibilityIssue::PlatformMismatch { required, current } => {
                write!(f, "平台不兼容: 需要 {:?}, 当前 {:?}", required, current)
            }
            CompatibilityIssue::InvalidVersionFormat { version, reason } => {
                write!(f, "版本格式无效: {} ({})", version, reason)
            }
            CompatibilityIssue::MissingDependency { dependency } => {
                write!(f, "缺少依赖: {}", dependency)
            }
        }
    }
}

/// 兼容性检查报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityReport {
    /// 插件ID
    pub plugin_id: String,
    /// 是否兼容
    pub compatible: bool,
    /// 问题列表
    pub issues: Vec<CompatibilityIssue>,
}

impl CompatibilityReport {
    pub fn new(plugin_id: String) -> Self {
        Self {
            plugin_id,
            compatible: true,
            issues: Vec::new(),
        }
    }

    pub fn add_issue(&mut self, issue: CompatibilityIssue) {
        self.compatible = false;
        self.issues.push(issue);
    }

    pub fn is_compatible(&self) -> bool {
        self.compatible && self.issues.is_empty()
    }
}
