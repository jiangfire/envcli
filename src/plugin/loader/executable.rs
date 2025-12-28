//! 外部可执行文件插件加载器
//!
//! 加载外部可执行文件插件（Python、Shell、Node.js 等）

use crate::plugin::loader::PluginLoader;
use crate::plugin::types::{
    HookContext, HookContextStatic, HookResult, Plugin, PluginConfig, PluginError, PluginMetadata,
    PluginRequest, PluginResponse, PluginType,
};
use serde_json;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// 外部可执行插件包装器
pub struct ExecutablePlugin {
    metadata: PluginMetadata,
    config: PluginConfig,
    executable_path: PathBuf,
}

impl ExecutablePlugin {
    /// 执行外部插件
    fn execute_plugin_action(
        &self,
        action: &str,
        hook_type: Option<crate::plugin::types::HookType>,
        context: Option<&HookContext>,
    ) -> Result<PluginResponse, PluginError> {
        // 构建请求（将 HookContext 转换为可序列化的 HookContextStatic）
        let context_static = context.map(HookContextStatic::from);

        let request = PluginRequest {
            action: action.to_string(),
            hook_type,
            context: context_static,
            config: Some(self.config.clone()),
        };

        let request_json = serde_json::to_string(&request)?;

        // 启动子进程
        let mut child = Command::new(&self.executable_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .envs(&self.config.env)
            .spawn()
            .map_err(|e| {
                PluginError::ExecutionFailed(format!(
                    "无法启动插件 {}: {}",
                    self.executable_path.display(),
                    e
                ))
            })?;

        // 写入请求
        if let Some(stdin) = &mut child.stdin {
            stdin
                .write_all(request_json.as_bytes())
                .map_err(|e| PluginError::ExecutionFailed(format!("写入请求失败: {}", e)))?;
        }

        // 读取响应
        let output = child
            .wait_with_output()
            .map_err(|e| PluginError::ExecutionFailed(format!("执行失败: {}", e)))?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(PluginError::ExecutionFailed(format!(
                "插件执行失败: {}",
                error_msg
            )));
        }

        // 解析响应
        let response_str = String::from_utf8(output.stdout)
            .map_err(|e| PluginError::ExecutionFailed(format!("无效UTF-8输出: {}", e)))?;
        let response: PluginResponse = serde_json::from_str(&response_str)?;

        if !response.success {
            return Err(PluginError::ExecutionFailed(
                response.error.unwrap_or_else(|| "Unknown error".to_string()),
            ));
        }

        Ok(response)
    }
}

impl Plugin for ExecutablePlugin {
    fn metadata(&self) -> PluginMetadata {
        self.metadata.clone()
    }

    fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        self.config = config.clone();
        // 获取元数据
        let response = self.execute_plugin_action("metadata", None, None)?;
        if let Some(metadata) = response.metadata {
            self.metadata = metadata;
        }
        Ok(())
    }

    fn execute_hook(
        &self,
        hook_type: crate::plugin::types::HookType,
        context: &HookContext,
    ) -> Result<HookResult, PluginError> {
        let response = self.execute_plugin_action("execute_hook", Some(hook_type), Some(context))?;
        response.result.ok_or_else(|| {
            PluginError::ExecutionFailed("插件未返回钩子结果".to_string())
        })
    }

    fn supports_extension(&self, _extension: crate::plugin::types::ExtensionPoint) -> bool {
        false
    }

    fn execute_extension(
        &self,
        _extension: crate::plugin::types::ExtensionPoint,
        _input: &[u8],
    ) -> Result<Vec<u8>, PluginError> {
        Err(PluginError::Unsupported("外部插件不支持扩展".to_string()))
    }

    fn shutdown(&mut self) -> Result<(), PluginError> {
        // 外部插件不需要特殊清理
        Ok(())
    }
}

/// 可执行文件加载器
pub struct ExecutableLoader;

impl PluginLoader for ExecutableLoader {
    fn load(&self, path: &Path, config: PluginConfig) -> Result<Box<dyn Plugin>, PluginError> {
        // 验证文件存在
        if !path.exists() {
            return Err(PluginError::LoadFailed(format!(
                "可执行文件不存在: {}",
                path.display()
            )));
        }

        // 验证文件可执行（Unix 系统）
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(path)?;
            let permissions = metadata.permissions();
            if (permissions.mode() & 0o111) == 0 {
                return Err(PluginError::LoadFailed(format!(
                    "文件不可执行: {}",
                    path.display()
                )));
            }
        }

        // 尝试获取元数据
        let plugin = ExecutablePlugin {
            metadata: PluginMetadata {
                id: path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                name: path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                version: "0.0.0".to_string(),
                description: None,
                author: None,
                plugin_type: PluginType::ExternalExecutable,
                hooks: vec![],
                extensions: vec![],
                config_schema: None,
                enabled: true,
                dependencies: vec![],
                platforms: vec![crate::plugin::types::Platform::current()],
                envcli_version: None,
                signature: None,
            },
            config: config.clone(),
            executable_path: path.to_path_buf(),
        };

        // 包装为 Box
        let mut boxed = Box::new(plugin);

        // 初始化（获取真实元数据）
        boxed.initialize(&config)?;

        Ok(boxed)
    }

    fn unload(&self, plugin: &mut dyn Plugin) -> Result<(), PluginError> {
        plugin.shutdown()
    }

    fn supported_types(&self) -> Vec<PluginType> {
        vec![PluginType::ExternalExecutable]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executable_loader_supported_types() {
        let loader = ExecutableLoader;
        let types = loader.supported_types();
        assert_eq!(types.len(), 1);
        assert!(types.contains(&PluginType::ExternalExecutable));
    }

    #[test]
    fn test_executable_loader_load_nonexistent() {
        let loader = ExecutableLoader;
        let path = PathBuf::from("/nonexistent/plugin.py");
        let config = PluginConfig::default();

        let result = loader.load(&path, config);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(matches!(err, PluginError::LoadFailed(_)));
    }
}
