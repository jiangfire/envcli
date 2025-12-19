//! EnvCLI 主程序入口
//!
//! 设计原则：
//! - 模块化：入口代码简洁，逻辑委托给各模块
//! - 安静模式：默认无输出，成功静默
//! - 错误处理：详细/安静错误模式，通过 --verbose 切换

mod types;
mod error;
mod utils;
mod config;
mod core;
mod cli;

use clap::Parser;
use types::{Config, EnvSource, OutputFormat};
use error::{EnvError, Result};
use core::Store;
use cli::{Cli, Commands};

fn main() {
    // 解析 CLI 参数
    let cli = Cli::parse();

    // 配置初始化
    let config = match init_config(&cli) {
        Ok(cfg) => cfg,
        Err(e) => {
            // 配置初始化失败，使用默认配置 + 详细输出
            eprintln!("配置初始化失败: {}", e);
            std::process::exit(1);
        }
    };

    // 创建存储引擎
    let store = Store::new(config.clone());

    // 执行命令，统一错误处理
    let result = run_command(cli.command, store, config.verbose);

    match result {
        Ok(_) => {
            // 静默成功 - 符合安静原则
            // 某些命令已经有自己的输出，这里不重复
        }
        Err(e) => {
            e.report(config.verbose);
            std::process::exit(1);
        }
    }
}

/// 初始化配置
fn init_config(cli: &Cli) -> Result<Config> {
    Ok(Config {
        verbose: cli.verbose,
    })
}

/// 运行具体命令
fn run_command(command: Commands, store: Store, verbose: bool) -> Result<()> {
    match command {
        // 读取系列
        Commands::Get { key } => {
            match store.get(&key)? {
                Some(value) => println!("{}", value),
                None => {
                    return Err(EnvError::NotFound(key));
                }
            }
        }

        // 写入系列
        Commands::Set { key, value } => store.set(key, value)?,

        Commands::Unset { key } => {
            let deleted = store.unset(&key)?;
            if verbose && deleted {
                println!("✓ 已删除");
            } else if !deleted {
                return Err(EnvError::NotFound(key));
            }
        }

        // 列出系列
        Commands::List { source, format } => {
            let source_filter = cli::parse_list_source(source.as_deref())?;
            let output_format = cli::parse_format(&format);
            let vars = store.list(source_filter)?;

            match output_format {
                OutputFormat::ENV => {
                    for var in &vars {
                        println!("{}={}", var.key, var.value);
                    }
                }
                OutputFormat::JSON => {
                    let json = serde_json::to_string_pretty(&vars)?;
                    println!("{}", json);
                }
            }
        }

        // 导入系列
        Commands::Import { file, target } => {
            let target_source = cli::validate_writable_source(&target)?;
            let count = store.import_file(&file, &target_source)?;
            if verbose {
                println!("✓ 成功导入 {} 个变量", count);
            }
        }

        // 导出系列
        Commands::Export { source, format } => {
            let source_filter = cli::parse_list_source(source.as_deref())?;
            let output_format = cli::parse_format(&format);
            let content = store.export(source_filter.clone())?;

            match output_format {
                OutputFormat::ENV => println!("{}", content),
                OutputFormat::JSON => {
                    let vars = store.list(source_filter)?;
                    let json = serde_json::to_string_pretty(&vars)?;
                    println!("{}", json);
                }
            }
        }

        // 状态显示
        Commands::Status => {
            show_status(&store, verbose)?;
        }

        // 问题诊断
        Commands::Doctor => {
            diagnose(&store, verbose)?;
        }

        // 运行命令注入环境变量
        Commands::Run { env, from_file, command: cmd } => {
            // 1. 解析临时环境变量
            let mut temp_vars = utils::env_merge::EnvMerger::parse_temp_vars(&env)?;

            // 2. 如果指定了文件，从文件加载
            if let Some(file_path) = from_file {
                let file_vars = utils::env_merge::EnvMerger::parse_file(&file_path)?;
                temp_vars.extend(file_vars);
            }

            // 3. 构建完整环境（按优先级合并）
            let final_env = utils::env_merge::EnvMerger::merge_environment(&store, &temp_vars)?;

            // 4. 执行命令
            let exit_code = utils::executor::CommandExecutor::exec_with_env(&cmd, &final_env)?;

            // 5. 退出码透传
            std::process::exit(exit_code);
        }
    }

    Ok(())
}

/// 显示当前状态 (详细信息，但仍然保持简洁)
fn show_status(store: &Store, verbose: bool) -> Result<()> {
    // 配置目录
    let config_dir = utils::paths::get_config_dir()?;
    println!("配置目录: {}", config_dir.display());

    // 各层级状态
    for source in [
        EnvSource::User,
        EnvSource::Project,
        EnvSource::Local,
    ] {
        let path = utils::paths::get_layer_path(&source)?;
        let exists = utils::paths::file_exists(&path);

        let status = if exists { "存在" } else { "不存在" };
        let count = if exists {
            let vars = store.list(Some(source.clone()))?;
            vars.len()
        } else {
            0
        };

        println!("  {}/{}: {} [{} 个变量]", source, path.display(), status, count);
    }

    // 合并后的变量总数
    let all_vars = store.list(None)?;
    println!("\n合并后总计: {} 个变量", all_vars.len());

    if verbose && !all_vars.is_empty() {
        println!("\n当前所有变量:");
        for var in &all_vars {
            println!("  {} = {} (来自 {})", var.key, var.value, var.source);
        }
    }

    Ok(())
}

/// 诊断问题
fn diagnose(store: &Store, verbose: bool) -> Result<()> {
    println!("🔍 环境变量诊断工具\n");

    let mut issues = 0;

    // 1. 检查配置目录
    match utils::paths::get_config_dir() {
        Ok(dir) => {
            if !dir.exists() {
                println!("⚠️  配置目录不存在: {}", dir.display());
                println!("   解决：首次运行时会自动创建");
                issues += 1;
            } else {
                println!("✓ 配置目录存在: {}", dir.display());
            }
        }
        Err(e) => {
            println!("❌ 无法确定配置目录: {}", e);
            issues += 1;
        }
    }

    // 2. 检查重复变量
    let all_vars = store.list(None)?;
    let mut key_map = std::collections::HashMap::new();

    for var in &all_vars {
        key_map
            .entry(&var.key)
            .or_insert_with(Vec::new)
            .push(&var.source);
    }

    for (key, sources) in key_map {
        if sources.len() > 1 {
            println!("⚠️  环境变量 {} 在多层定义:", key);
            for source in sources {
                println!("   - {}", source);
            }
            issues += 1;
        }
    }

    // 3. 检查空文件
    for source in [EnvSource::User, EnvSource::Project, EnvSource::Local] {
        let path = utils::paths::get_layer_path(&source)?;
        if utils::paths::file_exists(&path) {
            let content = utils::paths::read_file(&path)?;
            if content.trim().is_empty() {
                println!("⚠️  空配置文件: {}", path.display());
                issues += 1;
            }
        }
    }

    // 4. 系统变量警告（如果过多）
    if let Ok(system_vars) = utils::paths::get_system_env() {
        if system_vars.len() > 100 {
            println!("ℹ️  系统环境变量较多 ({})，建议使用 --source 过滤", system_vars.len());
        }
    }

    if issues == 0 {
        println!("✅ 未发现明显问题");
    } else {
        println!("\n发现 {} 个问题", issues);
        if !verbose {
            println!("提示：使用 --verbose 查看详细信息");
        }
    }

    Ok(())
}