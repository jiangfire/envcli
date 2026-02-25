//! import 命令处理器

use super::{CommandContext, CommandHandler};
use crate::application::services::EnvService;
use crate::domain::error::Result;
use crate::domain::models::EnvSource;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

/// import 命令
pub struct ImportCommand {
    env_service: Arc<EnvService>,
    file: PathBuf,
    target: EnvSource,
}

impl ImportCommand {
    pub fn new(env_service: Arc<EnvService>, file: PathBuf, target: EnvSource) -> Self {
        Self {
            env_service,
            file,
            target,
        }
    }
}

#[async_trait]
impl CommandHandler for ImportCommand {
    async fn execute(&self, ctx: &CommandContext) -> Result<()> {
        let count = self.env_service.import(&self.file, self.target).await?;

        if ctx.verbose {
            println!(
                "✓ 从 {} 导入了 {} 个变量到 {:?}",
                self.file.display(),
                count,
                self.target
            );
        }

        Ok(())
    }
}
