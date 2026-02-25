//! unset 命令处理器

use super::{CommandContext, CommandHandler};
use crate::application::services::EnvService;
use crate::domain::error::Result;
use crate::domain::models::EnvSource;
use async_trait::async_trait;
use std::sync::Arc;

/// unset 命令
pub struct UnsetCommand {
    env_service: Arc<EnvService>,
    key: String,
    source: EnvSource,
}

impl UnsetCommand {
    pub fn new(env_service: Arc<EnvService>, key: String, source: EnvSource) -> Self {
        Self {
            env_service,
            key,
            source,
        }
    }
}

#[async_trait]
impl CommandHandler for UnsetCommand {
    async fn execute(&self, ctx: &CommandContext) -> Result<()> {
        let deleted = self.env_service.unset(&self.key, &self.source).await?;

        if deleted {
            if ctx.verbose {
                println!("✓ 已删除变量: {}", self.key);
            }
            Ok(())
        } else {
            Err(crate::domain::error::DomainError::NotFound(
                self.key.clone(),
            ))
        }
    }
}
