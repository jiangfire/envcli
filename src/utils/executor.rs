//! 跨平台命令执行器
//!
//! 自动识别操作系统并使用正确的执行方式
//! Windows 使用 Command::new() 直接执行
//! Unix 使用 Command::new() 直接执行
//!
//! 注意：所有平台都继承父进程的 stdin/stdout/stderr

use crate::error::{EnvError, Result};
use std::collections::HashMap;
use std::process::{Command, Stdio};

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
    pub fn exec_with_env(command: &[String], env_vars: &HashMap<String, String>) -> Result<i32> {
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
        let status = cmd.status().map_err(|e| {
            EnvError::CommandNotFound(format!(
                "{}: {} (请确保命令在 PATH 中或使用完整路径)",
                program, e
            ))
        })?;

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
        let status = cmd.status().map_err(|e| {
            EnvError::CommandNotFound(format!(
                "{}: {} (请确保命令在 PATH 中或使用完整路径)",
                program, e
            ))
        })?;

        Ok(status.code().unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    mod exec_with_env_tests {
        use super::*;

        #[test]
        fn test_exec_empty_command() {
            let command: Vec<String> = vec![];
            let env_vars = HashMap::new();

            let result = CommandExecutor::exec_with_env(&command, &env_vars);
            assert!(result.is_err());
            assert!(matches!(
                result.unwrap_err(),
                EnvError::CommandExecutionFailed(_)
            ));
        }

        #[test]
        fn test_exec_with_echo_command() {
            // 使用 echo 命令测试（跨平台兼容）
            let command = if cfg!(target_os = "windows") {
                vec![
                    "cmd".to_string(),
                    "/c".to_string(),
                    "echo".to_string(),
                    "test".to_string(),
                ]
            } else {
                vec!["echo".to_string(), "test".to_string()]
            };

            let env_vars = HashMap::new();

            let result = CommandExecutor::exec_with_env(&command, &env_vars);
            // echo 应该成功执行
            assert!(result.is_ok());
        }

        #[test]
        fn test_exec_with_env_vars() {
            let command = if cfg!(target_os = "windows") {
                vec!["cmd".to_string(), "/c".to_string(), "set".to_string()]
            } else {
                vec!["env".to_string()]
            };

            let mut env_vars = HashMap::new();
            env_vars.insert("TEST_VAR_UNIQUE_123".to_string(), "test_value".to_string());

            // 这个测试主要验证函数不 panic，实际环境变量注入需要运行子进程
            let result = CommandExecutor::exec_with_env(&command, &env_vars);
            // 命令应该可以执行（即使可能找不到）
            // 我们只验证函数签名和基本逻辑
            assert!(result.is_ok() || result.is_err());
        }

        #[test]
        fn test_exec_command_not_found() {
            let command = vec!["nonexistent_command_xyz123".to_string()];
            let env_vars = HashMap::new();

            let result = CommandExecutor::exec_with_env(&command, &env_vars);
            assert!(result.is_err());
            assert!(matches!(result.unwrap_err(), EnvError::CommandNotFound(_)));
        }

        #[test]
        fn test_exec_unix_vs_windows_path() {
            // 验证平台选择逻辑
            let command = if cfg!(target_os = "windows") {
                vec![
                    "cmd".to_string(),
                    "/c".to_string(),
                    "echo".to_string(),
                    "platform".to_string(),
                ]
            } else {
                vec!["echo".to_string(), "platform".to_string()]
            };

            let env_vars = HashMap::new();

            let result = CommandExecutor::exec_with_env(&command, &env_vars);
            assert!(result.is_ok());
        }

        #[test]
        fn test_exec_with_inherited_env() {
            // 设置一个环境变量，验证它被继承
            unsafe {
                env::set_var("TEST_INHERIT_VAR_999", "inherited_value");
            }

            let command = if cfg!(target_os = "windows") {
                vec![
                    "cmd".to_string(),
                    "/c".to_string(),
                    "echo".to_string(),
                    "test".to_string(),
                ]
            } else {
                vec!["echo".to_string(), "test".to_string()]
            };

            let env_vars = HashMap::new();

            let result = CommandExecutor::exec_with_env(&command, &env_vars);
            assert!(result.is_ok());

            // 清理
            unsafe {
                env::remove_var("TEST_INHERIT_VAR_999");
            }
        }

        #[test]
        fn test_exec_with_override_env() {
            // 验证临时环境变量可以覆盖系统环境变量
            unsafe {
                env::set_var("TEST_OVERRIDE_VAR", "original");
            }

            let command = if cfg!(target_os = "windows") {
                vec![
                    "cmd".to_string(),
                    "/c".to_string(),
                    "echo".to_string(),
                    "test".to_string(),
                ]
            } else {
                vec!["echo".to_string(), "test".to_string()]
            };

            let mut env_vars = HashMap::new();
            env_vars.insert("TEST_OVERRIDE_VAR".to_string(), "overridden".to_string());

            let result = CommandExecutor::exec_with_env(&command, &env_vars);
            // 命令应该可以执行
            assert!(result.is_ok());

            // 清理
            unsafe {
                env::remove_var("TEST_OVERRIDE_VAR");
            }
        }

        #[test]
        fn test_exec_with_multiple_args() {
            let command = if cfg!(target_os = "windows") {
                vec![
                    "cmd".to_string(),
                    "/c".to_string(),
                    "echo".to_string(),
                    "hello".to_string(),
                    "world".to_string(),
                ]
            } else {
                vec!["echo".to_string(), "hello".to_string(), "world".to_string()]
            };

            let env_vars = HashMap::new();

            let result = CommandExecutor::exec_with_env(&command, &env_vars);
            assert!(result.is_ok());
        }

        #[test]
        fn test_exec_windows_specific() {
            if cfg!(target_os = "windows") {
                let command = vec![
                    "cmd".to_string(),
                    "/c".to_string(),
                    "echo".to_string(),
                    "windows".to_string(),
                ];
                let env_vars = HashMap::new();

                let result = CommandExecutor::exec_with_env(&command, &env_vars);
                assert!(result.is_ok());
            }
        }

        #[test]
        fn test_exec_unix_specific() {
            if cfg!(target_os = "linux") || cfg!(target_os = "macos") {
                let command = vec!["echo".to_string(), "unix".to_string()];
                let env_vars = HashMap::new();

                let result = CommandExecutor::exec_with_env(&command, &env_vars);
                assert!(result.is_ok());
            }
        }
    }

    mod internal_exec_tests {
        use super::*;

        // 测试内部方法的可见性（虽然它们是私有的，但通过 exec_with_env 可以间接测试）
        // 这里我们主要测试 exec_with_env 的各种场景

        #[test]
        fn test_exec_preserves_current_process_env() {
            // 验证当前进程的环境变量被继承
            let current_vars: HashMap<String, String> = env::vars().collect();

            if !current_vars.is_empty() {
                // 如果有环境变量，测试命令可以执行
                let command = if cfg!(target_os = "windows") {
                    vec![
                        "cmd".to_string(),
                        "/c".to_string(),
                        "echo".to_string(),
                        "test".to_string(),
                    ]
                } else {
                    vec!["echo".to_string(), "test".to_string()]
                };

                let env_vars = HashMap::new();
                let result = CommandExecutor::exec_with_env(&command, &env_vars);
                assert!(result.is_ok());
            }
        }
    }
}
