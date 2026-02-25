//! set 命令处理器

use super::{CommandContext, CommandHandler};
use crate::application::services::EnvService;
use crate::domain::error::Result;
use crate::domain::models::EnvSource;
use async_trait::async_trait;
use std::sync::Arc;

/// set 命令
pub struct SetCommand {
    env_service: Arc<EnvService>,
    key: String,
    value: String,
    source: EnvSource,
}

impl SetCommand {
    pub fn new(
        env_service: Arc<EnvService>,
        key: String,
        value: String,
        source: EnvSource,
    ) -> Self {
        Self {
            env_service,
            key,
            value,
            source,
        }
    }
}

#[async_trait]
impl CommandHandler for SetCommand {
    async fn execute(&self, ctx: &CommandContext) -> Result<()> {
        self.env_service
            .set(&self.key, &self.value, self.source)
            .await?;
        if ctx.verbose {
            println!("✓ 已设置变量 {} = {}", self.key, self.value);
        }
        Ok(())
    }
}
