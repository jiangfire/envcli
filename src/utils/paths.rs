//! 跨平台路径处理工具 (传统原则：常识性接口设计)

use crate::error::{EnvError, Result};
use crate::types::EnvSource;
use std::path::{Path, PathBuf};

/// 获取用户配置目录：~/.envcli
pub fn get_config_dir() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| EnvError::ConfigDirMissing("无法找到用户主目录".to_string()))?;

    Ok(home.join(".envcli"))
}

/// 获取指定层级的文件路径
pub fn get_layer_path(source: &EnvSource) -> Result<PathBuf> {
    let config_dir = get_config_dir()?;

    match source {
        EnvSource::System => Err(EnvError::PermissionDenied("系统环境变量层只读".to_string())),
        EnvSource::User => Ok(config_dir.join("user.env")),
        EnvSource::Project => {
            let current_dir = std::env::current_dir()
                .map_err(|e| EnvError::ConfigDirMissing(format!("无法获取当前目录: {}", e)))?;
            Ok(current_dir.join(".envcli").join("project.env"))
        }
        EnvSource::Local => {
            let current_dir = std::env::current_dir()
                .map_err(|e| EnvError::ConfigDirMissing(format!("无法获取当前目录: {}", e)))?;
            Ok(current_dir.join(".envcli").join("local.env"))
        }
    }
}

/// 确保配置目录存在 (幂等操作)
pub fn ensure_config_dir() -> Result<()> {
    let config_dir = get_config_dir()?;
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir)?;
    }
    Ok(())
}

/// 确保项目级目录存在
pub fn ensure_project_dir() -> Result<()> {
    let current_dir = std::env::current_dir()
        .map_err(|e| EnvError::ConfigDirMissing(format!("无法获取当前目录: {}", e)))?;
    let project_dir = current_dir.join(".envcli");
    if !project_dir.exists() {
        std::fs::create_dir_all(&project_dir)?;
    }
    Ok(())
}

/// 检查文件是否存在
pub fn file_exists(path: &Path) -> bool {
    path.exists() && path.is_file()
}

/// 读取文件内容，返回错误时提供详细信息
pub fn read_file(path: &Path) -> Result<String> {
    if !path.exists() {
        return Err(EnvError::FileNotFound(path.to_path_buf()));
    }
    std::fs::read_to_string(path).map_err(|e| {
        EnvError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("读取文件 {} 失败: {}", path.display(), e),
        ))
    })
}

/// 安全写入文件 (使用临时文件 + 原子替换)
pub fn write_file_safe(path: &Path, content: &str) -> Result<()> {
    // 确保父目录存在
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    // 写入临时文件
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, content)?;

    // 原子替换
    std::fs::rename(&temp_path, path)?;

    Ok(())
}

/// 追加内容到文件 (如果内容已存在则不追加)
pub fn append_to_file_unique(path: &Path, line: &str) -> Result<()> {
    // 如果文件存在，检查是否已有该行
    if path.exists() {
        let content = read_file(path)?;
        if content.lines().any(|l| l.trim() == line.trim()) {
            return Ok(()); // 已存在，无需操作
        }
    }

    // 追加内容
    let mut content = if path.exists() {
        read_file(path)?
    } else {
        String::new()
    };

    if !content.ends_with('\n') && !content.is_empty() {
        content.push('\n');
    }
    content.push_str(line);
    content.push('\n');

    write_file_safe(path, &content)
}

/// 获取系统环境变量 (跨平台统一接口)
pub fn get_system_env() -> Result<std::collections::HashMap<String, String>> {
    let mut env = std::collections::HashMap::new();

    // 在 Unix 上区分大小写，Windows 上不区分
    // 这里我们统一为大小写敏感，但在比较时会处理
    for (key, value) in std::env::vars() {
        env.insert(key, value);
    }

    Ok(env)
}