//! cache 命令处理器

use super::{CommandContext, CommandHandler};
use crate::application::services::EnvService;
use crate::domain::error::Result;
use crate::infrastructure::paths;
use async_trait::async_trait;
use std::sync::Arc;

/// cache stats 命令
pub struct CacheStatsCommand;

impl Default for CacheStatsCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheStatsCommand {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for CacheStatsCommand {
    async fn execute(&self, _ctx: &CommandContext) -> Result<()> {
        let (cached, age) = paths::get_system_env_cache_stats();

        println!("📋 缓存统计信息\n");
        println!("系统环境缓存:");
        if cached {
            println!("  状态: ✓ 已缓存");
            println!("  存在时间: {:?}", age);
        } else {
            println!("  状态: ✗ 未缓存");
        }

        Ok(())
    }
}

/// cache clear 命令
pub struct CacheClearCommand {
    env_service: Arc<EnvService>,
    cache_type: String,
}

impl CacheClearCommand {
    pub fn new(env_service: Arc<EnvService>, cache_type: String) -> Self {
        Self {
            env_service,
            cache_type,
        }
    }
}

#[async_trait]
impl CommandHandler for CacheClearCommand {
    async fn execute(&self, ctx: &CommandContext) -> Result<()> {
        match self.cache_type.as_str() {
            "file" => {
                self.env_service.clear_cache().await;
                if ctx.verbose {
                    println!("✓ 文件缓存已清除");
                }
            }
            "system" => {
                paths::clear_system_env_cache();
                if ctx.verbose {
                    println!("✓ 系统环境缓存已清除");
                }
            }
            "all" => {
                self.env_service.clear_cache().await;
                paths::clear_system_env_cache();
                if ctx.verbose {
                    println!("✓ 所有缓存已清除");
                }
            }
            _ => {
                return Err(crate::domain::error::DomainError::InvalidArgument(
                    "缓存类型必须是: file/system/all".to_string(),
                ));
            }
        }
        Ok(())
    }
}
