//! get 命令处理器

use super::{CommandContext, CommandHandler};
use crate::application::services::EnvService;
use crate::domain::error::Result;
use async_trait::async_trait;
use std::sync::Arc;

/// get 命令
pub struct GetCommand {
    env_service: Arc<EnvService>,
    key: String,
}

impl GetCommand {
    pub fn new(env_service: Arc<EnvService>, key: String) -> Self {
        Self { env_service, key }
    }
}

#[async_trait]
impl CommandHandler for GetCommand {
    async fn execute(&self, _ctx: &CommandContext) -> Result<()> {
        match self.env_service.get(&self.key).await? {
            Some(value) => {
                println!("{}", value);
                Ok(())
            }
            None => Err(crate::domain::error::DomainError::NotFound(
                self.key.clone(),
            )),
        }
    }
}
