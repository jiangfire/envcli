//! system-set/system-unset 命令处理器

use super::{CommandContext, CommandHandler};
use crate::domain::error::{DomainError, Result};
use async_trait::async_trait;

/// system-set 命令
pub struct SystemSetCommand {
    key: String,
    value: String,
    scope: String,
}

impl SystemSetCommand {
    pub fn new(key: String, value: String, scope: String) -> Self {
        Self { key, value, scope }
    }
}

#[async_trait]
impl CommandHandler for SystemSetCommand {
    async fn execute(&self, _ctx: &CommandContext) -> Result<()> {
        if self.scope != "global" && self.scope != "machine" {
            return Err(DomainError::InvalidArgument(
                "scope 必须是 'global' 或 'machine'".to_string(),
            ));
        }

        #[cfg(windows)]
        {
            use winreg::RegKey;
            use winreg::enums::*;

            let hkcu = RegKey::predef(HKEY_CURRENT_USER);
            let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

            let (key, reg_path) = if self.scope == "machine" {
                (
                    hklm,
                    "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
                )
            } else {
                (hkcu, "Environment")
            };

            let env = key
                .open_subkey_with_flags(reg_path, KEY_WRITE)
                .map_err(|e| DomainError::SystemEnvWriteFailed(e.to_string()))?;

            env.set_value(&self.key, &self.value)
                .map_err(|e| DomainError::SystemEnvWriteFailed(e.to_string()))?;
        }

        #[cfg(not(windows))]
        {
            if self.scope == "machine" {
                return Err(DomainError::PermissionDenied(
                    "Unix 系统不支持机器级环境变量".to_string(),
                ));
            }

            // Unix: 写入 shell 配置文件
            let home = dirs::home_dir()
                .ok_or_else(|| DomainError::Config("无法获取主目录".to_string()))?;
            let profile = home.join(".bashrc");

            let content = format!("\nexport {}=\"{}\"\n", self.key, self.value);
            tokio::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(&profile)
                .await
                .map_err(|e| DomainError::Io(e.to_string()))?;

            tokio::fs::write(&profile, content)
                .await
                .map_err(|e| DomainError::Io(e.to_string()))?;
        }

        Ok(())
    }
}

/// system-unset 命令
pub struct SystemUnsetCommand {
    key: String,
    scope: String,
}

impl SystemUnsetCommand {
    pub fn new(key: String, scope: String) -> Self {
        Self { key, scope }
    }
}

#[async_trait]
impl CommandHandler for SystemUnsetCommand {
    async fn execute(&self, _ctx: &CommandContext) -> Result<()> {
        if self.scope != "global" && self.scope != "machine" {
            return Err(DomainError::InvalidArgument(
                "scope 必须是 'global' 或 'machine'".to_string(),
            ));
        }

        #[cfg(windows)]
        {
            use winreg::RegKey;
            use winreg::enums::*;

            let hkcu = RegKey::predef(HKEY_CURRENT_USER);
            let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

            let (key, reg_path) = if self.scope == "machine" {
                (
                    hklm,
                    "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment",
                )
            } else {
                (hkcu, "Environment")
            };

            let env = key
                .open_subkey_with_flags(reg_path, KEY_WRITE)
                .map_err(|e| DomainError::SystemEnvWriteFailed(e.to_string()))?;

            env.delete_value(&self.key)
                .map_err(|e| DomainError::SystemEnvWriteFailed(e.to_string()))?;
        }

        #[cfg(not(windows))]
        {
            // Unix: 从 shell 配置文件中移除（简化处理）
            println!(
                "⚠️ Unix 系统请手动从 ~/.bashrc 或 ~/.zshrc 中移除 {} 变量",
                self.key
            );
        }

        Ok(())
    }
}
