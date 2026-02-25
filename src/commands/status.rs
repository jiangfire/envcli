//! status 命令处理器

use super::{CommandContext, CommandHandler};
use crate::application::services::EnvService;
use crate::domain::error::Result;
use crate::domain::models::EnvSource;
use crate::infrastructure::paths;
use async_trait::async_trait;
use std::sync::Arc;

/// status 命令
pub struct StatusCommand {
    env_service: Arc<EnvService>,
}

impl StatusCommand {
    pub fn new(env_service: Arc<EnvService>) -> Self {
        Self { env_service }
    }
}

#[async_trait]
impl CommandHandler for StatusCommand {
    async fn execute(&self, ctx: &CommandContext) -> Result<()> {
        // 配置目录
        let config_dir = paths::get_config_dir()?;
        println!("配置目录: {}", config_dir.display());

        // 各层级状态
        for source in [EnvSource::User, EnvSource::Project, EnvSource::Local] {
            let path = paths::get_layer_path(&source)?;
            let exists = path.exists();
            let status = if exists { "存在" } else { "不存在" };

            let count = if exists {
                match self.env_service.list(Some(source)).await {
                    Ok(vars) => vars.len(),
                    Err(_) => 0,
                }
            } else {
                0
            };

            println!(
                "  {}/{}: {} [{} 个变量]",
                source,
                path.display(),
                status,
                count
            );
        }

        // 合并后的变量总数
        let all_vars = self.env_service.list(None).await?;
        println!("\n合并后总计: {} 个变量", all_vars.len());

        if ctx.verbose && !all_vars.is_empty() {
            println!("\n当前所有变量:");
            for var in &all_vars {
                println!("  {} = {} (来自 {})", var.key, var.value, var.source);
            }
        }

        Ok(())
    }
}
