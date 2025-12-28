//! EnvCLI 插件系统示例插件
//!
//! 这是一个完整的示例插件，展示了如何实现 EnvCLI 插件接口。
//!
//! 编译为动态库：
//! ```bash
//! # 方法1: 使用 rustc
//! rustc --crate-type dylib example_plugin.rs -o example_plugin.dll
//!
//! # 方法2: 使用 cargo (推荐)
//! # 1. 创建新项目: cargo new --lib example-plugin
//! # 2. 复制此代码到 src/lib.rs
//! # 3. 在 Cargo.toml 添加:
//! #    [lib]
//! #    crate-type = ["dylib"]
//! #    [dependencies]
//! #    envcli = { path = "../envcli" }
//! # 4. 编译: cargo build --release
//! # 5. 加载: envcli plugin load ./target/release/example_plugin.dll
//!
//! 使用方法：
//! envcli plugin load ./example_plugin.dll
//! envcli plugin test example-plugin
//! envcli plugin config set example-plugin timeout 30

use std::collections::HashMap;

// 重新导出 EnvCLI 插件类型
pub use envcli::plugin::{
    HookContext, HookPriority, HookResult, HookType, Plugin, PluginConfig, PluginError,
    PluginMetadata, PluginType, ExtensionPoint, Platform,
};

/// 示例插件结构体
#[derive(Clone)]
pub struct ExamplePlugin {
    metadata: PluginMetadata,
    config: PluginConfig,
    execution_count: u64,
}

impl ExamplePlugin {
    /// 创建新插件实例
    pub fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                id: "example-plugin".to_string(),
                name: "Example Plugin".to_string(),
                version: "1.0.0".to_string(),
                description: Some("EnvCLI 插件系统示例插件".to_string()),
                author: Some("EnvCLI Team".to_string()),
                plugin_type: PluginType::DynamicLibrary,
                hooks: vec![
                    HookType::PreCommand,
                    HookType::PostCommand,
                    HookType::PreRun,
                    HookType::PostRun,
                ],
                extensions: vec![],
                config_schema: None,
                enabled: true,
                dependencies: vec![],
                platforms: vec![],
                envcli_version: None,
            },
            config: PluginConfig::default(),
            execution_count: 0,
        }
    }

    /// 处理命令执行前钩子
    fn handle_pre_command(&self, context: &HookContext) -> Result<HookResult, PluginError> {
        println!("[ExamplePlugin] PreCommand: command={}, args={:?}", context.command, context.args);

        // 注入环境变量
        let mut modified_env = HashMap::new();
        modified_env.insert("EXAMPLE_PLUGIN_PRE".to_string(), "active".to_string());
        modified_env.insert("COMMAND_NAME".to_string(), context.command.to_string());

        Ok(HookResult {
            modified_env,
            plugin_data: HashMap::new(),
            continue_execution: true,
            message: Some("Pre-command hook executed".to_string()),
        })
    }

    /// 处理命令执行后钩子
    fn handle_post_command(&self, context: &HookContext) -> Result<HookResult, PluginError> {
        println!("[ExamplePlugin] PostCommand: command={}", context.command);

        let mut plugin_data = HashMap::new();
        plugin_data.insert("last_command".to_string(), context.command.to_string());
        plugin_data.insert("execution_time".to_string(), "0ms".to_string());

        Ok(HookResult {
            modified_env: HashMap::new(),
            plugin_data,
            continue_execution: true,
            message: Some("Post-command hook executed".to_string()),
        })
    }

    /// 处理运行前钩子
    fn handle_pre_run(&self, context: &HookContext) -> Result<HookResult, PluginError> {
        println!("[ExamplePlugin] PreRun: command={}", context.command);

        let mut modified_env = HashMap::new();
        modified_env.insert("RUN_MODE".to_string(), "example".to_string());
        modified_env.insert("EXAMPLE_VERSION".to_string(), "1.0.0".to_string());

        Ok(HookResult {
            modified_env,
            plugin_data: HashMap::new(),
            continue_execution: true,
            message: Some("Pre-run hook executed".to_string()),
        })
    }

    /// 处理运行后钩子
    fn handle_post_run(&self, context: &HookContext) -> Result<HookResult, PluginError> {
        println!("[ExamplePlugin] PostRun: command={}", context.command);

        Ok(HookResult {
            modified_env: HashMap::new(),
            plugin_data: HashMap::new(),
            continue_execution: true,
            message: Some("Post-run hook executed".to_string()),
        })
    }
}

/// 实现 Plugin trait
impl Plugin for ExamplePlugin {
    /// 获取插件元数据
    fn metadata(&self) -> PluginMetadata {
        self.metadata.clone()
    }

    /// 初始化插件
    fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        self.config = config.clone();
        println!("[ExamplePlugin] Initialized with config: {:?}", config);
        Ok(())
    }

    /// 执行钩子
    fn execute_hook(&self, hook_type: HookType, context: &HookContext) -> Result<HookResult, PluginError> {
        match hook_type {
            HookType::PreCommand => self.handle_pre_command(context),
            HookType::PostCommand => self.handle_post_command(context),
            HookType::PreRun => self.handle_pre_run(context),
            HookType::PostRun => self.handle_post_run(context),
            _ => Ok(HookResult::default()),
        }
    }

    /// 检查是否支持扩展点
    fn supports_extension(&self, _extension: envcli::plugin::ExtensionPoint) -> bool {
        false
    }

    /// 执行扩展功能
    fn execute_extension(
        &self,
        _extension: envcli::plugin::ExtensionPoint,
        _input: &[u8],
    ) -> Result<Vec<u8>, PluginError> {
        Err(PluginError::Unsupported("No extensions supported".to_string()))
    }

    /// 清理资源
    fn shutdown(&mut self) -> Result<(), PluginError> {
        println!("[ExamplePlugin] Shutdown, total executions: {}", self.execution_count);
        Ok(())
    }
}

/// 工厂函数 - 用于动态库加载
///
/// # Safety
/// 这个函数是 FFI 接口，必须按照约定实现
#[no_mangle]
pub extern "C" fn create_plugin() -> *mut dyn Plugin {
    let plugin = Box::new(ExamplePlugin::new());
    Box::into_raw(plugin)
}

/// 销毁函数 - 用于动态库卸载
///
/// # Safety
/// 这个函数是 FFI 接口，必须按照约定实现
#[no_mangle]
pub extern "C" fn destroy_plugin(plugin: *mut dyn Plugin) {
    unsafe {
        if !plugin.is_null() {
            // 重新获取所有权并自动释放
            let _ = Box::from_raw(plugin);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_metadata() {
        let plugin = ExamplePlugin::new();
        let metadata = plugin.metadata();
        assert_eq!(metadata.id, "example-plugin");
        assert_eq!(metadata.name, "Example Plugin");
        assert_eq!(metadata.hooks.len(), 4);
    }

    #[test]
    fn test_pre_command_hook() {
        let plugin = ExamplePlugin::new();
        let context = HookContext {
            command: "test",
            args: &["arg1".to_string()],
            env: HashMap::new(),
            plugin_data: HashMap::new(),
            continue_execution: true,
            error: None,
        };

        let result = plugin.execute_hook(HookType::PreCommand, &context).unwrap();
        assert!(result.modified_env.contains_key("EXAMPLE_PLUGIN_PRE"));
        assert!(result.continue_execution);
    }

    #[test]
    fn test_initialize() {
        let mut plugin = ExamplePlugin::new();
        let config = PluginConfig::default();
        let result = plugin.initialize(&config);
        assert!(result.is_ok());
    }
}
