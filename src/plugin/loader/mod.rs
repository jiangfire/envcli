//! 插件加载器模块
//!
//! 提供不同类型的插件加载器

pub mod dynamic;
pub mod executable;

use crate::plugin::types::{Plugin, PluginConfig, PluginError, PluginType};
use std::path::Path;

/// 插件加载器工厂
pub struct LoaderFactory;

impl LoaderFactory {
    /// 根据插件类型创建合适的加载器
    pub fn get_loader(plugin_type: PluginType) -> Box<dyn PluginLoader> {
        match plugin_type {
            PluginType::DynamicLibrary => Box::new(dynamic::DynamicLibraryLoader),
            PluginType::ExternalExecutable => Box::new(executable::ExecutableLoader),
            PluginType::Wasm => unimplemented!("WASM 插件支持尚未实现"),
        }
    }

    /// 自动检测插件类型
    pub fn detect_type(path: &Path) -> Result<PluginType, PluginError> {
        let extension: &str = path
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| PluginError::LoadFailed("无法检测插件扩展名".to_string()))?;

        match extension.to_lowercase().as_str() {
            "so" | "dll" | "dylib" => Ok(PluginType::DynamicLibrary),
            "exe" | "bin" | "py" | "sh" | "js" | "ts" => Ok(PluginType::ExternalExecutable),
            "wasm" => Ok(PluginType::Wasm),
            _ => Err(PluginError::LoadFailed(format!(
                "不支持的插件类型: {}",
                extension
            ))),
        }
    }

    /// 加载插件（自动检测类型）
    #[allow(clippy::ptr_arg)]
    pub fn load_auto(path: &Path, config: PluginConfig) -> Result<Box<dyn Plugin>, PluginError> {
        let plugin_type = Self::detect_type(path)?;
        let loader = Self::get_loader(plugin_type);
        loader.load(path, config)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_loader_factory_detect_type() {
        // 测试动态库
        let path = PathBuf::from("/test/plugin.so");
        assert_eq!(
            LoaderFactory::detect_type(&path).unwrap(),
            PluginType::DynamicLibrary
        );

        let path = PathBuf::from("/test/plugin.dll");
        assert_eq!(
            LoaderFactory::detect_type(&path).unwrap(),
            PluginType::DynamicLibrary
        );

        // 测试可执行文件
        let path = PathBuf::from("/test/plugin.exe");
        assert_eq!(
            LoaderFactory::detect_type(&path).unwrap(),
            PluginType::ExternalExecutable
        );

        let path = PathBuf::from("/test/plugin.py");
        assert_eq!(
            LoaderFactory::detect_type(&path).unwrap(),
            PluginType::ExternalExecutable
        );

        // 测试 WASM
        let path = PathBuf::from("/test/plugin.wasm");
        assert_eq!(LoaderFactory::detect_type(&path).unwrap(), PluginType::Wasm);
    }

    #[test]
    fn test_loader_factory_get_loader() {
        let loader = LoaderFactory::get_loader(PluginType::DynamicLibrary);
        assert!(
            loader
                .supported_types()
                .contains(&PluginType::DynamicLibrary)
        );

        let loader = LoaderFactory::get_loader(PluginType::ExternalExecutable);
        assert!(
            loader
                .supported_types()
                .contains(&PluginType::ExternalExecutable)
        );
    }
}
