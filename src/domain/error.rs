//! 领域层错误类型

use miette::Diagnostic;
use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, DomainError>;

/// 领域层错误类型
#[derive(Error, Debug, Diagnostic, Clone)]
pub enum DomainError {
    #[error("变量未找到: {0}")]
    #[diagnostic(code(envcli::not_found), help("使用 'envcli list' 查看所有变量"))]
    NotFound(String),

    #[error("存储错误: {0}")]
    #[diagnostic(code(envcli::storage))]
    Storage(String),

    #[error("IO 错误: {0}")]
    #[diagnostic(code(envcli::io))]
    Io(String),

    #[error("权限不足: {0}")]
    #[diagnostic(
        code(envcli::permission_denied),
        help("检查文件权限，或在 Windows 上以管理员身份运行")
    )]
    PermissionDenied(String),

    #[error("无效的环境层级: {0}")]
    #[diagnostic(
        code(envcli::invalid_source),
        help("有效层级: system, user, project, local")
    )]
    InvalidSource(String),

    #[error("文件不存在: {0}")]
    #[diagnostic(code(envcli::file_not_found))]
    FileNotFound(PathBuf),

    #[error("解析错误: {0}")]
    #[diagnostic(code(envcli::parse))]
    Parse(String),

    #[error("序列化错误: {0}")]
    #[diagnostic(code(envcli::serialization))]
    Serialization(String),

    #[error("环境变量解析错误: {0}")]
    #[diagnostic(code(envcli::env_parse))]
    EnvParse(String),

    #[error("命令未找到: {0}")]
    #[diagnostic(code(envcli::command_not_found), help("确保命令在 PATH 中"))]
    CommandNotFound(String),

    #[error("命令执行失败: {0}")]
    #[diagnostic(code(envcli::command_execution))]
    CommandExecutionFailed(String),

    #[error("无效参数: {0}")]
    #[diagnostic(code(envcli::invalid_argument))]
    InvalidArgument(String),

    #[error("配置错误: {0}")]
    #[diagnostic(code(envcli::config))]
    Config(String),

    #[error("系统环境变量写入失败: {0}")]
    #[diagnostic(code(envcli::system_env_write))]
    SystemEnvWriteFailed(String),
}

impl From<std::io::Error> for DomainError {
    fn from(err: std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::PermissionDenied => DomainError::PermissionDenied(err.to_string()),
            std::io::ErrorKind::NotFound => DomainError::FileNotFound(PathBuf::from("")),
            _ => DomainError::Io(err.to_string()),
        }
    }
}

impl From<serde_json::Error> for DomainError {
    fn from(err: serde_json::Error) -> Self {
        DomainError::Serialization(err.to_string())
    }
}
