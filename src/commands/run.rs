//! run 命令处理器

use super::{CommandContext, CommandHandler};
use crate::application::services::EnvService;
use crate::domain::error::{DomainError, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// run 命令
pub struct RunCommand {
    env_service: Arc<EnvService>,
    temp_vars: Vec<String>,
    from_file: Option<String>,
    command: Vec<String>,
}

impl RunCommand {
    pub fn new(
        env_service: Arc<EnvService>,
        temp_vars: Vec<String>,
        from_file: Option<String>,
        command: Vec<String>,
    ) -> Self {
        Self {
            env_service,
            temp_vars,
            from_file,
            command,
        }
    }

    /// 解析临时变量 KEY=VALUE
    fn parse_temp_var(s: &str) -> Result<(String, String)> {
        let pos = s
            .find('=')
            .ok_or_else(|| DomainError::EnvParse(format!("无效的环境变量格式: {}", s)))?;

        let key = s[..pos].to_string();
        let value = s[pos + 1..].to_string();

        Ok((key, value))
    }
}

#[async_trait]
impl CommandHandler for RunCommand {
    async fn execute(&self, _ctx: &CommandContext) -> Result<()> {
        // 1. 获取所有存储的环境变量
        let mut env_vars: HashMap<String, String> = self
            .env_service
            .list(None)
            .await?
            .into_iter()
            .map(|v| (v.key, v.value))
            .collect();

        // 2. 从文件加载变量
        if let Some(file) = &self.from_file {
            let content = tokio::fs::read_to_string(file)
                .await
                .map_err(|e| DomainError::Io(format!("读取文件失败: {}", e)))?;

            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }

                if let Some(pos) = trimmed.find('=') {
                    let key = trimmed[..pos].to_string();
                    let value = trimmed[pos + 1..].to_string();
                    env_vars.insert(key, value);
                }
            }
        }

        // 3. 应用临时变量（最高优先级）
        for var in &self.temp_vars {
            let (key, value) = Self::parse_temp_var(var)?;
            env_vars.insert(key, value);
        }

        // 4. 执行命令
        if self.command.is_empty() {
            return Err(DomainError::InvalidArgument(
                "未指定要执行的命令".to_string(),
            ));
        }

        let program = &self.command[0];
        let args = &self.command[1..];

        let mut cmd = tokio::process::Command::new(program);
        cmd.args(args);

        // 设置环境变量
        for (key, value) in &env_vars {
            cmd.env(key, value);
        }

        // 执行并传递退出码
        let status = cmd
            .status()
            .await
            .map_err(|e| DomainError::CommandExecutionFailed(e.to_string()))?;

        let code = status.code().unwrap_or(1);
        std::process::exit(code);
    }
}
