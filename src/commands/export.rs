//! export 命令处理器

use super::{CommandContext, CommandHandler};
use crate::application::services::EnvService;
use crate::domain::error::Result;
use crate::domain::models::{EnvSource, OutputFormat};
use async_trait::async_trait;
use std::sync::Arc;

/// export 命令
pub struct ExportCommand {
    env_service: Arc<EnvService>,
    source: Option<EnvSource>,
    format: OutputFormat,
}

impl ExportCommand {
    pub fn new(
        env_service: Arc<EnvService>,
        source: Option<EnvSource>,
        format: OutputFormat,
    ) -> Self {
        Self {
            env_service,
            source,
            format,
        }
    }
}

#[async_trait]
impl CommandHandler for ExportCommand {
    async fn execute(&self, _ctx: &CommandContext) -> Result<()> {
        let output = self
            .env_service
            .export(self.source, self.format.clone())
            .await?;
        println!("{}", output);
        Ok(())
    }
}
