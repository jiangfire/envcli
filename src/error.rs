//! 错误处理模块 (修复原则：明确抛出异常)

use std::error::Error;
use std::path::PathBuf;
use thiserror::Error;
use crate::plugin::PluginError;

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

    #[error("TOML序列化错误: {0}")]
    Toml(String),

    // === 新增：run 命令相关错误 ===
    #[error("环境变量解析错误: {0}")]
    EnvParseError(String),

    #[error("命令未找到: {0}")]
    CommandNotFound(String),

    #[error("命令执行失败: {0}")]
    CommandExecutionFailed(String),

    // === 新增：模板相关错误 ===
    #[error("模板不存在: {0}")]
    TemplateNotFound(String),

    #[error("缺少必需变量: {0}")]
    MissingVariable(String),

    #[error("循环继承检测到: {0}")]
    CircularInheritance(String),

    #[error("解析错误: {0}")]
    ParseError(String),

    // === 新增：加密相关错误 ===
    #[error("加密错误: {0}")]
    EncryptionError(String),

    #[error("解密错误: {0}")]
    DecryptionError(String),

    // === 新增：插件相关错误 ===
    #[error("插件未找到: {0}")]
    PluginNotFound(String),

    #[error("插件加载失败: {0}")]
    PluginLoadFailed(String),

    #[error("插件执行失败: {0}")]
    PluginExecutionFailed(String),

    #[error("插件配置错误: {0}")]
    PluginConfigError(String),

    #[error("插件依赖缺失: {0}")]
    PluginDependencyMissing(String),

    #[error("插件不兼容: {0}")]
    PluginIncompatible(String),

    // === 新增：系统环境变量相关错误 ===
    #[error("系统环境变量写入失败: {0}")]
    SystemEnvWriteFailed(String),

    #[error("需要管理员权限: {0}")]
    AdminPrivilegesRequired(String),

    #[error("无效参数: {0}")]
    InvalidArgument(String),
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
                EnvError::SystemEnvWriteFailed(msg) => eprintln!("系统环境变量写入失败: {}", msg),
                EnvError::AdminPrivilegesRequired(msg) => eprintln!("需要管理员权限: {}", msg),
                EnvError::InvalidArgument(msg) => eprintln!("无效参数: {}", msg),
                _ => eprintln!("错误: {}", self),
            }
        }
    }
}

/// 简化 Result 类型别名
pub type Result<T> = std::result::Result<T, EnvError>;

/// From 实现 for TOML 错误
impl From<toml::de::Error> for EnvError {
    fn from(err: toml::de::Error) -> Self {
        EnvError::Toml(err.to_string())
    }
}

impl From<toml::ser::Error> for EnvError {
    fn from(err: toml::ser::Error) -> Self {
        EnvError::Toml(err.to_string())
    }
}

impl From<PluginError> for EnvError {
    fn from(err: PluginError) -> Self {
        match err {
            PluginError::NotFound(s) => EnvError::PluginNotFound(s),
            PluginError::LoadFailed(s) => EnvError::PluginLoadFailed(s),
            PluginError::ExecutionFailed(s) => EnvError::PluginExecutionFailed(s),
            PluginError::ConfigError(s) => EnvError::PluginConfigError(s),
            PluginError::DependencyMissing(s) => EnvError::PluginDependencyMissing(s),
            PluginError::Incompatible(s) => EnvError::PluginIncompatible(s),
            PluginError::Io(io_err) => EnvError::Io(io_err),
            PluginError::Json(json_err) => EnvError::Json(json_err),
            PluginError::Toml(toml_err) => EnvError::Toml(toml_err.to_string()),
            PluginError::Timeout(s) => EnvError::PluginExecutionFailed(s),
            PluginError::AlreadyExists(s) => EnvError::PluginLoadFailed(s),
            PluginError::Unsupported(s) => EnvError::PluginExecutionFailed(s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    mod error_display_tests {
        use super::*;

        #[test]
        fn test_io_error_display() {
            let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
            let env_err = EnvError::Io(io_err);
            assert!(env_err.to_string().contains("文件IO错误"));
        }

        #[test]
        fn test_parse_error_display() {
            let err = EnvError::Parse("invalid format".to_string());
            assert!(err.to_string().contains("解析错误"));
            assert!(err.to_string().contains("invalid format"));
        }

        #[test]
        fn test_not_found_error_display() {
            let err = EnvError::NotFound("MY_VAR".to_string());
            assert!(err.to_string().contains("变量未找到"));
            assert!(err.to_string().contains("MY_VAR"));
        }

        #[test]
        fn test_config_dir_missing_error_display() {
            let err = EnvError::ConfigDirMissing("no home dir".to_string());
            assert!(err.to_string().contains("配置目录不存在"));
            assert!(err.to_string().contains("no home dir"));
        }

        #[test]
        fn test_permission_denied_error_display() {
            let err = EnvError::PermissionDenied("read-only".to_string());
            assert!(err.to_string().contains("权限不足"));
            assert!(err.to_string().contains("read-only"));
        }

        #[test]
        fn test_invalid_source_error_display() {
            let err = EnvError::InvalidSource("invalid".to_string());
            assert!(err.to_string().contains("无效的环境层级"));
            assert!(err.to_string().contains("invalid"));
        }

        #[test]
        fn test_file_not_found_error_display() {
            use std::path::PathBuf;
            let err = EnvError::FileNotFound(PathBuf::from("/nonexistent/file"));
            assert!(err.to_string().contains("文件不存在"));
            assert!(err.to_string().contains("/nonexistent/file"));
        }

        #[test]
        fn test_json_error_display() {
            let json_err = serde_json::from_str::<i32>("invalid").unwrap_err();
            let err = EnvError::Json(json_err);
            assert!(err.to_string().contains("JSON序列化错误"));
        }

        #[test]
        fn test_env_parse_error_display() {
            let err = EnvError::EnvParseError("invalid=env=format".to_string());
            assert!(err.to_string().contains("环境变量解析错误"));
            assert!(err.to_string().contains("invalid=env=format"));
        }

        #[test]
        fn test_command_not_found_error_display() {
            let err = EnvError::CommandNotFound("missing_command".to_string());
            assert!(err.to_string().contains("命令未找到"));
            assert!(err.to_string().contains("missing_command"));
        }

        #[test]
        fn test_command_execution_failed_error_display() {
            let err = EnvError::CommandExecutionFailed("exit code 1".to_string());
            assert!(err.to_string().contains("命令执行失败"));
            assert!(err.to_string().contains("exit code 1"));
        }

        #[test]
        fn test_template_not_found_error_display() {
            let err = EnvError::TemplateNotFound("missing_template".to_string());
            assert!(err.to_string().contains("模板不存在"));
            assert!(err.to_string().contains("missing_template"));
        }

        #[test]
        fn test_missing_variable_error_display() {
            let err = EnvError::MissingVariable("REQUIRED_VAR".to_string());
            assert!(err.to_string().contains("缺少必需变量"));
            assert!(err.to_string().contains("REQUIRED_VAR"));
        }

        #[test]
        fn test_circular_inheritance_error_display() {
            let err = EnvError::CircularInheritance("a -> b -> c -> a".to_string());
            assert!(err.to_string().contains("循环继承检测到"));
            assert!(err.to_string().contains("a -> b -> c -> a"));
        }

        #[test]
        fn test_parse_error_variant_display() {
            let err = EnvError::ParseError("syntax error".to_string());
            assert!(err.to_string().contains("解析错误"));
            assert!(err.to_string().contains("syntax error"));
        }
    }

    mod error_report_tests {
        use super::*;

        // 注意：实际捕获 stderr 需要复杂的设置
        // 这里我们只测试 report 方法可以被调用且不 panic

        #[test]
        fn test_report_verbose_mode() {
            let err = EnvError::NotFound("TEST_VAR".to_string());
            // 只是验证 report 方法可以被调用且不 panic
            err.report(true);
        }

        #[test]
        fn test_report_quiet_mode() {
            let err = EnvError::Io(io::Error::new(io::ErrorKind::NotFound, "test"));
            err.report(false);
        }

        #[test]
        fn test_report_with_io_error() {
            let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
            let err = EnvError::Io(io_err);
            err.report(true);
            err.report(false);
        }

        #[test]
        fn test_report_with_permission_denied() {
            let err = EnvError::PermissionDenied("no permission".to_string());
            err.report(true);
            err.report(false);
        }

        #[test]
        fn test_report_with_file_not_found() {
            use std::path::PathBuf;
            let err = EnvError::FileNotFound(PathBuf::from("/missing"));
            err.report(true);
            err.report(false);
        }

        #[test]
        fn test_report_with_invalid_source() {
            let err = EnvError::InvalidSource("bad".to_string());
            err.report(true);
            err.report(false);
        }
    }

    mod result_type_tests {
        use super::*;

        #[test]
        fn test_result_type_with_success() {
            fn returns_result() -> Result<String> {
                Ok("success".to_string())
            }

            let result = returns_result();
            assert!(result.is_ok());
            assert_eq!(result.unwrap(), "success");
        }

        #[test]
        fn test_result_type_with_error() {
            fn returns_result() -> Result<String> {
                Err(EnvError::NotFound("missing".to_string()))
            }

            let result = returns_result();
            assert!(result.is_err());
        }

        #[test]
        fn test_result_type_from_io_error() {
            fn returns_io_result() -> Result<()> {
                let io_err = io::Error::other("io error");
                Err(EnvError::Io(io_err))
            }

            let result = returns_io_result();
            assert!(result.is_err());
        }

        #[test]
        fn test_result_type_from_json_error() {
            fn returns_json_result() -> Result<()> {
                let json_err = serde_json::from_str::<i32>("not a number").unwrap_err();
                Err(EnvError::Json(json_err))
            }

            let result = returns_json_result();
            assert!(result.is_err());
        }
    }

    mod error_chaining_tests {
        use super::*;

        #[test]
        fn test_io_error_source_chain() {
            let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
            let env_err = EnvError::Io(io_err);

            // 验证错误可以被格式化
            let display = format!("{}", env_err);
            assert!(display.contains("文件IO错误"));
        }

        #[test]
        fn test_json_error_source_chain() {
            let json_err = serde_json::from_str::<i32>("invalid").unwrap_err();
            let env_err = EnvError::Json(json_err);

            let display = format!("{}", env_err);
            assert!(display.contains("JSON序列化错误"));
        }
    }
}
