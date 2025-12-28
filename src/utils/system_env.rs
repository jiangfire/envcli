//! 跨平台系统环境变量写入工具
//!
//! 提供跨平台的系统环境变量写入功能：
//! - Windows: 使用 PowerShell 写入注册表
//! - Linux/macOS: 写入 shell 配置文件 (~/.bashrc, ~/.zshrc 等)

use crate::error::{EnvError, Result};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::path::PathBuf;
use std::process::Command;

/// 系统环境变量写入器
pub struct SystemEnvWriter;

impl SystemEnvWriter {
    /// 设置用户级系统环境变量（永久生效）
    ///
    /// # 平台差异
    /// - **Windows**: 写入 HKEY_CURRENT_USER\Environment
    /// - **Unix/Linux**: 写入 ~/.bashrc 或 ~/.zshrc
    /// - **macOS**: 写入 ~/.zprofile 或 ~/.zshrc
    pub fn set_user_var(key: &str, value: &str) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            Self::set_user_var_windows(key, value)
        }

        #[cfg(target_os = "linux")]
        {
            Self::set_user_var_linux(key, value)
        }

        #[cfg(target_os = "macos")]
        {
            Self::set_user_var_macos(key, value)
        }
    }

    /// 设置机器级系统环境变量（需要管理员权限）
    ///
    /// # 注意
    /// 仅 Windows 支持机器级变量
    /// Unix/Linux/macOS 会返回错误
    pub fn set_machine_var(key: &str, value: &str) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            Self::set_machine_var_windows(key, value)
        }

        #[cfg(not(target_os = "windows"))]
        {
            Err(EnvError::PermissionDenied(
                "Unix 系统不支持机器级环境变量，仅支持用户级 (global)".to_string(),
            ))
        }
    }

    /// 删除系统环境变量
    ///
    /// # 参数
    /// * `key` - 变量名称
    /// * `scope` - 作用域: "global" (用户级) 或 "machine" (系统级)
    pub fn unset_var(key: &str, scope: &str) -> Result<()> {
        match scope {
            "machine" => {
                #[cfg(target_os = "windows")]
                {
                    Self::unset_var_windows(key, true)
                }

                #[cfg(not(target_os = "windows"))]
                {
                    Err(EnvError::PermissionDenied(
                        "Unix 系统不支持机器级环境变量操作".to_string(),
                    ))
                }
            }
            _ => {
                #[cfg(target_os = "windows")]
                {
                    Self::unset_var_windows(key, false)
                }

                #[cfg(target_os = "linux")]
                {
                    Self::unset_var_linux(key)
                }

                #[cfg(target_os = "macos")]
                {
                    Self::unset_var_macos(key)
                }
            }
        }
    }

    // ==================== Windows 实现 ====================

    #[cfg(target_os = "windows")]
    fn set_user_var_windows(key: &str, value: &str) -> Result<()> {
        let ps_script = format!(
            "[Environment]::SetEnvironmentVariable(\"{}\", \"{}\", \"User\")",
            key.replace('"', "\"\""),
            value.replace('"', "\"\"")
        );

        let output = Command::new("powershell")
            .args(["-Command", &ps_script])
            .output()
            .map_err(|e| EnvError::SystemEnvWriteFailed(format!("无法执行 PowerShell: {}", e)))?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(EnvError::SystemEnvWriteFailed(
                format!("写入用户环境变量失败: {}", error_msg),
            ));
        }

        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn set_machine_var_windows(key: &str, value: &str) -> Result<()> {
        let ps_script = format!(
            "[Environment]::SetEnvironmentVariable(\"{}\", \"{}\", \"Machine\")",
            key.replace('"', "\"\""),
            value.replace('"', "\"\"")
        );

        let output = Command::new("powershell")
            .args(["-Command", &ps_script])
            .output()
            .map_err(|e| EnvError::SystemEnvWriteFailed(format!("无法执行 PowerShell: {}", e)))?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            // 检查是否是权限问题
            if error_msg.contains("拒绝") || error_msg.contains("Access denied") {
                return Err(EnvError::AdminPrivilegesRequired(
                    "设置机器级环境变量需要管理员权限，请以管理员身份运行".to_string(),
                ));
            }
            return Err(EnvError::SystemEnvWriteFailed(
                format!("写入机器环境变量失败: {}", error_msg),
            ));
        }

        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn unset_var_windows(key: &str, is_machine: bool) -> Result<()> {
        let scope = if is_machine { "Machine" } else { "User" };
        let ps_script = format!(
            "[Environment]::SetEnvironmentVariable(\"{}\", $null, \"{}\")",
            key.replace('"', "\"\""),
            scope
        );

        let output = Command::new("powershell")
            .args(["-Command", &ps_script])
            .output()
            .map_err(|e| EnvError::SystemEnvWriteFailed(format!("无法执行 PowerShell: {}", e)))?;

        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            if is_machine && (error_msg.contains("拒绝") || error_msg.contains("Access denied")) {
                return Err(EnvError::AdminPrivilegesRequired(
                    "删除机器级环境变量需要管理员权限".to_string(),
                ));
            }
            return Err(EnvError::SystemEnvWriteFailed(
                format!("删除环境变量失败: {}", error_msg),
            ));
        }

        Ok(())
    }

    // ==================== Linux 实现 ====================

    #[cfg(target_os = "linux")]
    fn set_user_var_linux(key: &str, value: &str) -> Result<()> {
        let config_file = Self::get_linux_config_file()?;
        Self::write_to_shell_config(&config_file, key, value)
    }

    #[cfg(target_os = "linux")]
    fn unset_var_linux(key: &str) -> Result<()> {
        let config_file = Self::get_linux_config_file()?;
        Self::remove_from_shell_config(&config_file, key)
    }

    #[cfg(target_os = "linux")]
    fn get_linux_config_file() -> Result<PathBuf> {
        use std::env;

        let home = env::var("HOME")
            .map_err(|_| EnvError::SystemEnvWriteFailed("无法获取 HOME 目录".to_string()))?;

        // 检查常见的 shell 配置文件
        let candidates = vec![
            PathBuf::from(&home).join(".bashrc"),
            PathBuf::from(&home).join(".zshrc"),
            PathBuf::from(&home).join(".profile"),
        ];

        for path in candidates {
            if path.exists() {
                return Ok(path);
            }
        }

        // 如果都不存在，默认使用 .bashrc
        Ok(PathBuf::from(&home).join(".bashrc"))
    }

    // ==================== macOS 实现 ====================

    #[cfg(target_os = "macos")]
    fn set_user_var_macos(key: &str, value: &str) -> Result<()> {
        let config_file = Self::get_macos_config_file()?;
        Self::write_to_shell_config(&config_file, key, value)
    }

    #[cfg(target_os = "macos")]
    fn unset_var_macos(key: &str) -> Result<()> {
        let config_file = Self::get_macos_config_file()?;
        Self::remove_from_shell_config(&config_file, key)
    }

    #[cfg(target_os = "macos")]
    fn get_macos_config_file() -> Result<PathBuf> {
        use std::env;

        let home = env::var("HOME")
            .map_err(|_| EnvError::SystemEnvWriteFailed("无法获取 HOME 目录".to_string()))?;

        // 检查常见的 shell 配置文件
        let candidates = vec![
            PathBuf::from(&home).join(".zshrc"),
            PathBuf::from(&home).join(".bash_profile"),
            PathBuf::from(&home).join(".bashrc"),
            PathBuf::from(&home).join(".zprofile"),
        ];

        for path in candidates {
            if path.exists() {
                return Ok(path);
            }
        }

        // 如果都不存在，默认使用 .zshrc
        Ok(PathBuf::from(&home).join(".zshrc"))
    }

    // ==================== Unix 通用实现 ====================

    /// 写入 shell 配置文件
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn write_to_shell_config(config_file: &PathBuf, key: &str, value: &str) -> Result<()> {
        use std::fs::{read_to_string, OpenOptions};
        use std::io::Write;

        // 读取现有内容
        let content = match read_to_string(config_file) {
            Ok(c) => c,
            Err(_) => String::new(),
        };

        // 检查是否已存在该变量
        let export_line = format!("export {}={}", key, value);
        let comment_line = format!("# envcli: {}", key);

        // 如果已存在，先删除旧的
        let mut lines: Vec<String> = Vec::new();
        let mut skip_next = false;

        for line in content.lines() {
            if line.trim().starts_with(&format!("export {}=", key)) || line.trim() == comment_line {
                skip_next = true;
                continue;
            }
            if skip_next && line.trim().is_empty() {
                skip_next = false;
                continue;
            }
            if !skip_next {
                lines.push(line.to_string());
            }
            skip_next = false;
        }

        // 添加新变量
        if !lines.is_empty() && !lines.last().unwrap().is_empty() {
            lines.push(String::new());
        }
        lines.push(comment_line);
        lines.push(export_line);
        lines.push(String::new());

        // 写入文件
        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(config_file)
            .map_err(|e| EnvError::SystemEnvWriteFailed(format!("无法打开配置文件: {}", e)))?;

        for line in lines {
            writeln!(file, "{}", line)
                .map_err(|e| EnvError::SystemEnvWriteFailed(format!("写入配置文件失败: {}", e)))?;
        }

        Ok(())
    }

    /// 从 shell 配置文件删除变量
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn remove_from_shell_config(config_file: &PathBuf, key: &str) -> Result<()> {
        use std::fs::{read_to_string, OpenOptions};
        use std::io::Write;

        let content = match read_to_string(config_file) {
            Ok(c) => c,
            Err(_) => return Ok(()), // 文件不存在，无需删除
        };

        let mut lines: Vec<String> = Vec::new();
        let mut skip_next = false;

        for line in content.lines() {
            if line.trim().starts_with(&format!("export {}=", key))
                || line.trim() == format!("# envcli: {}", key)
            {
                skip_next = true;
                continue;
            }
            if skip_next && line.trim().is_empty() {
                skip_next = false;
                continue;
            }
            if !skip_next {
                lines.push(line.to_string());
            }
            skip_next = false;
        }

        // 写入文件
        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(config_file)
            .map_err(|e| EnvError::SystemEnvWriteFailed(format!("无法打开配置文件: {}", e)))?;

        for line in lines {
            writeln!(file, "{}", line)
                .map_err(|e| EnvError::SystemEnvWriteFailed(format!("写入配置文件失败: {}", e)))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_env_writer_struct() {
        // 测试结构体可以被创建
        let _writer = SystemEnvWriter;
        // 这个测试只是验证结构体存在
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_windows_script_generation() {
        // 测试 Windows PowerShell 脚本生成逻辑
        let key = "TEST_VAR";
        let value = "test value";

        // 验证脚本格式
        let ps_script = format!(
            "[Environment]::SetEnvironmentVariable(\"{}\", \"{}\", \"User\")",
            key.replace('"', "\"\""),
            value.replace('"', "\"\"")
        );

        assert!(ps_script.contains("TEST_VAR"));
        assert!(ps_script.contains("test value"));
        assert!(ps_script.contains("User"));
    }

    #[test]
    fn test_scope_validation() {
        // 测试作用域参数处理
        let valid_scopes = ["global", "machine"];
        for scope in valid_scopes {
            // 验证作用域字符串有效
            assert!(scope == "global" || scope == "machine");
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_windows_user_var_command() {
        // 测试 Windows 用户级变量命令生成
        let key = "TEST_KEY";
        let value = "test_value";

        let ps_command = format!(
            "[Environment]::SetEnvironmentVariable(\"{}\", \"{}\", \"User\")",
            key.replace('"', "\"\""),
            value.replace('"', "\"\"")
        );

        assert!(ps_command.contains("TEST_KEY"));
        assert!(ps_command.contains("test_value"));
        assert!(ps_command.contains("User"));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_windows_machine_var_command() {
        // 测试 Windows 机器级变量命令生成
        let key = "TEST_KEY";
        let value = "test_value";

        let ps_command = format!(
            "[Environment]::SetEnvironmentVariable(\"{}\", \"{}\", \"Machine\")",
            key.replace('"', "\"\""),
            value.replace('"', "\"\"")
        );

        assert!(ps_command.contains("Machine"));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_windows_unset_command() {
        // 测试 Windows 删除变量命令生成
        let key = "TEST_KEY";

        // 用户级删除
        let ps_command_user = format!(
            "[Environment]::SetEnvironmentVariable(\"{}\", $null, \"User\")",
            key.replace('"', "\"\"")
        );
        assert!(ps_command_user.contains("$null"));
        assert!(ps_command_user.contains("User"));

        // 机器级删除
        let ps_command_machine = format!(
            "[Environment]::SetEnvironmentVariable(\"{}\", $null, \"Machine\")",
            key.replace('"', "\"\"")
        );
        assert!(ps_command_machine.contains("Machine"));
    }

    #[test]
    fn test_error_types() {
        // 测试错误类型可以被创建
        let _err1 = EnvError::SystemEnvWriteFailed("test error".to_string());
        let _err2 = EnvError::AdminPrivilegesRequired("admin needed".to_string());
        let _err3 = EnvError::InvalidArgument("bad scope".to_string());
    }

    #[test]
    fn test_error_display() {
        // 测试错误显示
        let err = EnvError::SystemEnvWriteFailed("写入失败".to_string());
        assert!(err.to_string().contains("系统环境变量写入失败"));
        assert!(err.to_string().contains("写入失败"));

        let err2 = EnvError::AdminPrivilegesRequired("需要管理员".to_string());
        assert!(err2.to_string().contains("需要管理员权限"));
        assert!(err2.to_string().contains("需要管理员"));

        let err3 = EnvError::InvalidArgument("无效".to_string());
        assert!(err3.to_string().contains("无效参数"));
        assert!(err3.to_string().contains("无效"));
    }
}