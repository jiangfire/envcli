//! 错误处理模块 (修复原则：明确抛出异常)

use thiserror::Error;
use std::error::Error;
use std::path::PathBuf;

#[derive(Error, Debug)]
pub enum EnvError {
    #[error("文件IO错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("解析错误: {0}")]
    Parse(String),

    #[error("变量未找到: {0}")]
    NotFound(String),

    #[error("配置目录不存在: {0}")]
    ConfigDirMissing(String),

    #[error("权限不足: {0}")]
    PermissionDenied(String),

    #[error("无效的环境层级: {0}")]
    InvalidSource(String),

    #[error("文件不存在: {0}")]
    FileNotFound(PathBuf),

    #[error("JSON序列化错误: {0}")]
    Json(#[from] serde_json::Error),

    // === 新增：run 命令相关错误 ===
    #[error("环境变量解析错误: {0}")]
    EnvParseError(String),

    #[error("命令未找到: {0}")]
    CommandNotFound(String),

    #[error("命令执行失败: {0}")]
    CommandExecutionFailed(String),
}

/// 详细的错误报告函数 (透明原则)
impl EnvError {
    /// 报告错误，支持详细/安静模式
    /// verbose = true: 详细错误链
    /// verbose = false: 关键信息，安静模式
    pub fn report(&self, verbose: bool) {
        if verbose {
            // 详细模式：打印完整错误链
            eprintln!("❌ 错误: {}", self);

            // 如果有源错误，打印级联信息
            // (thiserror 支持自动的 source() 链)
            if let Some(source) = self.source() {
                eprintln!("  └─ 原因: {}", source);
                let mut current = source.source();
                while let Some(next) = current {
                    eprintln!("     └─ {}", next);
                    current = next.source();
                }
            }
        } else {
            // 安静模式：只打印关键信息
            match self {
                EnvError::NotFound(key) => eprintln!("未找到变量: {}", key),
                EnvError::Io(err) => eprintln!("文件错误: {}", err),
                EnvError::PermissionDenied(msg) => eprintln!("权限被拒绝: {}", msg),
                EnvError::InvalidSource(src) => eprintln!("无效层级: {}", src),
                EnvError::FileNotFound(path) => eprintln!("文件不存在: {}", path.display()),
                _ => eprintln!("错误: {}", self),
            }
        }
    }
}

/// 简化 Result 类型别名
pub type Result<T> = std::result::Result<T, EnvError>;