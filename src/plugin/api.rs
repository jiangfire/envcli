//! 插件 API 接口定义
//!
//! 提供插件系统的核心 API 和辅助宏

use crate::plugin::types::{HookContext, HookResult, PluginError};

/// 简化插件实现的辅助宏
///
/// # 使用示例
///
/// ```rust,ignore
/// use envcli::{define_plugin, plugin::{Plugin, PluginMetadata, HookType, HookContext, HookResult}};
///
/// define_plugin!(
///     name = "MyPlugin",
///     version = "1.0.0",
///     hooks = [HookType::PreCommand],
///     on_pre_command = |_context: &HookContext| {
///         // 处理逻辑
///         Ok(HookResult::default())
///     },
/// );
/// ```
#[macro_export]
macro_rules! define_plugin {
    (
        name = $name:expr,
        version = $version:expr,
        hooks = [$($hook:expr),*],
        $(on_pre_command = $pre_fn:expr,)*
        $(on_post_command = $post_fn:expr,)*
        $(on_error = $error_fn:expr,)*
    ) => {
        use $crate::plugin::types::{Plugin, PluginMetadata, HookType, HookContext, HookResult, PluginConfig, PluginError, ExtensionPoint};

        #[derive(Default)]
        pub struct AutoPlugin {
            config: PluginConfig,
        }

        impl Plugin for AutoPlugin {
            fn metadata(&self) -> PluginMetadata {
                PluginMetadata {
                    id: $name.to_string().to_lowercase().replace(" ", "-"),
                    name: $name.to_string(),
                    version: $version.to_string(),
                    description: Some("Auto-generated plugin".to_string()),
                    author: None,
                    plugin_type: $crate::plugin::types::PluginType::DynamicLibrary,
                    hooks: vec![$($hook),*],
                    extensions: vec![],
                    config_schema: None,
                    enabled: true,
                    dependencies: vec![],
                    platforms: vec![$crate::plugin::types::Platform::current()],
                    envcli_version: None,
                    signature: None,
                }
            }

            fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
                self.config = config.clone();
                Ok(())
            }

            fn execute_hook(&self, hook_type: HookType, context: &HookContext) -> Result<HookResult, PluginError> {
                match hook_type {
                    $(HookType::PreCommand => {
                        let f: fn(&HookContext) -> Result<HookResult, PluginError> = $pre_fn;
                        f(context)
                    })*
                    $(HookType::PostCommand => {
                        let f: fn(&HookContext) -> Result<HookResult, PluginError> = $post_fn;
                        f(context)
                    })*
                    $(HookType::Error => {
                        let f: fn(&HookContext) -> Result<HookResult, PluginError> = $error_fn;
                        f(context)
                    })*
                    _ => Ok(HookResult::default()),
                }
            }

            fn supports_extension(&self, _extension: ExtensionPoint) -> bool {
                false
            }

            fn execute_extension(&self, _extension: ExtensionPoint, _input: &[u8]) -> Result<Vec<u8>, PluginError> {
                Err(PluginError::Unsupported("No extensions".to_string()))
            }

            fn shutdown(&mut self) -> Result<(), PluginError> {
                Ok(())
            }
        }

        // 导出工厂函数
        // 注意：虽然 *mut dyn Plugin 在技术上不是 FFI-safe，但这是 Rust 插件系统的标准做法
        // 插件通过 trait 对象的虚表进行调用，宿主和插件在同一 Rust 版本下编译时是安全的
        #[allow(improper_ctypes_definitions)]
        #[unsafe(no_mangle)]
        pub extern "C" fn create_plugin() -> *mut dyn Plugin {
            let plugin = Box::new(AutoPlugin::default());
            Box::into_raw(plugin)
        }

        #[allow(improper_ctypes_definitions)]
        #[unsafe(no_mangle)]
        pub extern "C" fn destroy_plugin(ptr: *mut dyn Plugin) {
            if !ptr.is_null() {
                unsafe {
                    let _ = Box::from_raw(ptr);
                }
            }
        }
    };
}

/// 插件 SDK 版本
pub const PLUGIN_SDK_VERSION: &str = "0.2.0";

/// 插件开发辅助函数
pub mod helpers {
    use super::*;
    use std::collections::HashMap;

    /// 创建成功的钩子结果
    pub fn ok_result() -> Result<HookResult, PluginError> {
        Ok(HookResult::default())
    }

    /// 创建带消息的成功结果
    pub fn ok_with_message(msg: &str) -> Result<HookResult, PluginError> {
        Ok(HookResult {
            message: Some(msg.to_string()),
            ..Default::default()
        })
    }

    /// 创建修改环境变量的结果
    pub fn with_env(key: &str, value: &str) -> Result<HookResult, PluginError> {
        let mut env = HashMap::new();
        env.insert(key.to_string(), value.to_string());
        Ok(HookResult {
            modified_env: env,
            ..Default::default()
        })
    }

    /// 创建阻止执行的结果
    pub fn block_execution(reason: &str) -> Result<HookResult, PluginError> {
        Ok(HookResult {
            continue_execution: false,
            message: Some(reason.to_string()),
            ..Default::default()
        })
    }

    /// 创建错误结果
    pub fn error(msg: &str) -> Result<HookResult, PluginError> {
        Err(PluginError::ExecutionFailed(msg.to_string()))
    }

    /// 从上下文获取环境变量
    pub fn get_env<'a>(context: &'a HookContext, key: &str) -> Option<&'a String> {
        context.env.get(key)
    }

    /// 从上下文获取参数
    pub fn get_arg<'a>(context: &'a HookContext, index: usize) -> Option<&'a String> {
        context.args.get(index)
    }

    /// 检查上下文是否有错误
    pub fn has_error(context: &HookContext) -> bool {
        context.error.is_some()
    }

    /// 获取错误信息
    pub fn get_error<'a>(context: &'a HookContext) -> Option<&'a String> {
        context.error.as_ref()
    }
}

/// 插件测试辅助工具
#[cfg(test)]
pub mod test_helpers {
    use super::*;
    use crate::PluginConfig;
    use std::collections::HashMap;

    /// 创建测试上下文
    pub fn create_test_context<'a>(
        command: &'a str,
        args: &'a [String],
        env: HashMap<String, String>,
    ) -> HookContext<'a> {
        HookContext {
            command,
            args,
            env,
            plugin_data: HashMap::new(),
            continue_execution: true,
            error: None,
        }
    }

    /// 创建测试配置
    pub fn create_test_config(plugin_id: &str) -> PluginConfig {
        PluginConfig {
            plugin_id: plugin_id.to_string(),
            enabled: true,
            settings: HashMap::new(),
            path: None,
            timeout: Some(30),
            env: HashMap::new(),
        }
    }

    /// 验证钩子结果
    pub fn assert_valid_result(result: &Result<HookResult, PluginError>) {
        assert!(result.is_ok(), "Hook execution failed: {:?}", result);
        let result = result.as_ref().unwrap();
        assert!(result.continue_execution, "Should continue execution");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_define_plugin_macro() {
        define_plugin!(
            name = "TestPlugin",
            version = "1.0.0",
            hooks = [HookType::PreCommand],
            on_pre_command = |_context: &HookContext| { Ok(HookResult::default()) },
        );

        let plugin = AutoPlugin::default();
        let metadata = plugin.metadata();

        assert_eq!(metadata.name, "TestPlugin");
        assert_eq!(metadata.version, "1.0.0");
        assert!(metadata.hooks.contains(&HookType::PreCommand));
    }

    #[test]
    fn test_helper_functions() {
        // ok_result
        let result = helpers::ok_result();
        assert!(result.is_ok());

        // ok_with_message
        let result = helpers::ok_with_message("test");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().message, Some("test".to_string()));

        // with_env
        let result = helpers::with_env("KEY", "value");
        assert!(result.is_ok());
        let env = &result.unwrap().modified_env;
        assert_eq!(env.get("KEY"), Some(&"value".to_string()));

        // block_execution
        let result = helpers::block_execution("blocked");
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(!result.continue_execution);
        assert_eq!(result.message, Some("blocked".to_string()));

        // error
        let result = helpers::error("failed");
        assert!(result.is_err());
    }

    #[test]
    fn test_context_helpers() {
        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "test_value".to_string());

        let context = HookContext {
            command: "run",
            args: &["arg1".to_string(), "arg2".to_string()],
            env,
            plugin_data: HashMap::new(),
            continue_execution: true,
            error: None,
        };

        // get_env
        assert_eq!(
            helpers::get_env(&context, "TEST_VAR"),
            Some(&"test_value".to_string())
        );
        assert_eq!(helpers::get_env(&context, "MISSING"), None);

        // get_arg
        assert_eq!(helpers::get_arg(&context, 0), Some(&"arg1".to_string()));
        assert_eq!(helpers::get_arg(&context, 1), Some(&"arg2".to_string()));
        assert_eq!(helpers::get_arg(&context, 99), None);

        // has_error
        assert!(!helpers::has_error(&context));

        // get_error
        assert_eq!(helpers::get_error(&context), None);
    }

    #[test]
    fn test_test_helpers() {
        let env = HashMap::new();
        let context = test_helpers::create_test_context("run", &[], env);
        assert_eq!(context.command, "run");

        let config = test_helpers::create_test_config("test");
        assert_eq!(config.plugin_id, "test");
        assert!(config.enabled);

        // 创建一个有效的钩子结果（continue_execution = true）
        let result = Ok(HookResult {
            continue_execution: true,
            ..Default::default()
        });
        test_helpers::assert_valid_result(&result);
    }

    #[test]
    fn test_plugin_sdk_version() {
        assert_eq!(PLUGIN_SDK_VERSION, "0.2.0");
    }
}
