//! 动态库插件加载器
//!
//! 加载 Rust 动态库插件 (.so/.dll/.dylib)

use crate::plugin::loader::PluginLoader;
use crate::plugin::types::{CreatePluginFn, Plugin, PluginConfig, PluginError, PluginType};
use libloading::{Library, Symbol};
use std::path::Path;
use std::sync::Arc;

/// 动态库插件包装器
///
/// 注意：使用 Arc<Library> 确保库引用计数正确管理。
/// 当所有插件实例和钩子引用都被释放后，库才会被卸载。
pub struct DynamicLibraryPlugin {
    plugin: Box<dyn Plugin>,
    library: Arc<Library>, // 保持库引用，防止过早卸载
}

impl DynamicLibraryPlugin {
    /// 获取库引用计数
    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.library)
    }

    /// 获取库弱引用计数
    pub fn weak_count(&self) -> usize {
        Arc::weak_count(&self.library)
    }
}

impl Plugin for DynamicLibraryPlugin {
    fn metadata(&self) -> crate::plugin::types::PluginMetadata {
        self.plugin.metadata()
    }

    fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        self.plugin.initialize(config)
    }

    fn execute_hook(
        &self,
        hook_type: crate::plugin::types::HookType,
        context: &crate::plugin::types::HookContext,
    ) -> Result<crate::plugin::types::HookResult, PluginError> {
        self.plugin.execute_hook(hook_type, context)
    }

    fn supports_extension(&self, extension: crate::plugin::types::ExtensionPoint) -> bool {
        self.plugin.supports_extension(extension)
    }

    fn execute_extension(
        &self,
        extension: crate::plugin::types::ExtensionPoint,
        input: &[u8],
    ) -> Result<Vec<u8>, PluginError> {
        self.plugin.execute_extension(extension, input)
    }

    fn shutdown(&mut self) -> Result<(), PluginError> {
        self.plugin.shutdown()
    }
}

/// 动态库加载器
pub struct DynamicLibraryLoader;

impl PluginLoader for DynamicLibraryLoader {
    fn load(&self, path: &Path, _config: PluginConfig) -> Result<Box<dyn Plugin>, PluginError> {
        // 验证文件存在
        if !path.exists() {
            return Err(PluginError::LoadFailed(format!(
                "动态库文件不存在: {}",
                path.display()
            )));
        }

        // 加载动态库（需要 unsafe）
        let library = unsafe {
            Library::new(path).map_err(|e| {
                PluginError::LoadFailed(format!("无法加载动态库 {}: {}", path.display(), e))
            })?
        };

        // 获取创建函数
        let create_fn: Symbol<CreatePluginFn> = unsafe {
            library
                .get(b"create_plugin")
                .map_err(|e| PluginError::LoadFailed(format!("找不到 create_plugin 函数: {}", e)))?
        };

        // 调用创建函数获取插件
        let plugin_ptr = create_fn();
        let plugin = unsafe { Box::from_raw(plugin_ptr) };

        // 包装插件以保持库引用
        // 使用 Arc 确保库引用计数正确，当所有引用释放后才会卸载
        let library_arc = Arc::new(library);

        // 创建包装器
        let wrapped = Box::new(DynamicLibraryPlugin {
            plugin,
            library: library_arc.clone(),
        });

        Ok(wrapped)
    }

    fn unload(&self, plugin: &mut dyn Plugin) -> Result<(), PluginError> {
        // 调用插件的 shutdown
        plugin.shutdown()
    }

    fn supported_types(&self) -> Vec<PluginType> {
        vec![PluginType::DynamicLibrary]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // 注意：这些测试需要实际的动态库文件
    // 在实际环境中，需要编译测试插件

    #[test]
    fn test_dynamic_loader_supported_types() {
        let loader = DynamicLibraryLoader;
        let types = loader.supported_types();
        assert_eq!(types.len(), 1);
        assert!(types.contains(&PluginType::DynamicLibrary));
    }

    #[test]
    fn test_dynamic_loader_load_nonexistent() {
        let loader = DynamicLibraryLoader;
        let path = PathBuf::from("/nonexistent/plugin.so");
        let config = PluginConfig::default();

        let result = loader.load(&path, config);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(matches!(err, PluginError::LoadFailed(_)));
    }
}
