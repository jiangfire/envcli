//! 命令处理器
//!
//! 每个命令一个模块，实现 CommandHandler trait

use crate::domain::error::Result;
use async_trait::async_trait;

pub mod cache;
pub mod config;
pub mod doctor;
pub mod export;
pub mod get;
pub mod import;
pub mod list;
pub mod run;
pub mod set;
pub mod status;
pub mod system;
pub mod unset;

/// 命令上下文
#[derive(Debug)]
pub struct CommandContext {
    pub verbose: bool,
}

/// 命令处理器 trait
#[async_trait]
pub trait CommandHandler: Send + Sync {
    /// 执行命令
    async fn execute(&self, ctx: &CommandContext) -> Result<()>;
}

/// 命令输出
pub trait CommandOutput {
    /// 打印结果
    fn print(&self);
}

impl CommandOutput for String {
    fn print(&self) {
        println!("{}", self);
    }
}

impl<T: std::fmt::Display> CommandOutput for Vec<T> {
    fn print(&self) {
        for item in self {
            println!("{}", item);
        }
    }
}
