//! list 命令处理器

use super::{CommandContext, CommandHandler};
use crate::application::services::EnvService;
use crate::domain::error::Result;
use crate::domain::models::{EnvSource, OutputFormat};
use async_trait::async_trait;
use std::sync::Arc;

/// list 命令
pub struct ListCommand {
    env_service: Arc<EnvService>,
    source: Option<EnvSource>,
    format: OutputFormat,
}

impl ListCommand {
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
impl CommandHandler for ListCommand {
    async fn execute(&self, _ctx: &CommandContext) -> Result<()> {
        let vars = self.env_service.list(self.source).await?;

        match self.format {
            OutputFormat::Env => {
                for var in &vars {
                    println!("{}={}", var.key, var.value);
                }
            }
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&vars)?);
            }
        }

        Ok(())
    }
}
