//! 跨平台命令执行器
//!
//! 自动识别操作系统并使用正确的执行方式
//! Windows 使用 Command::new() 直接执行
//! Unix 使用 Command::new() 直接执行
//!
//! 注意：所有平台都继承父进程的 stdin/stdout/stderr

use std::process::{Command, Stdio};
use std::collections::HashMap;
use crate::error::{EnvError, Result};

/// 跨平台命令执行器
pub struct CommandExecutor;

impl CommandExecutor {
    /// 执行命令并注入环境变量
    ///
    /// # 参数
    /// - `command`: 命令和参数，如 `["python", "app.py"]`
    /// - `env_vars`: 要注入的环境变量
    ///
    /// # 返回
    /// 子进程的退出码
    pub fn exec_with_env(
        command: &[String],
        env_vars: &HashMap<String, String>,
    ) -> Result<i32> {
        if command.is_empty() {
            return Err(EnvError::CommandExecutionFailed("命令不能为空".to_string()));
        }

        let (program, args) = command.split_first().unwrap();

        // 根据平台选择执行策略
        if cfg!(target_os = "windows") {
            Self::exec_windows(program, args, env_vars)
        } else {
            Self::exec_unix(program, args, env_vars)
        }
    }

    /// Windows 实现
    fn exec_windows(
        program: &str,
        args: &[String],
        env_vars: &HashMap<String, String>,
    ) -> Result<i32> {
        let mut cmd = Command::new(program);
        cmd.args(args);

        // 继承当前进程的环境变量
        for (key, value) in std::env::vars() {
            cmd.env(key, value);
        }

        // 注入临时环境变量（覆盖继承的变量）
        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        // 继承标准流
        cmd.stdin(Stdio::inherit())
           .stdout(Stdio::inherit())
           .stderr(Stdio::inherit());

        // 执行并等待
        let status = cmd.status()
            .map_err(|e| EnvError::CommandNotFound(
                format!("{}: {} (请确保命令在 PATH 中或使用完整路径)", program, e)
            ))?;

        Ok(status.code().unwrap_or(0))
    }

    /// Unix 实现 (Linux/macOS)
    fn exec_unix(
        program: &str,
        args: &[String],
        env_vars: &HashMap<String, String>,
    ) -> Result<i32> {
        let mut cmd = Command::new(program);
        cmd.args(args);

        // 继承当前进程的环境变量
        for (key, value) in std::env::vars() {
            cmd.env(key, value);
        }

        // 注入临时环境变量
        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        // 继承标准流
        cmd.stdin(Stdio::inherit())
           .stdout(Stdio::inherit())
           .stderr(Stdio::inherit());

        // 执行并等待
        let status = cmd.status()
            .map_err(|e| EnvError::CommandNotFound(
                format!("{}: {} (请确保命令在 PATH 中或使用完整路径)", program, e)
            ))?;

        Ok(status.code().unwrap_or(0))
    }
}
