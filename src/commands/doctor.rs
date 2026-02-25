//! doctor 命令处理器

use super::{CommandContext, CommandHandler};
use crate::application::services::EnvService;
use crate::domain::error::Result;
use crate::domain::models::EnvSource;
use crate::infrastructure::paths;
use async_trait::async_trait;
use std::sync::Arc;

/// doctor 命令
pub struct DoctorCommand {
    env_service: Arc<EnvService>,
}

impl DoctorCommand {
    pub fn new(env_service: Arc<EnvService>) -> Self {
        Self { env_service }
    }
}

#[async_trait]
impl CommandHandler for DoctorCommand {
    async fn execute(&self, _ctx: &CommandContext) -> Result<()> {
        println!("🔍 EnvCLI 健康诊断工具\n");
        println!("版本: v0.2.0 | 平台: {}", std::env::consts::OS);
        println!("──────────────────────────────────────────────\n");

        let mut issues = 0;
        let mut warnings = 0;

        // 1. 检查配置目录
        println!("📁 1. 配置目录检查");
        match paths::get_config_dir() {
            Ok(dir) => {
                if !dir.exists() {
                    println!("   ❌ 配置目录不存在: {}", dir.display());
                    issues += 1;
                } else {
                    println!("   ✓ 配置目录存在: {}", dir.display());
                }
            }
            Err(e) => {
                println!("   ❌ 无法确定配置目录: {}", e);
                issues += 1;
            }
        }
        println!();

        // 2. 检查层级文件
        println!("📄 2. 配置文件状态");
        for source in [EnvSource::User, EnvSource::Project, EnvSource::Local] {
            match paths::get_layer_path(&source) {
                Ok(path) => {
                    if path.exists() {
                        println!("   ✓ {}: {}", source, path.display());
                    } else {
                        println!("   ○ {}: 不存在", source);
                    }
                }
                Err(e) => {
                    println!("   ❌ {}: {}", source, e);
                    issues += 1;
                }
            }
        }
        println!();

        // 3. 检查变量冲突
        println!("🔄 3. 变量冲突检查");
        match self.env_service.check_conflicts().await {
            Ok(conflicts) => {
                if conflicts.is_empty() {
                    println!("   ✓ 无变量冲突");
                } else {
                    for (key, sources) in conflicts.iter().take(5) {
                        println!("   ⚠️  {} 在 {} 层定义", key, sources.len());
                    }
                    if conflicts.len() > 5 {
                        println!("   ... 还有 {} 个冲突", conflicts.len() - 5);
                    }
                    warnings += conflicts.len();
                }
            }
            Err(e) => {
                println!("   ❌ 检查失败: {}", e);
                issues += 1;
            }
        }
        println!();

        // 4. 系统环境
        println!("🖥️ 4. 系统环境");
        match paths::get_system_env() {
            Ok(vars) => {
                println!("   系统变量数: {}", vars.len());
            }
            Err(e) => {
                println!("   ❌ 无法读取: {}", e);
                issues += 1;
            }
        }
        println!();

        // 总结
        println!("──────────────────────────────────────────────");
        if issues == 0 && warnings == 0 {
            println!("✅ 所有检查通过，系统健康！");
        } else {
            if issues > 0 {
                println!("❌ 发现 {} 个问题需要修复", issues);
            }
            if warnings > 0 {
                println!("⚠️  发现 {} 个警告", warnings);
            }
        }

        Ok(())
    }
}
