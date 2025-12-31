//! 插件配置管理
//!
//! 负责插件配置的加载、验证和持久化

use crate::error::{EnvError, Result};
use crate::plugin::types::{PluginConfig, PluginMetadata};
use crate::utils::paths::{self, file_exists};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

/// 插件全局配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginGlobalConfig {
    /// 全局设置
    pub global: GlobalSettings,
    /// 插件配置列表
    pub plugins: Vec<PluginConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    /// 日志级别
    pub log_level: String,
    /// 默认超时（秒）
    pub default_timeout: u64,
    /// 是否启用沙箱
    pub enable_sandbox: bool,
    /// 插件目录路径
    pub plugin_dir: Option<String>,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            default_timeout: 30,
            enable_sandbox: false,
            plugin_dir: None,
        }
    }
}

/// 插件配置管理器（线程安全）
pub struct PluginConfigManager {
    config_path: PathBuf,
    global_config: Arc<RwLock<PluginGlobalConfig>>,
    /// 沙箱基础路径（用于限制插件路径范围）
    sandbox_base: Option<PathBuf>,
}

impl PluginConfigManager {
    /// 创建配置管理器（从默认配置文件加载）
    pub fn new() -> Result<Self> {
        let config_path = paths::get_plugin_config_path()?;
        Self::load_from_file(&config_path)
    }

    /// 从文件加载配置
    pub fn load_from_file(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self {
                config_path: path.to_path_buf(),
                global_config: Arc::new(RwLock::new(PluginGlobalConfig::default())),
                sandbox_base: None,
            });
        }

        let content = paths::read_file(path)?;
        let global_config: PluginGlobalConfig =
            toml::from_str(&content).map_err(|e| EnvError::PluginConfigError(e.to_string()))?;

        Ok(Self {
            config_path: path.to_path_buf(),
            global_config: Arc::new(RwLock::new(global_config)),
            sandbox_base: None,
        })
    }

    /// 创建空的配置管理器（用于测试或降级）
    pub fn empty() -> Self {
        Self {
            config_path: PathBuf::new(),
            global_config: Arc::new(RwLock::new(PluginGlobalConfig::default())),
            sandbox_base: None,
        }
    }

    /// 设置沙箱基础路径
    pub fn set_sandbox_base(&mut self, base_path: PathBuf) -> Result<()> {
        // 验证路径存在且是目录
        if !base_path.exists() {
            return Err(EnvError::PluginConfigError(format!(
                "沙箱基础路径不存在: {}",
                base_path.display()
            )));
        }
        if !base_path.is_dir() {
            return Err(EnvError::PluginConfigError(format!(
                "沙箱基础路径必须是目录: {}",
                base_path.display()
            )));
        }

        // 标准化路径
        let canonical_base = base_path.canonicalize().map_err(|e| {
            EnvError::PluginConfigError(format!("无法解析沙箱路径 {}: {}", base_path.display(), e))
        })?;

        self.sandbox_base = Some(canonical_base);
        Ok(())
    }

    /// 获取沙箱基础路径
    pub fn get_sandbox_base(&self) -> Option<&PathBuf> {
        self.sandbox_base.as_ref()
    }

    /// 验证路径是否在沙箱范围内
    fn validate_path_sandbox(&self, path: &Path) -> Result<()> {
        // 如果没有设置沙箱，跳过验证
        let sandbox_base = match &self.sandbox_base {
            Some(base) => base,
            None => return Ok(()),
        };

        // 标准化路径
        let canonical_path = path.canonicalize().map_err(|e| {
            EnvError::PluginConfigError(format!("无法解析路径 {}: {}", path.display(), e))
        })?;

        // 检查是否在沙箱基础路径下
        if !canonical_path.starts_with(sandbox_base) {
            return Err(EnvError::PluginConfigError(format!(
                "插件路径 {} 不在允许的沙箱目录 {} 下",
                canonical_path.display(),
                sandbox_base.display()
            )));
        }

        Ok(())
    }

    /// 保存配置到文件
    pub fn save(&self) -> Result<()> {
        if self.config_path.as_os_str().is_empty() {
            return Ok(()); // 没有配置路径，不保存
        }

        let content = toml::to_string_pretty(&*self.global_config.read().unwrap())
            .map_err(|e| EnvError::PluginConfigError(e.to_string()))?;

        paths::write_file_safe(&self.config_path, &content)?;
        Ok(())
    }

    /// 获取全局设置（可变引用，用于修改）
    pub fn get_global_settings_mut(
        &mut self,
    ) -> std::sync::RwLockWriteGuard<'_, PluginGlobalConfig> {
        self.global_config.write().unwrap()
    }

    /// 获取全局设置
    pub fn get_global_settings(&self) -> std::sync::RwLockReadGuard<'_, PluginGlobalConfig> {
        self.global_config.read().unwrap()
    }

    /// 修改全局设置
    pub fn update_global_settings(&mut self, settings: GlobalSettings) -> Result<()> {
        self.global_config.write().unwrap().global = settings;
        self.save()
    }

    /// 获取单个插件配置
    pub fn get_plugin_config(&self, plugin_id: &str) -> Option<PluginConfig> {
        self.global_config
            .read()
            .unwrap()
            .plugins
            .iter()
            .find(|p| p.plugin_id == plugin_id)
            .cloned()
    }

    /// 获取可变插件配置引用
    /// 注意：由于使用 RwLock，此方法返回整个配置的写锁保护器
    /// 调用者需要通过 guard.plugins 访问具体插件
    pub fn get_plugin_config_mut(
        &mut self,
        plugin_id: &str,
    ) -> Option<std::sync::RwLockWriteGuard<'_, PluginGlobalConfig>> {
        let guard = self.global_config.write().unwrap();
        if guard.plugins.iter().any(|p| p.plugin_id == plugin_id) {
            Some(guard)
        } else {
            None
        }
    }

    /// 获取或创建插件配置
    pub fn get_or_create_plugin_config(&mut self, plugin_id: &str) -> PluginConfig {
        if let Some(config) = self.get_plugin_config(plugin_id) {
            config
        } else {
            let default_timeout = self.global_config.read().unwrap().global.default_timeout;
            let config = PluginConfig {
                plugin_id: plugin_id.to_string(),
                enabled: true,
                settings: HashMap::new(),
                path: None,
                timeout: Some(default_timeout),
                env: HashMap::new(),
            };
            self.global_config
                .write()
                .unwrap()
                .plugins
                .push(config.clone());
            config
        }
    }

    /// 设置插件配置
    pub fn set_plugin_config(&mut self, config: PluginConfig) -> Result<()> {
        // 查找并更新或添加
        let mut guard = self.global_config.write().unwrap();
        if let Some(index) = guard
            .plugins
            .iter()
            .position(|p| p.plugin_id == config.plugin_id)
        {
            guard.plugins[index] = config;
        } else {
            guard.plugins.push(config);
        }
        self.save()
    }

    /// 设置插件配置项
    pub fn set_plugin_setting(&mut self, plugin_id: &str, key: &str, value: &str) -> Result<()> {
        let mut config = self.get_or_create_plugin_config(plugin_id);
        config.settings.insert(key.to_string(), value.to_string());
        self.set_plugin_config(config)
    }

    /// 获取插件配置项
    pub fn get_plugin_setting(&self, plugin_id: &str, key: &str) -> Option<String> {
        self.get_plugin_config(plugin_id)?
            .settings
            .get(key)
            .cloned()
    }

    /// 启用插件
    pub fn enable_plugin(&mut self, plugin_id: &str) -> Result<()> {
        let mut guard = self.global_config.write().unwrap();
        if let Some(config) = guard.plugins.iter_mut().find(|p| p.plugin_id == plugin_id) {
            config.enabled = true;
            self.save()
        } else {
            Err(EnvError::PluginNotFound(plugin_id.to_string()))
        }
    }

    /// 禁用插件
    pub fn disable_plugin(&mut self, plugin_id: &str) -> Result<()> {
        let mut guard = self.global_config.write().unwrap();
        if let Some(config) = guard.plugins.iter_mut().find(|p| p.plugin_id == plugin_id) {
            config.enabled = false;
            self.save()
        } else {
            Err(EnvError::PluginNotFound(plugin_id.to_string()))
        }
    }

    /// 移除插件配置
    pub fn remove_plugin(&mut self, plugin_id: &str) -> Result<()> {
        let mut guard = self.global_config.write().unwrap();
        if let Some(index) = guard.plugins.iter().position(|p| p.plugin_id == plugin_id) {
            guard.plugins.remove(index);
            self.save()
        } else {
            Err(EnvError::PluginNotFound(plugin_id.to_string()))
        }
    }

    /// 列出所有插件配置
    pub fn list_plugins(&self) -> Vec<PluginConfig> {
        self.global_config.read().unwrap().plugins.clone()
    }

    /// 设置插件路径
    pub fn set_plugin_path(&mut self, plugin_id: &str, path: PathBuf) -> Result<()> {
        // 验证路径是否在沙箱范围内
        self.validate_path_sandbox(&path)?;

        let mut config = self.get_or_create_plugin_config(plugin_id);
        config.path = Some(path);
        self.set_plugin_config(config)
    }

    /// 设置插件超时
    pub fn set_plugin_timeout(&mut self, plugin_id: &str, timeout: u64) -> Result<()> {
        let mut config = self.get_or_create_plugin_config(plugin_id);
        config.timeout = Some(timeout);
        self.set_plugin_config(config)
    }

    /// 设置插件环境变量
    pub fn set_plugin_env(&mut self, plugin_id: &str, key: &str, value: &str) -> Result<()> {
        let mut config = self.get_or_create_plugin_config(plugin_id);
        config.env.insert(key.to_string(), value.to_string());
        self.set_plugin_config(config)
    }

    /// 验证插件配置
    pub fn validate_config(&self, config: &PluginConfig, metadata: &PluginMetadata) -> Result<()> {
        // 检查插件 ID 匹配
        if config.plugin_id != metadata.id {
            return Err(EnvError::PluginConfigError(format!(
                "配置的插件 ID '{}' 与元数据 ID '{}' 不匹配",
                config.plugin_id, metadata.id
            )));
        }

        // 检查路径（对于外部插件）
        if metadata.plugin_type == crate::plugin::types::PluginType::ExternalExecutable {
            if config.path.is_none() {
                return Err(EnvError::PluginConfigError(
                    "外部插件必须指定路径".to_string(),
                ));
            }
            if let Some(path) = &config.path {
                // 验证路径存在
                if !file_exists(path) {
                    return Err(EnvError::PluginConfigError(format!(
                        "插件路径不存在: {}",
                        path.display()
                    )));
                }
                // 验证路径沙箱
                self.validate_path_sandbox(path)?;
            }
        }

        // 验证配置模式（如果存在）
        if let Some(schema) = &metadata.config_schema {
            for field in &schema.fields {
                if field.required && !config.settings.contains_key(&field.name) {
                    return Err(EnvError::PluginConfigError(format!(
                        "缺少必需配置项: {}",
                        field.name
                    )));
                }
            }
        }

        Ok(())
    }

    /// 重置插件配置
    pub fn reset_plugin(&mut self, plugin_id: &str) -> Result<()> {
        let default_timeout = self.global_config.read().unwrap().global.default_timeout;
        let config = PluginConfig {
            plugin_id: plugin_id.to_string(),
            enabled: true,
            settings: HashMap::new(),
            path: None,
            timeout: Some(default_timeout),
            env: HashMap::new(),
        };
        self.set_plugin_config(config)
    }

    /// 设置配置项（别名，用于兼容）
    pub fn set(&mut self, plugin_id: &str, key: &str, value: &str) -> Result<()> {
        self.set_plugin_setting(plugin_id, key, value)
    }

    /// 获取配置项（别名，用于兼容）
    pub fn get(&self, plugin_id: &str, key: &str) -> Option<String> {
        self.get_plugin_setting(plugin_id, key)
    }

    /// 获取所有配置（别名，用于兼容）
    pub fn get_all(&self, plugin_id: &str) -> HashMap<String, String> {
        if let Some(config) = self.get_plugin_config(plugin_id) {
            config.settings
        } else {
            HashMap::new()
        }
    }

    /// 重置配置（别名，用于兼容）
    pub fn reset(&mut self, plugin_id: &str) -> Result<()> {
        self.reset_plugin(plugin_id)
    }

    /// 导出配置
    pub fn export_config(&self) -> Result<String> {
        toml::to_string_pretty(&*self.global_config.read().unwrap())
            .map_err(|e| EnvError::PluginConfigError(e.to_string()))
    }

    /// 导入配置
    pub fn import_config(&mut self, content: &str) -> Result<()> {
        let config: PluginGlobalConfig = toml::from_str(content)?;
        *self.global_config.write().unwrap() = config;
        self.save()
    }

    /// 获取配置文件路径
    pub fn get_config_path(&self) -> &Path {
        &self.config_path
    }

    /// 获取插件目录
    pub fn get_plugin_dir(&self) -> PathBuf {
        self.config_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// 插件配置格式化器（用于导出）
pub struct PluginConfigFormatter;

impl PluginConfigFormatter {
    /// 格式化为人类可读的文本
    pub fn format(config: &PluginConfig, verbose: bool) -> String {
        let mut output = String::new();

        output.push_str(&format!("插件 ID: {}\n", config.plugin_id));
        output.push_str(&format!(
            "启用状态: {}\n",
            if config.enabled { "✓" } else { "✗" }
        ));

        if verbose {
            if let Some(path) = &config.path {
                output.push_str(&format!("路径: {}\n", path.display()));
            }

            if let Some(timeout) = config.timeout {
                output.push_str(&format!("超时: {}秒\n", timeout));
            }

            if !config.settings.is_empty() {
                output.push_str("配置项:\n");
                for (key, value) in &config.settings {
                    output.push_str(&format!("  {}: {}\n", key, value));
                }
            }

            if !config.env.is_empty() {
                output.push_str("环境变量:\n");
                for (key, value) in &config.env {
                    output.push_str(&format!("  {}: {}\n", key, value));
                }
            }
        }

        output
    }

    /// 格式化为 JSON
    pub fn to_json(config: &PluginConfig) -> Result<String> {
        serde_json::to_string_pretty(config).map_err(|e| EnvError::PluginConfigError(e.to_string()))
    }

    /// 格式化为 TOML
    pub fn to_toml(config: &PluginConfig) -> Result<String> {
        toml::to_string_pretty(config).map_err(|e| EnvError::PluginConfigError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::types::{Platform, PluginType};

    #[test]
    fn test_plugin_config_manager() {
        // 创建临时目录用于测试
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        // 创建测试配置
        let mut global_config = PluginGlobalConfig::default();
        global_config.plugins.push(PluginConfig {
            plugin_id: "test-plugin".to_string(),
            enabled: true,
            settings: HashMap::new(),
            path: None,
            timeout: Some(30),
            env: HashMap::new(),
        });

        // 保存配置
        let content = toml::to_string_pretty(&global_config).unwrap();
        std::fs::write(&config_path, content).unwrap();

        // 验证配置可以加载
        let loaded: PluginGlobalConfig =
            toml::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();

        assert_eq!(loaded.plugins.len(), 1);
        assert_eq!(loaded.plugins[0].plugin_id, "test-plugin");
    }

    #[test]
    fn test_plugin_config_validation() {
        let manager = PluginConfigManager::new().unwrap();

        let metadata = PluginMetadata {
            id: "test".to_string(),
            name: "Test".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
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

        let config = PluginConfig {
            plugin_id: "test".to_string(),
            enabled: true,
            settings: HashMap::new(),
            path: None,
            timeout: Some(30),
            env: HashMap::new(),
        };

        // 应该通过验证
        assert!(manager.validate_config(&config, &metadata).is_ok());
    }

    #[test]
    fn test_plugin_config_formatter() {
        let config = PluginConfig {
            plugin_id: "test".to_string(),
            enabled: true,
            settings: {
                let mut map = HashMap::new();
                map.insert("key".to_string(), "value".to_string());
                map
            },
            path: Some(PathBuf::from("/test/path")),
            timeout: Some(60),
            env: HashMap::new(),
        };

        let formatted = PluginConfigFormatter::format(&config, true);
        assert!(formatted.contains("test"));
        assert!(formatted.contains("key"));
        assert!(formatted.contains("60"));
    }
}
