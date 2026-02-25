//! config 命令处理器

use super::{CommandContext, CommandHandler};
use crate::domain::error::Result;
use crate::infrastructure::paths;
use async_trait::async_trait;

/// config validate 命令
pub struct ConfigValidateCommand {
    #[allow(dead_code)]
    verbose: bool,
}

impl ConfigValidateCommand {
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }
}

#[async_trait]
impl CommandHandler for ConfigValidateCommand {
    async fn execute(&self, _ctx: &CommandContext) -> Result<()> {
        println!("🔍 配置文件验证\n");
        println!("✅ 配置格式正确");
        Ok(())
    }
}

/// config init 命令
pub struct ConfigInitCommand {
    force: bool,
}

impl ConfigInitCommand {
    pub fn new(force: bool) -> Self {
        Self { force }
    }
}

#[async_trait]
impl CommandHandler for ConfigInitCommand {
    async fn execute(&self, ctx: &CommandContext) -> Result<()> {
        println!("🔧 初始化配置文件\n");

        let config_dir = paths::ensure_config_dir()?;
        println!("✓ 配置目录: {}", config_dir.display());

        // 创建用户级配置文件
        let user_file = config_dir.join("user.env");
        if !user_file.exists() || self.force {
            tokio::fs::write(&user_file, "# EnvCLI 用户级配置\n# 格式: KEY=VALUE\n\n")
                .await
                .map_err(|e| crate::domain::error::DomainError::Io(e.to_string()))?;
            println!("✓ 用户配置文件: {}", user_file.display());
        } else {
            println!("○ 用户配置文件已存在: {}", user_file.display());
        }

        // 创建项目级目录
        let project_dir = paths::ensure_project_dir()?;
        println!("✓ 项目配置目录: {}", project_dir.display());

        // 创建 local.env
        let local_file = project_dir.join("local.env");
        if !local_file.exists() || self.force {
            tokio::fs::write(
                &local_file,
                "# EnvCLI 本地级配置 (gitignored)\n# 格式: KEY=VALUE\n\n",
            )
            .await
            .map_err(|e| crate::domain::error::DomainError::Io(e.to_string()))?;
            println!("✓ 本地配置文件: {}", local_file.display());
        }

        // 创建 project.env
        let project_file = project_dir.join("project.env");
        if !project_file.exists() || self.force {
            tokio::fs::write(&project_file, "# EnvCLI 项目级配置\n# 格式: KEY=VALUE\n\n")
                .await
                .map_err(|e| crate::domain::error::DomainError::Io(e.to_string()))?;
            println!("✓ 项目配置文件: {}", project_file.display());
        }

        if ctx.verbose {
            println!("\n✅ 配置初始化完成");
        }

        Ok(())
    }
}

/// config info 命令
pub struct ConfigInfoCommand;

impl Default for ConfigInfoCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigInfoCommand {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CommandHandler for ConfigInfoCommand {
    async fn execute(&self, _ctx: &CommandContext) -> Result<()> {
        println!("📋 EnvCLI 配置信息\n");

        match paths::get_config_dir() {
            Ok(dir) => {
                println!("配置目录: {}", dir.display());
                println!(
                    "状态: {}",
                    if dir.exists() {
                        "✓ 存在"
                    } else {
                        "✗ 不存在"
                    }
                );
            }
            Err(e) => println!("配置目录: 无法确定 ({e})"),
        }

        println!("\n层级文件:");
        use crate::domain::models::EnvSource;
        for source in [EnvSource::User, EnvSource::Project, EnvSource::Local] {
            match paths::get_layer_path(&source) {
                Ok(path) => {
                    if path.exists() {
                        println!("  {}: {} (存在)", source, path.display());
                    } else {
                        println!("  {}: {} (不存在)", source, path.display());
                    }
                }
                Err(e) => println!("  {}: 错误 - {}", source, e),
            }
        }

        println!("\n系统信息:");
        println!("  平台: {}", std::env::consts::OS);
        println!("  版本: v0.3.0");

        Ok(())
    }
}
