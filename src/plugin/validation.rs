//! 插件验证工具
//!
//! 提供插件ID、路径等输入的验证功能

use crate::plugin::types::PluginError;
use std::path::Path;

/// 插件ID验证器
pub struct PluginIdValidator;

impl PluginIdValidator {
    /// 验证插件ID是否合法
    ///
    /// # 规则
    /// - 长度：1-64字符
    /// - 只允许：字母、数字、下划线、连字符
    /// - 不能以连字符开头或结尾
    /// - 不能包含连续的特殊字符
    pub fn validate(id: &str) -> Result<(), PluginError> {
        if id.is_empty() {
            return Err(PluginError::ConfigError("插件ID不能为空".to_string()));
        }

        if id.len() > 64 {
            return Err(PluginError::ConfigError(
                "插件ID长度不能超过64字符".to_string(),
            ));
        }

        // 检查字符范围
        for c in id.chars() {
            if !c.is_ascii_alphanumeric() && c != '_' && c != '-' {
                return Err(PluginError::ConfigError(format!(
                    "插件ID包含非法字符 '{}', 只允许字母、数字、下划线和连字符",
                    c
                )));
            }
        }

        // 检查开头和结尾
        if id.starts_with('-') || id.starts_with('_') {
            return Err(PluginError::ConfigError(
                "插件ID不能以连字符或下划线开头".to_string(),
            ));
        }
        if id.ends_with('-') || id.ends_with('_') {
            return Err(PluginError::ConfigError(
                "插件ID不能以连字符或下划线结尾".to_string(),
            ));
        }

        // 检查连续特殊字符
        if id.contains("--") || id.contains("__") || id.contains("-_") || id.contains("_-") {
            return Err(PluginError::ConfigError(
                "插件ID不能包含连续的特殊字符".to_string(),
            ));
        }

        Ok(())
    }

    /// 检查插件ID是否已存在
    pub fn check_conflict(id: &str, existing_ids: &[String]) -> Result<(), PluginError> {
        if existing_ids.contains(&id.to_string()) {
            return Err(PluginError::AlreadyExists(format!(
                "插件ID '{}' 已存在",
                id
            )));
        }
        Ok(())
    }
}

/// 路径验证器
pub struct PathValidator;

impl PathValidator {
    /// 验证插件文件路径
    pub fn validate_plugin_path(path: &Path) -> Result<(), PluginError> {
        // 检查路径是否为空
        if path.as_os_str().is_empty() {
            return Err(PluginError::LoadFailed("插件路径不能为空".to_string()));
        }

        // 检查路径是否包含空字符（安全检查）
        if path.to_string_lossy().contains('\0') {
            return Err(PluginError::LoadFailed(
                "插件路径包含空字符".to_string(),
            ));
        }

        Ok(())
    }

    /// 验证路径是否在允许的目录范围内（防止路径遍历攻击）
    pub fn validate_path_sandbox(path: &Path, allowed_base: &Path) -> Result<(), PluginError> {
        // 标准化路径
        let path = path.canonicalize().map_err(|e| {
            PluginError::LoadFailed(format!("无法解析路径 {}: {}", path.display(), e))
        })?;

        let base = allowed_base.canonicalize().map_err(|e| {
            PluginError::LoadFailed(format!("无法解析基础路径 {}: {}", allowed_base.display(), e))
        })?;

        // 检查是否在基础路径下
        if !path.starts_with(&base) {
            return Err(PluginError::LoadFailed(format!(
                "插件路径 {} 不在允许的目录 {} 下",
                path.display(),
                base.display()
            )));
        }

        Ok(())
    }
}

/// 配置验证器
pub struct ConfigValidator;

impl ConfigValidator {
    /// 验证插件配置
    pub fn validate_config(config: &crate::plugin::types::PluginConfig) -> Result<(), PluginError> {
        // 验证超时
        if let Some(timeout) = config.timeout {
            if timeout == 0 {
                return Err(PluginError::ConfigError(
                    "超时时间不能为0".to_string(),
                ));
            }
            if timeout > 3600 {
                return Err(PluginError::ConfigError(
                    "超时时间不能超过3600秒".to_string(),
                ));
            }
        }

        // 验证环境变量键名
        for key in config.env.keys() {
            if key.is_empty() {
                return Err(PluginError::ConfigError(
                    "环境变量键名不能为空".to_string(),
                ));
            }
            // 检查是否包含特殊字符（可能导致注入攻击）
            if key.contains(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
                return Err(PluginError::ConfigError(format!(
                    "环境变量键名包含非法字符: {}",
                    key
                )));
            }
        }

        // 验证设置键名
        for key in config.settings.keys() {
            if key.is_empty() {
                return Err(PluginError::ConfigError(
                    "配置项键名不能为空".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// 验证全局配置
    pub fn validate_global_config(
        config: &crate::plugin::config::PluginGlobalConfig,
    ) -> Result<(), PluginError> {
        // 验证全局设置
        if config.global.default_timeout == 0 {
            return Err(PluginError::ConfigError(
                "默认超时不能为0".to_string(),
            ));
        }

        if config.global.default_timeout > 3600 {
            return Err(PluginError::ConfigError(
                "默认超时不能超过3600秒".to_string(),
            ));
        }

        // 检查插件ID重复
        let mut ids = std::collections::HashSet::new();
        for plugin in &config.plugins {
            if !ids.insert(&plugin.plugin_id) {
                return Err(PluginError::ConfigError(format!(
                    "检测到重复的插件ID: {}",
                    plugin.plugin_id
                )));
            }

            // 验证每个插件配置
            Self::validate_config(plugin)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_id_validation() {
        // 合法的ID
        assert!(PluginIdValidator::validate("my-plugin").is_ok());
        assert!(PluginIdValidator::validate("plugin123").is_ok());
        assert!(PluginIdValidator::validate("my_plugin").is_ok());
        assert!(PluginIdValidator::validate("my-plugin-123").is_ok());

        // 非法的ID
        assert!(PluginIdValidator::validate("").is_err()); // 空
        assert!(PluginIdValidator::validate(&"a".repeat(65)).is_err()); // 太长
        assert!(PluginIdValidator::validate("plugin@").is_err()); // 特殊字符
        assert!(PluginIdValidator::validate("-plugin").is_err()); // 开头是-
        assert!(PluginIdValidator::validate("plugin-").is_err()); // 结尾是-
        assert!(PluginIdValidator::validate("plugin--test").is_err()); // 连续-
        assert!(PluginIdValidator::validate("plugin__test").is_err()); // 连续_
    }

    #[test]
    fn test_plugin_id_conflict() {
        let existing = vec!["plugin1".to_string(), "plugin2".to_string()];
        assert!(PluginIdValidator::check_conflict("plugin3", &existing).is_ok());
        assert!(PluginIdValidator::check_conflict("plugin1", &existing).is_err());
    }

    #[test]
    fn test_path_validation() {
        use std::path::PathBuf;

        // 空路径
        assert!(PathValidator::validate_plugin_path(Path::new("")).is_err());

        // 包含空字符
        let path = PathBuf::from("plugin\0.so");
        assert!(PathValidator::validate_plugin_path(&path).is_err());
    }
}
