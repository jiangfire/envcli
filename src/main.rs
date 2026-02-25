//! EnvCLI 主程序入口
//!
//! 重构后的入口点，使用 Clean Architecture

use clap::Parser;
use envcli::app::{AppConfig, Application};
use envcli::cli::{self, CacheCommands, Cli, Commands, ConfigCommands};
use envcli::commands::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化 tracing
    tracing_subscriber::fmt::init();

    // 解析 CLI 参数
    let cli = Cli::parse();

    // 创建应用配置
    let app_config = AppConfig {
        verbose: cli.verbose,
        ..Default::default()
    };

    // 初始化应用程序
    let app = Application::new(app_config.clone()).await?;

    // 创建命令上下文
    let ctx = CommandContext {
        verbose: cli.verbose,
    };

    // 执行命令
    let result = match cli.command {
        // 核心命令
        Commands::Get { key } => {
            let cmd = get::GetCommand::new(app.env_service.clone(), key);
            cmd.execute(&ctx).await
        }

        Commands::Set { key, value, target } => {
            let source = cli::parse_writable_source(&target)?;
            let cmd = set::SetCommand::new(app.env_service.clone(), key, value, source);
            cmd.execute(&ctx).await
        }

        Commands::Unset { key, target } => {
            let source = cli::parse_writable_source(&target)?;
            let cmd = unset::UnsetCommand::new(app.env_service.clone(), key, source);
            cmd.execute(&ctx).await
        }

        Commands::List { source, format } => {
            let source = cli::parse_source(source.as_deref());
            let format = format.as_str().into();
            let cmd = list::ListCommand::new(app.env_service.clone(), source, format);
            cmd.execute(&ctx).await
        }

        Commands::Export { source, format } => {
            let source = cli::parse_source(source.as_deref());
            let format = format.as_str().into();
            let cmd = export::ExportCommand::new(app.env_service.clone(), source, format);
            cmd.execute(&ctx).await
        }

        Commands::Import { file, target } => {
            let source = cli::parse_writable_source(&target)?;
            let cmd = import::ImportCommand::new(
                app.env_service.clone(),
                std::path::PathBuf::from(file),
                source,
            );
            cmd.execute(&ctx).await
        }

        Commands::Run {
            env,
            from_file,
            command,
        } => {
            let cmd = run::RunCommand::new(app.env_service.clone(), env, from_file, command);
            cmd.execute(&ctx).await
        }

        Commands::Status => {
            let cmd = status::StatusCommand::new(app.env_service.clone());
            cmd.execute(&ctx).await
        }

        Commands::Doctor => {
            let cmd = doctor::DoctorCommand::new(app.env_service.clone());
            cmd.execute(&ctx).await
        }

        // 系统命令
        Commands::SystemSet { key, value, scope } => {
            let cmd = system::SystemSetCommand::new(key, value, scope);
            cmd.execute(&ctx).await
        }

        Commands::SystemUnset { key, scope } => {
            let cmd = system::SystemUnsetCommand::new(key, scope);
            cmd.execute(&ctx).await
        }

        // 缓存命令
        Commands::Cache(cache_cmd) => match cache_cmd {
            CacheCommands::Stats => {
                let cmd = cache::CacheStatsCommand::new();
                cmd.execute(&ctx).await
            }

            CacheCommands::Clear { cache_type } => {
                let cmd = cache::CacheClearCommand::new(app.env_service.clone(), cache_type);
                cmd.execute(&ctx).await
            }
        },

        // 配置命令
        Commands::Config(config_cmd) => match config_cmd {
            ConfigCommands::Validate { verbose } => {
                let cmd = config::ConfigValidateCommand::new(verbose);
                cmd.execute(&ctx).await
            }

            ConfigCommands::Init { force } => {
                let cmd = config::ConfigInitCommand::new(force);
                cmd.execute(&ctx).await
            }

            ConfigCommands::Info => {
                let cmd = config::ConfigInfoCommand::new();
                cmd.execute(&ctx).await
            }
        },
    };

    // 处理错误
    if let Err(e) = result {
        eprintln!("错误: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
