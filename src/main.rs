//! EnvCLI 主程序入口
//!
//! 设计原则：
//! - 模块化：入口代码简洁，逻辑委托给各模块
//! - 安静模式：默认无输出，成功静默
//! - 错误处理：详细/安静错误模式，通过 --verbose 切换

use clap::Parser;
use envcli::{
    cli::{
        self, CacheCommands, Cli, Commands, PluginCommands, PluginConfigCommands, TemplateCommands,
    },
    core::Store,
    error::{EnvError, Result},
    plugin::{HookContext, HookType, PluginManager, SignatureAlgorithm},
    template,
    types::{Config, EnvSource, OutputFormat},
    utils::{self, encryption::SopsEncryptor},
};
use std::collections::HashMap;
use std::path::PathBuf;

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
    let result = run_command(&cli.command, store, config.verbose);

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

/// 运行具体命令（带插件钩子集成）- 简化为路由分发器
fn run_command(command: &Commands, store: Store, verbose: bool) -> Result<()> {
    // 创建插件管理器（如果失败则使用空管理器）
    let plugin_manager = PluginManager::new().unwrap_or_else(|_| PluginManager::empty());

    // 获取命令名称
    let command_name = get_command_name(command);

    // 执行 PreCommand 钩子
    let (_, merged_env) = execute_pre_command_hooks(command_name, &plugin_manager, verbose)?;

    // 根据命令类型分发到对应的处理函数
    let result = match &command {
        // 读取类命令
        Commands::Get { .. }
        | Commands::List { .. }
        | Commands::Export { .. }
        | Commands::Status => handle_read_commands(command, &store, &merged_env, verbose),

        // 写入类命令
        Commands::Set { .. } | Commands::Unset { .. } | Commands::Import { .. } => {
            handle_write_commands(command, &store, &merged_env, verbose)
        }

        // 插件类命令
        Commands::Plugin {
            command: plugin_cmd,
        } => handle_plugin_commands(plugin_cmd, verbose),

        // 加密类命令
        Commands::Encrypt { .. }
        | Commands::Decrypt { .. }
        | Commands::SetEncrypt { .. }
        | Commands::CheckSops => handle_encrypt_commands(command, &store, verbose),

        // 系统类命令
        Commands::SystemSet { .. }
        | Commands::SystemUnset { .. }
        | Commands::Doctor
        | Commands::Run { .. } => {
            handle_system_commands(command, &store, &plugin_manager, &merged_env, verbose)
        }

        // 配置类命令
        Commands::Config {
            command: config_cmd,
        } => handle_config_commands(config_cmd, verbose),

        // 模板类命令
        Commands::Template {
            command: template_cmd,
        } => handle_template_commands(template_cmd, verbose),

        // 缓存类命令
        Commands::Cache { command: cache_cmd } => handle_cache_commands(cache_cmd, &store, verbose),
    };

    // 执行命令后的钩子
    execute_post_command_hooks(command_name, &plugin_manager)?;

    // 如果命令执行失败，执行错误钩子
    if let Err(ref e) = result {
        execute_error_hooks(command_name, e, &plugin_manager)?;
    }

    result
}

/// 显示当前状态 (详细信息，但仍然保持简洁)
fn show_status(store: &Store, verbose: bool) -> Result<()> {
    // 配置目录
    let config_dir = utils::paths::get_config_dir()?;
    println!("配置目录: {}", config_dir.display());

    // 各层级状态
    for source in [EnvSource::User, EnvSource::Project, EnvSource::Local] {
        let path = utils::paths::get_layer_path(&source)?;
        let exists = utils::paths::file_exists(&path);

        let status = if exists { "存在" } else { "不存在" };
        let count = if exists {
            let vars = store.list(Some(source.clone()))?;
            vars.len()
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

/// 诊断问题 - 增强版，提供更全面的健康检查
fn diagnose(store: &Store, verbose: bool) -> Result<()> {
    println!("🔍 EnvCLI 健康诊断工具\n");
    println!("版本: v0.1.0 | 平台: {}", std::env::consts::OS);
    println!("──────────────────────────────────────────────\n");

    let mut issues = 0;
    let mut warnings = 0;

    // 1. 检查配置目录
    println!("📁 1. 配置目录检查");
    match utils::paths::get_config_dir() {
        Ok(dir) => {
            if !dir.exists() {
                println!("   ❌ 配置目录不存在: {}", dir.display());
                println!("   💡 解决: 首次运行时会自动创建");
                issues += 1;
            } else {
                println!("   ✓ 配置目录存在: {}", dir.display());
                if verbose {
                    // 检查目录权限
                    match std::fs::metadata(&dir) {
                        Ok(metadata) => {
                            if metadata.permissions().readonly() {
                                println!("   ⚠️  目录为只读模式");
                                warnings += 1;
                            }
                        }
                        Err(_) => {
                            println!("   ❌ 无法读取目录权限");
                            issues += 1;
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("   ❌ 无法确定配置目录: {}", e);
            issues += 1;
        }
    }
    println!();

    // 2. 检查层级文件状态
    println!("📄 2. 配置文件状态");
    let mut file_count = 0;
    for source in [
        EnvSource::System,
        EnvSource::User,
        EnvSource::Project,
        EnvSource::Local,
    ] {
        let path = match utils::paths::get_layer_path(&source) {
            Ok(p) => p,
            Err(e) => {
                println!("   ❌ {} 无法获取路径: {}", source, e);
                issues += 1;
                continue;
            }
        };

        if utils::paths::file_exists(&path) {
            file_count += 1;

            // 尝试读取文件，处理权限问题
            let content_result = utils::paths::read_file(&path);
            match content_result {
                Ok(content) => {
                    let line_count = content.lines().count();
                    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

                    println!("   ✓ {} ({} 行, {} bytes)", source, line_count, size);

                    // 检查空文件
                    if content.trim().is_empty() {
                        println!("     ⚠️  空文件");
                        warnings += 1;
                    }

                    // 检查文件格式问题
                    if verbose {
                        let invalid_lines: Vec<_> = content
                            .lines()
                            .enumerate()
                            .filter(|(_, line)| {
                                let trimmed = line.trim();
                                !trimmed.is_empty()
                                    && !trimmed.starts_with('#')
                                    && !trimmed.contains('=')
                            })
                            .map(|(i, line)| (i + 1, line))
                            .collect();

                        if !invalid_lines.is_empty() {
                            println!("     ⚠️  发现 {} 行格式问题", invalid_lines.len());
                            for (line_num, line) in invalid_lines.iter().take(3) {
                                println!("       行 {}: {}", line_num, line);
                            }
                            if invalid_lines.len() > 3 {
                                println!("       ... 还有 {} 行", invalid_lines.len() - 3);
                            }
                            issues += 1;
                        }
                    }
                }
                Err(EnvError::PermissionDenied(_msg)) => {
                    println!("   ⚠️  {} 权限不足 (只读)", source);
                    warnings += 1;
                }
                Err(e) => {
                    println!("   ❌ {} 读取失败: {}", source, e);
                    issues += 1;
                }
            }
        } else {
            println!("   ○ {} (不存在)", source);
        }
    }
    if file_count == 0 {
        println!("   ⚠️  未找到任何配置文件");
        warnings += 1;
    }
    println!();

    // 3. 检查重复变量
    println!("🔄 3. 变量冲突检查");
    let all_vars = store.list(None)?;
    let mut key_map = std::collections::HashMap::new();

    for var in &all_vars {
        key_map
            .entry(&var.key)
            .or_insert_with(Vec::new)
            .push(&var.source);
    }

    let mut duplicate_count = 0;
    for (key, sources) in &key_map {
        if sources.len() > 1 {
            duplicate_count += 1;
            if verbose || duplicate_count <= 5 {
                println!("   ⚠️  {} 在 {} 层定义:", key, sources.len());
                for source in sources {
                    println!("     - {}", source);
                }
            }
        }
    }

    if duplicate_count > 5 {
        println!("   ... 还有 {} 个重复变量", duplicate_count - 5);
    }

    if duplicate_count > 0 {
        println!("   💡 建议: 使用 envcli get <key> 查看优先级");
        issues += duplicate_count;
    } else {
        println!("   ✓ 无变量冲突");
    }
    println!();

    // 4. 系统环境变量统计
    println!("🖥️  4. 系统环境变量");
    match utils::paths::get_system_env() {
        Ok(system_vars) => {
            println!("   总数: {} 个变量", system_vars.len());

            if system_vars.len() > 100 {
                println!("   ⚠️  系统变量较多，建议使用 --source 过滤");
                warnings += 1;
            }

            if verbose {
                // 显示一些关键变量
                let key_vars = ["PATH", "HOME", "USERPROFILE", "TEMP", "TMP"];
                for key in key_vars {
                    if let Some(value) = system_vars.get(key) {
                        let display_len = if value.len() > 50 { 47 } else { value.len() };
                        println!("   ✓ {}={}", key, &value[..display_len]);
                        if value.len() > 50 {
                            println!("       ... ({} more chars)", value.len() - 50);
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("   ❌ 无法读取系统环境: {}", e);
            issues += 1;
        }
    }
    println!();

    // 5. 插件系统检查（如果插件已加载）
    println!("🔌 5. 插件系统状态");
    let plugin_manager = PluginManager::new().unwrap_or_else(|_| PluginManager::empty());
    let plugin_stats = plugin_manager.get_stats();
    println!("   已加载插件: {}", plugin_stats.loaded_plugins);
    println!("   总执行次数: {}", plugin_stats.total_executions);

    if plugin_stats.loaded_plugins > 0 && verbose {
        println!("   详细信息:");
        for plugin in plugin_manager.list_plugins(true) {
            println!(
                "     - {} (v{})",
                plugin.metadata.id, plugin.metadata.version
            );
        }
    }
    println!();

    // 6. 运行环境检查
    println!("🔧 6. 运行环境");
    println!(
        "   当前工作目录: {:?}",
        std::env::current_dir().unwrap_or_default()
    );
    println!(
        "   可执行文件路径: {:?}",
        std::env::current_exe().unwrap_or_default()
    );

    // 检查 PATH
    if let Some(path_var) = std::env::var_os("PATH") {
        let path_count = std::env::split_paths(&path_var).count();
        println!("   PATH 包含 {} 个目录", path_count);
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

        println!("\n💡 快速修复建议:");
        if issues > 0 {
            println!("   1. 运行 'envcli doctor --verbose' 查看详细信息");
            println!("   2. 按照上述建议修复问题");
            println!("   3. 再次运行诊断确认修复");
        }
        if warnings > 0 {
            println!("   • 警告信息可选择性处理");
        }
    }

    Ok(())
}

/// 处理配置管理命令
fn handle_config_commands(command: &cli::ConfigCommands, verbose: bool) -> Result<()> {
    match command {
        cli::ConfigCommands::Validate {
            verbose: verbose_flag,
        } => validate_config(*verbose_flag || verbose),
        cli::ConfigCommands::Init { force } => init_config_files(*force),
        cli::ConfigCommands::Info => show_config_info(),
    }
}

/// 验证配置文件格式和完整性
fn validate_config(verbose: bool) -> Result<()> {
    println!("🔍 配置文件验证\n");

    let mut issues = 0;
    let mut warnings = 0;

    // 检查所有层级的配置文件
    for source in [
        EnvSource::System,
        EnvSource::User,
        EnvSource::Project,
        EnvSource::Local,
    ] {
        let path = utils::paths::get_layer_path(&source)?;

        if utils::paths::file_exists(&path) {
            println!("📄 {} 层级:", source);

            // 读取文件内容
            let content = utils::paths::read_file(&path)?;

            // 检查空文件
            if content.trim().is_empty() {
                println!("   ⚠️  空文件");
                warnings += 1;
                continue;
            }

            // 检查格式
            let mut line_num = 0;
            let mut valid_vars = 0;
            let mut invalid_lines = Vec::new();

            for line in content.lines() {
                line_num += 1;
                let trimmed = line.trim();

                // 跳过空行和注释
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }

                // 检查是否包含等号
                if let Some(eq_pos) = trimmed.find('=') {
                    let key = trimmed[..eq_pos].trim();
                    let value = trimmed[eq_pos + 1..].trim();

                    if key.is_empty() {
                        invalid_lines.push((line_num, "键名为空"));
                        issues += 1;
                    } else if value.is_empty() {
                        warnings += 1;
                        if verbose {
                            println!("   ⚠️  行 {}: 值为空 (key={})", line_num, key);
                        }
                    } else {
                        valid_vars += 1;
                    }
                } else {
                    invalid_lines.push((line_num, "缺少等号"));
                    issues += 1;
                }
            }

            println!("   ✓ 有效变量: {}", valid_vars);

            if !invalid_lines.is_empty() {
                println!("   ❌ 格式错误:");
                for (line_num, reason) in &invalid_lines {
                    println!("      行 {}: {}", line_num, reason);
                }
            }

            // 详细模式：显示所有变量
            if verbose && valid_vars > 0 {
                println!("   📋 变量列表:");
                for line in content.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty()
                        && !trimmed.starts_with('#')
                        && trimmed.contains('=')
                        && let Some(eq_pos) = trimmed.find('=')
                    {
                        let key = trimmed[..eq_pos].trim();
                        let value = trimmed[eq_pos + 1..].trim();
                        let display_value = if value.len() > 30 {
                            format!("{}...", &value[..27])
                        } else {
                            value.to_string()
                        };
                        println!("      {} = {}", key, display_value);
                    }
                }
            }
            println!();
        } else {
            println!("📄 {} 层级: 不存在", source);
        }
    }

    // 总结
    println!("──────────────────────────────────────────────");
    if issues == 0 && warnings == 0 {
        println!("✅ 所有配置文件格式正确");
    } else {
        if issues > 0 {
            println!("❌ 发现 {} 个格式错误", issues);
        }
        if warnings > 0 {
            println!("⚠️  发现 {} 个警告", warnings);
        }
        println!("\n💡 建议:");
        println!("   1. 格式: KEY=VALUE (每行一个)");
        println!("   2. 注释以 # 开头");
        println!("   3. 空行会被忽略");
    }

    Ok(())
}

/// 初始化配置文件
fn init_config_files(force: bool) -> Result<()> {
    println!("🔧 初始化配置文件\n");

    let config_dir = utils::paths::get_config_dir()?;

    // 检查配置目录是否存在
    if config_dir.exists() && !force {
        println!("⚠️  配置目录已存在: {}", config_dir.display());
        println!("   使用 --force 重新初始化");
        return Ok(());
    }

    // 创建配置目录
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir)?;
        println!("✓ 创建配置目录: {}", config_dir.display());
    }

    // 创建各层级文件（如果不存在或强制模式）
    for source in [EnvSource::User, EnvSource::Project, EnvSource::Local] {
        let path = utils::paths::get_layer_path(&source)?;

        if !path.exists() || force {
            // 创建空文件
            std::fs::write(&path, "# EnvCLI 配置文件\n# 格式: KEY=VALUE\n\n")?;
            println!("✓ 创建文件: {}", path.display());
        } else {
            println!("○ 文件已存在: {}", path.display());
        }
    }

    println!("\n✅ 配置初始化完成");
    println!("💡 提示:");
    println!("   - 使用 'envcli set KEY VALUE' 添加变量");
    println!("   - 使用 'envcli config validate' 验证配置");
    println!("   - 使用 'envcli doctor' 诊断问题");

    Ok(())
}

/// 显示配置信息
fn show_config_info() -> Result<()> {
    println!("📋 EnvCLI 配置信息\n");

    // 配置目录
    match utils::paths::get_config_dir() {
        Ok(dir) => {
            println!("配置目录: {}", dir.display());
            if dir.exists() {
                println!("状态: ✓ 存在");
            } else {
                println!("状态: ✗ 不存在");
            }
        }
        Err(e) => {
            println!("配置目录: 无法确定 ({})", e);
        }
    }
    println!();

    // 各层级文件状态
    println!("层级文件:");
    for source in [
        EnvSource::System,
        EnvSource::User,
        EnvSource::Project,
        EnvSource::Local,
    ] {
        let path = utils::paths::get_layer_path(&source)?;
        if utils::paths::file_exists(&path) {
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let content = utils::paths::read_file(&path).unwrap_or_default();
            let lines = content.lines().count();
            println!(
                "  {}: {} ({} bytes, {} lines)",
                source,
                path.display(),
                size,
                lines
            );
        } else {
            println!("  {}: 不存在", source);
        }
    }
    println!();

    // 系统信息
    println!("系统信息:");
    println!("  平台: {}", std::env::consts::OS);
    println!("  版本: v0.1.0");

    // 当前工作目录
    if let Ok(cwd) = std::env::current_dir() {
        println!("  工作目录: {}", cwd.display());
    }

    Ok(())
}

// ==================== 重构辅助函数 (KISS/DRY/LOD 原则) ====================

/// 执行插件钩子（提取重复逻辑）
fn execute_plugin_hooks(
    hook_type: HookType,
    context: &HookContext,
    plugin_manager: &PluginManager,
) -> Result<Vec<envcli::plugin::HookResult>> {
    Ok(plugin_manager.execute_hooks(hook_type, context)?)
}

/// 合并插件环境变量（提取重复逻辑）
fn merge_plugin_env(results: &[envcli::plugin::HookResult]) -> HashMap<String, String> {
    let mut merged_env = HashMap::new();
    for result in results {
        for (k, v) in &result.modified_env {
            merged_env.insert(k.clone(), v.clone());
        }
    }
    merged_env
}

/// 检查插件是否阻止执行（提取重复逻辑）
fn check_plugin_block(results: &[envcli::plugin::HookResult], verbose: bool) -> Result<()> {
    for result in results {
        if !result.continue_execution {
            if verbose {
                println!("⚠️  插件阻止了命令执行: {:?}", result.message);
            }
            return Ok(()); // 返回 Ok 但停止执行
        }
    }
    Ok(())
}

/// 验证作用域参数（提取重复逻辑）
fn validate_scope(scope: &str) -> Result<()> {
    if scope != "global" && scope != "machine" {
        return Err(EnvError::InvalidArgument(
            "scope 必须是 'global' 或 'machine'".to_string(),
        ));
    }
    Ok(())
}

/// 创建钩子上下文（提取重复逻辑）
fn create_hook_context(command: &str) -> HookContext<'_> {
    HookContext {
        command,
        args: &[],
        env: HashMap::new(),
        plugin_data: HashMap::new(),
        continue_execution: true,
        error: None,
    }
}

/// 通用结果处理器（提取重复逻辑）
fn handle_result<T>(result: Result<T>, verbose: bool, success_msg: Option<&str>) -> Result<()> {
    match result {
        Ok(_) => {
            if verbose && let Some(msg) = success_msg {
                println!("✓ {}", msg);
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// 从命令获取命令名称（提取重复逻辑）
fn get_command_name(command: &Commands) -> &'static str {
    match command {
        Commands::Get { .. } => "get",
        Commands::Set { .. } => "set",
        Commands::Unset { .. } => "unset",
        Commands::List { .. } => "list",
        Commands::Import { .. } => "import",
        Commands::Export { .. } => "export",
        Commands::Status => "status",
        Commands::Doctor => "doctor",
        Commands::Config { .. } => "config",
        Commands::Run { .. } => "run",
        Commands::Template { .. } => "template",
        Commands::Encrypt { .. } => "encrypt",
        Commands::Decrypt { .. } => "decrypt",
        Commands::SetEncrypt { .. } => "set-encrypt",
        Commands::CheckSops => "check-sops",
        Commands::Plugin { .. } => "plugin",
        Commands::SystemSet { .. } => "system-set",
        Commands::SystemUnset { .. } => "system-unset",
        Commands::Cache { .. } => "cache",
    }
}

/// 执行命令前的插件钩子（提取重复逻辑）
fn execute_pre_command_hooks(
    command_name: &str,
    plugin_manager: &PluginManager,
    verbose: bool,
) -> Result<(Vec<envcli::plugin::HookResult>, HashMap<String, String>)> {
    let context = create_hook_context(command_name);
    let results = execute_plugin_hooks(HookType::PreCommand, &context, plugin_manager)?;

    // 检查是否被阻止
    check_plugin_block(&results, verbose)?;

    // 合并环境变量
    let merged_env = merge_plugin_env(&results);

    Ok((results, merged_env))
}

/// 执行命令后的插件钩子（提取重复逻辑）
fn execute_post_command_hooks(command_name: &str, plugin_manager: &PluginManager) -> Result<()> {
    let context = create_hook_context(command_name);
    let _ = execute_plugin_hooks(HookType::PostCommand, &context, plugin_manager)?;
    Ok(())
}

/// 执行错误插件钩子（提取重复逻辑）
fn execute_error_hooks(
    command_name: &str,
    error: &EnvError,
    plugin_manager: &PluginManager,
) -> Result<()> {
    let mut context = create_hook_context(command_name);
    context.error = Some(error.to_string());
    let _ = execute_plugin_hooks(HookType::Error, &context, plugin_manager)?;
    Ok(())
}

/// 处理 Run 命令的特殊逻辑
fn handle_run_command(
    env: &[String],
    from_file: &Option<String>,
    cmd: &[String],
    store: &Store,
    plugin_manager: &PluginManager,
    _verbose: bool,
) -> Result<()> {
    // 执行 PreRun 钩子
    let pre_run_context = create_hook_context("run");
    let pre_run_results = execute_plugin_hooks(HookType::PreRun, &pre_run_context, plugin_manager)?;
    let run_env = merge_plugin_env(&pre_run_results);

    // 1. 解析临时环境变量
    let mut temp_vars = utils::env_merge::EnvMerger::parse_temp_vars(env)?;

    // 2. 从文件解析
    if let Some(file) = from_file {
        let file_vars = utils::env_merge::EnvMerger::parse_file(file)?;
        temp_vars.extend(file_vars);
    }

    // 3. 合并所有环境变量
    let mut merged_run_env = utils::env_merge::EnvMerger::merge_environment(store, &temp_vars)?;

    // 4. 合并插件添加的环境变量
    for (k, v) in &run_env {
        merged_run_env.insert(k.clone(), v.clone());
    }

    // 5. 执行命令
    let exit_code = utils::executor::CommandExecutor::exec_with_env(cmd, &merged_run_env)?;

    // 6. 执行 PostRun 钩子
    let post_run_context = create_hook_context("run");
    let _ = execute_plugin_hooks(HookType::PostRun, &post_run_context, plugin_manager)?;

    // 7. 退出码透传
    std::process::exit(exit_code);
}

// ==================== 命令分组处理函数 ====================

/// 处理读取类命令 (Get, List, Status, Export)
fn handle_read_commands(
    command: &Commands,
    store: &Store,
    merged_env: &HashMap<String, String>,
    verbose: bool,
) -> Result<()> {
    match command {
        Commands::Get { key } => {
            // 检查是否有插件修改的环境变量
            if let Some(value) = merged_env.get(key) {
                println!("{}", value);
                Ok(())
            } else {
                match store.get(key)? {
                    Some(value) => {
                        println!("{}", value);
                        Ok(())
                    }
                    None => Err(EnvError::NotFound(key.clone())),
                }
            }
        }

        Commands::List { source, format } => {
            let source_filter = cli::parse_list_source(source.as_deref())?;
            let output_format = cli::parse_format(format);
            let mut vars = store.list(source_filter)?;

            // 合并插件添加的环境变量
            for (k, v) in merged_env {
                vars.push(envcli::types::EnvVar::new(
                    k.clone(),
                    v.clone(),
                    EnvSource::Local, // 插件添加的变量归入 Local 层
                ));
            }

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
            Ok(())
        }

        Commands::Export { source, format } => {
            let source_filter = cli::parse_list_source(source.as_deref())?;
            let output_format = cli::parse_format(format);
            let content = store.export(source_filter.clone())?;

            match output_format {
                OutputFormat::ENV => println!("{}", content),
                OutputFormat::JSON => {
                    let vars = store.list(source_filter)?;
                    let json = serde_json::to_string_pretty(&vars)?;
                    println!("{}", json);
                }
            }
            Ok(())
        }

        Commands::Status => show_status(store, verbose),

        _ => Err(EnvError::InvalidArgument("非读取类命令".to_string())),
    }
}

/// 处理写入类命令 (Set, Unset, Import)
fn handle_write_commands(
    command: &Commands,
    store: &Store,
    merged_env: &HashMap<String, String>,
    verbose: bool,
) -> Result<()> {
    match command {
        Commands::Set { key, value } => {
            // 应用插件修改
            if let Some(plugin_value) = merged_env.get(key) {
                store.set(key.clone(), plugin_value.clone())?;
            } else {
                store.set(key.clone(), value.clone())?;
            }
            Ok(())
        }

        Commands::Unset { key } => {
            let deleted = store.unset(key)?;
            if verbose && deleted {
                println!("✓ 已删除");
            } else if !deleted {
                return Err(EnvError::NotFound(key.clone()));
            }
            Ok(())
        }

        Commands::Import { file, target } => {
            let target_source = cli::validate_writable_source(target)?;
            let count = store.import_file(file, &target_source)?;
            handle_result(Ok(()), verbose, Some(&format!("成功导入 {} 个变量", count)))
        }

        _ => Err(EnvError::InvalidArgument("非写入类命令".to_string())),
    }
}

/// 处理插件类命令
fn handle_plugin_commands(command: &PluginCommands, verbose: bool) -> Result<()> {
    match command {
        PluginCommands::List {
            verbose: list_verbose,
            show_disabled,
        } => {
            let manager = PluginManager::new()?;
            let plugins = manager.list_plugins(*show_disabled);

            if plugins.is_empty() {
                println!("暂无插件");
                return Ok(());
            }

            for plugin_info in plugins {
                let status = if plugin_info.metadata.enabled {
                    "✓"
                } else {
                    "✗"
                };
                println!(
                    "{} {} ({})",
                    status, plugin_info.metadata.name, plugin_info.metadata.id
                );

                if *list_verbose {
                    println!("  版本: {}", plugin_info.metadata.version);
                    if let Some(desc) = &plugin_info.metadata.description {
                        println!("  描述: {}", desc);
                    }
                    if let Some(author) = &plugin_info.metadata.author {
                        println!("  作者: {}", author);
                    }
                    println!("  类型: {:?}", plugin_info.metadata.plugin_type);
                    if !plugin_info.metadata.hooks.is_empty() {
                        println!(
                            "  钩子: {}",
                            plugin_info
                                .metadata
                                .hooks
                                .iter()
                                .map(|h| format!("{:?}", h))
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                    println!();
                }
            }
            Ok(())
        }

        PluginCommands::Show { plugin_id } => {
            let manager = PluginManager::new()?;
            let plugin_info = manager
                .get_plugin_info(plugin_id)
                .ok_or_else(|| EnvError::PluginNotFound(plugin_id.clone()))?;

            println!("插件 ID: {}", plugin_info.metadata.id);
            println!("名称: {}", plugin_info.metadata.name);
            println!("版本: {}", plugin_info.metadata.version);
            println!("类型: {:?}", plugin_info.metadata.plugin_type);
            println!(
                "状态: {}",
                if plugin_info.metadata.enabled {
                    "已启用"
                } else {
                    "已禁用"
                }
            );

            if let Some(desc) = &plugin_info.metadata.description {
                println!("描述: {}", desc);
            }
            if let Some(author) = &plugin_info.metadata.author {
                println!("作者: {}", author);
            }

            if !plugin_info.metadata.hooks.is_empty() {
                println!(
                    "钩子: {}",
                    plugin_info
                        .metadata
                        .hooks
                        .iter()
                        .map(|h| format!("{:?}", h))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }

            if !plugin_info.metadata.dependencies.is_empty() {
                println!("依赖: {}", plugin_info.metadata.dependencies.join(", "));
            }

            if let Some(schema) = &plugin_info.metadata.config_schema {
                println!("\n配置模式:");
                for field in &schema.fields {
                    let required = if field.required { "必需" } else { "可选" };
                    let default = field
                        .default
                        .as_ref()
                        .map(|d| format!(" (默认: {})", d))
                        .unwrap_or_default();
                    println!(
                        "  {} ({:?}): {}{}",
                        field.name, field.field_type, required, default
                    );
                    if let Some(desc) = &field.description {
                        println!("    {}", desc);
                    }
                }
            }
            Ok(())
        }

        PluginCommands::Enable { plugin_id } => {
            let mut manager = PluginManager::new()?;
            manager
                .enable_plugin(plugin_id)
                .map_err(|e| EnvError::PluginExecutionFailed(e.to_string()))?;

            if verbose {
                println!("✓ 已启用插件: {}", plugin_id);
            }
            Ok(())
        }

        PluginCommands::Disable { plugin_id } => {
            let mut manager = PluginManager::new()?;
            manager
                .disable_plugin(plugin_id)
                .map_err(|e| EnvError::PluginExecutionFailed(e.to_string()))?;

            if verbose {
                println!("✓ 已禁用插件: {}", plugin_id);
            }
            Ok(())
        }

        PluginCommands::Load { path, config: _ } => {
            let mut manager = PluginManager::new()?;
            let path_buf = PathBuf::from(&path);

            manager
                .load_from_path(&path_buf)
                .map_err(|e| EnvError::PluginLoadFailed(e.to_string()))?;

            if verbose {
                println!("✓ 已加载插件: {}", path);
            }
            Ok(())
        }

        PluginCommands::Unload { plugin_id } => {
            let mut manager = PluginManager::new()?;
            manager
                .unload_plugin(plugin_id)
                .map_err(|e| EnvError::PluginExecutionFailed(e.to_string()))?;

            if verbose {
                println!("✓ 已卸载插件: {}", plugin_id);
            }
            Ok(())
        }

        PluginCommands::Reload { plugin_id } => {
            let mut manager = PluginManager::new()?;
            let new_id = manager
                .reload(plugin_id)
                .map_err(|e| EnvError::PluginExecutionFailed(e.to_string()))?;

            if verbose {
                if new_id == *plugin_id {
                    println!("✓ 已重载插件: {}", plugin_id);
                } else {
                    println!("✓ 已重载插件: {} -> {}", plugin_id, new_id);
                }
            }
            Ok(())
        }

        PluginCommands::Status { plugin_id } => {
            let manager = PluginManager::new()?;

            match plugin_id {
                Some(id) => {
                    let info = manager
                        .get_plugin_info(id)
                        .ok_or_else(|| EnvError::PluginNotFound(id.clone()))?;

                    println!("插件: {}", info.metadata.name);
                    println!(
                        "状态: {}",
                        if info.metadata.enabled {
                            "已启用"
                        } else {
                            "已禁用"
                        }
                    );
                    println!("已加载: {}", manager.is_loaded(id));

                    let stats = manager.get_stats();
                    println!("执行次数: {}", stats.total_executions);
                    println!("错误次数: {}", stats.total_errors);
                    Ok(())
                }
                None => {
                    let stats = manager.get_stats();
                    let plugins = manager.list_plugins(true);

                    println!("插件总数: {}", plugins.len());
                    println!(
                        "已启用: {}",
                        plugins.iter().filter(|p| p.metadata.enabled).count()
                    );
                    println!("已加载: {}", stats.loaded_plugins);
                    println!("总执行次数: {}", stats.total_executions);
                    println!("错误次数: {}", stats.total_errors);

                    if verbose && !plugins.is_empty() {
                        println!("\n详细状态:");
                        for plugin in plugins {
                            println!(
                                "  {}: {} ({}), {}",
                                plugin.metadata.name,
                                if plugin.metadata.enabled {
                                    "启用"
                                } else {
                                    "禁用"
                                },
                                if manager.is_loaded(&plugin.metadata.id) {
                                    "已加载"
                                } else {
                                    "未加载"
                                },
                                plugin.metadata.version
                            );
                        }
                    }
                    Ok(())
                }
            }
        }

        PluginCommands::Config(config_cmd) => match config_cmd {
            // 设置配置（简化：仅显示提示）
            PluginConfigCommands::Set {
                plugin_id,
                key,
                value,
            } => {
                if verbose {
                    println!("⚠️  配置管理功能暂未完全实现");
                    println!("   插件: {}, 配置: {} = {}", plugin_id, key, value);
                }
                Ok(())
            }

            // 获取配置（简化：显示提示）
            PluginConfigCommands::Get { plugin_id, key } => {
                if verbose {
                    println!("⚠️  配置管理功能暂未完全实现");
                    println!("   插件: {}, 配置项: {:?}", plugin_id, key);
                }
                Ok(())
            }

            // 重置配置（简化：显示提示）
            PluginConfigCommands::Reset { plugin_id } => {
                if verbose {
                    println!("⚠️  配置管理功能暂未完全实现");
                    println!("   插件: {}", plugin_id);
                }
                Ok(())
            }

            // 导出配置（简化：显示提示）
            PluginConfigCommands::Export => {
                println!("⚠️  配置管理功能暂未完全实现");
                Ok(())
            }

            // 导入配置（简化：显示提示）
            PluginConfigCommands::Import { file } => {
                if verbose {
                    println!("⚠️  配置管理功能暂未完全实现");
                    println!("   文件: {}", file);
                }
                Ok(())
            }
        },

        PluginCommands::GenerateKeyPair => match PluginManager::generate_key_pair() {
            Ok((private_key, public_key)) => {
                println!("✓ 密钥对生成成功");
                println!();
                println!("私钥 (请安全保存):");
                println!("{}", private_key);
                println!();
                println!("公钥:");
                println!("{}", public_key);
                println!();
                println!("指纹: {}", PluginManager::fingerprint(&public_key));
                Ok(())
            }
            Err(e) => Err(EnvError::PluginExecutionFailed(e.to_string())),
        },

        PluginCommands::Sign {
            plugin_id,
            key,
            algorithm,
            output,
        } => {
            let manager = PluginManager::new()?;

            // 解析算法
            let sig_algorithm = match algorithm.as_str() {
                "Ed25519" => SignatureAlgorithm::Ed25519,
                _ => {
                    return Err(EnvError::PluginExecutionFailed(
                        "不支持的签名算法，仅支持 Ed25519".to_string(),
                    ));
                }
            };

            match manager.sign_plugin(plugin_id, key, sig_algorithm) {
                Ok(signature) => {
                    let signature_json = serde_json::to_string_pretty(&signature)
                        .map_err(|e| EnvError::PluginExecutionFailed(e.to_string()))?;

                    if let Some(output_path) = output {
                        std::fs::write(output_path, &signature_json).map_err(EnvError::Io)?;
                        println!("✓ 签名已保存到 {}", output_path);
                    } else {
                        println!("✓ 签名生成成功:");
                        println!("{}", signature_json);
                    }
                    Ok(())
                }
                Err(e) => Err(EnvError::PluginExecutionFailed(e.to_string())),
            }
        }

        PluginCommands::Verify {
            plugin_id,
            trust_unsigned,
        } => {
            let manager = PluginManager::new()?;

            match manager.verify_plugin_signature(plugin_id, *trust_unsigned) {
                Ok(()) => {
                    println!("✓ 插件 {} 签名验证通过", plugin_id);
                    Ok(())
                }
                Err(e) => {
                    println!("✗ 插件 {} 签名验证失败: {}", plugin_id, e);
                    Err(EnvError::PluginExecutionFailed(e.to_string()))
                }
            }
        }

        PluginCommands::VerifyAll { trust_unsigned } => {
            let manager = PluginManager::new()?;

            match manager.verify_all_signatures(*trust_unsigned) {
                Ok(()) => {
                    println!("✓ 所有插件签名验证通过");
                    Ok(())
                }
                Err(e) => {
                    println!("✗ 签名验证失败: {}", e);
                    Err(EnvError::PluginExecutionFailed(e.to_string()))
                }
            }
        }

        PluginCommands::Fingerprint { public_key } => {
            let fingerprint = PluginManager::fingerprint(public_key);
            println!("公钥指纹: {}", fingerprint);
            Ok(())
        }

        PluginCommands::Test { plugin_id, hook } => {
            let manager = PluginManager::new()?;

            // 获取插件信息
            let info = manager
                .get_plugin_info(plugin_id)
                .ok_or_else(|| EnvError::PluginNotFound(plugin_id.clone()))?;

            println!("测试插件: {} ({})", info.metadata.name, info.metadata.id);

            // 创建测试上下文
            let context = HookContext {
                command: "test",
                args: &[],
                env: HashMap::new(),
                plugin_data: HashMap::new(),
                continue_execution: true,
                error: None,
            };

            // 确定要测试的钩子类型
            let hooks_to_test = if let Some(hook_name) = hook {
                // 解析钩子类型
                let hook_type = match hook_name.to_lowercase().as_str() {
                    "precommand" => HookType::PreCommand,
                    "postcommand" => HookType::PostCommand,
                    "error" => HookType::Error,
                    "prerun" => HookType::PreRun,
                    "postrun" => HookType::PostRun,
                    "configload" => HookType::ConfigLoad,
                    "configsave" => HookType::ConfigSave,
                    _ => {
                        return Err(EnvError::Parse(format!("未知钩子类型: {}", hook_name)));
                    }
                };
                vec![hook_type]
            } else {
                // 测试所有支持的钩子
                info.metadata.hooks.clone()
            };

            if hooks_to_test.is_empty() {
                println!("该插件没有注册任何钩子");
                return Ok(());
            }

            // 执行钩子测试
            for hook_type in hooks_to_test {
                println!("\n测试钩子: {:?}", hook_type);
                match manager.execute_hooks(hook_type, &context) {
                    Ok(results) => {
                        for result in results {
                            println!("  ✓ 成功");
                            if verbose {
                                println!("    消息: {:?}", result.message);
                                println!("    数据: {:?}", result.plugin_data);
                                println!("    继续执行: {}", result.continue_execution);
                            }
                        }
                    }
                    Err(e) => {
                        println!("  ✗ 执行错误: {}", e);
                    }
                }
            }
            Ok(())
        }

        PluginCommands::CheckDeps { plugin_id } => {
            let manager = PluginManager::new()?;

            match plugin_id {
                Some(id) => {
                    // 检查单个插件
                    let (satisfied, missing) = manager.check_dependencies(id);

                    println!("插件 {} 的依赖状态:", id);

                    if !satisfied.is_empty() {
                        println!("  ✓ 已满足: {}", satisfied.join(", "));
                    }

                    if !missing.is_empty() {
                        println!("  ✗ 缺失: {}", missing.join(", "));
                    }

                    if satisfied.is_empty() && missing.is_empty() {
                        println!("  - 无依赖");
                    }
                    Ok(())
                }
                None => {
                    // 检查所有插件
                    match manager.validate_all_dependencies() {
                        Ok(()) => {
                            println!("✓ 所有插件依赖关系有效");
                            Ok(())
                        }
                        Err(e) => {
                            println!("✗ 依赖验证失败: {}", e);
                            Ok(())
                        }
                    }
                }
            }
        }

        PluginCommands::LoadDeps { paths } => {
            let mut manager = PluginManager::new()?;

            // 转换路径
            let path_bufs: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();

            match manager.load_with_dependencies(&path_bufs) {
                Ok(loaded) => {
                    println!("✓ 成功加载 {} 个插件", loaded.len());
                    if verbose {
                        println!("加载顺序: {}", loaded.join(" -> "));
                    }
                    Ok(())
                }
                Err(e) => Err(EnvError::PluginExecutionFailed(e.to_string())),
            }
        }
    }
}

/// 处理加密类命令 (Encrypt, Decrypt, SetEncrypt, CheckSops)
fn handle_encrypt_commands(command: &Commands, store: &Store, verbose: bool) -> Result<()> {
    match command {
        Commands::Encrypt { key, value, target } => {
            let target_source = cli::validate_writable_source(target)?;

            // 检查 SOPS
            store.check_sops()?;

            if target_source == EnvSource::Local {
                store.set_encrypted(key.clone(), value.to_string())?;
                if verbose {
                    println!("✓ 已加密并存储变量: {}", key);
                }
            } else {
                return Err(EnvError::PermissionDenied(
                    "加密存储目前只支持 local 层".to_string(),
                ));
            }
            Ok(())
        }

        Commands::Decrypt { key, source } => {
            let encryptor = SopsEncryptor::new();
            let value = if let Some(source_str) = source {
                let source_filter = cli::parse_list_source(Some(source_str))?;
                let vars = store.list_encrypted(source_filter)?;
                if let Some(var) = vars.iter().find(|v| v.key == *key) {
                    if var.is_encrypted() {
                        encryptor.decrypt(&var.value)?
                    } else {
                        var.value.clone()
                    }
                } else {
                    return Err(EnvError::NotFound(key.clone()));
                }
            } else {
                match store.get_decrypted(key)? {
                    Some(v) => v,
                    None => return Err(EnvError::NotFound(key.clone())),
                }
            };

            println!("{}", value);
            Ok(())
        }

        Commands::SetEncrypt {
            key,
            value,
            encrypt,
        } => {
            if *encrypt {
                store.check_sops()?;
                store.set_encrypted(key.clone(), value.to_string())?;
                if verbose {
                    println!("✓ 已加密并存储变量: {}", key);
                }
            } else {
                store.set(key.to_string(), value.to_string())?;
                if verbose {
                    println!("✓ 已存储变量");
                }
            }
            Ok(())
        }

        Commands::CheckSops => {
            store.check_sops()?;
            let version = SopsEncryptor::version()?;
            println!("✓ SOPS 可用");
            println!("版本: {}", version);
            Ok(())
        }

        _ => Err(EnvError::InvalidArgument("非加密类命令".to_string())),
    }
}

/// 处理系统类命令 (SystemSet, SystemUnset, Doctor, Run)
fn handle_system_commands(
    command: &Commands,
    store: &Store,
    plugin_manager: &PluginManager,
    _merged_env: &HashMap<String, String>,
    verbose: bool,
) -> Result<()> {
    match command {
        Commands::SystemSet { key, value, scope } => {
            validate_scope(scope)?;
            store.set_system(key.clone(), value.clone(), scope)?;
            Ok(())
        }

        Commands::SystemUnset { key, scope } => {
            validate_scope(scope)?;
            store.unset_system(key.clone(), scope)?;
            Ok(())
        }

        Commands::Doctor => diagnose(store, verbose),

        Commands::Run {
            env,
            from_file,
            command: cmd,
        } => {
            // Run 命令需要特殊处理，因为它会直接退出进程
            handle_run_command(env, from_file, cmd, store, plugin_manager, verbose)
        }

        _ => Err(EnvError::InvalidArgument("非系统类命令".to_string())),
    }
}

/// 处理模板类命令
fn handle_template_commands(command: &TemplateCommands, verbose: bool) -> Result<()> {
    let engine = template::TemplateEngine::new()?;

    match command {
        TemplateCommands::Create {
            name,
            vars,
            inherits,
        } => {
            let template = engine.create_template(name, vars, inherits)?;

            if verbose {
                println!("✓ 已创建模板: {}", template.name);
                println!("  变量: {:?}", template.variables);
                if !template.inherits.is_empty() {
                    println!("  继承: {:?}", template.inherits);
                }
            }
            Ok(())
        }

        TemplateCommands::Show { name } => {
            let template = engine.get_template(name)?;

            println!("模板名称: {}", template.name);
            println!("内容:\n{}", template.content);

            if !template.variables.is_empty() {
                println!("\n必需变量:");
                for var in &template.variables {
                    if var.required {
                        print!("  {}", var.name);
                        if let Some(default) = &var.default {
                            print!(" (默认: {})", default);
                        }
                        println!();
                    }
                }
            }

            if !template.inherits.is_empty() {
                println!("\n继承模板: {:?}", template.inherits);
            }

            Ok(())
        }

        TemplateCommands::List {
            verbose: list_verbose,
        } => {
            let templates = engine.list_templates()?;

            if templates.is_empty() {
                println!("暂无模板");
                return Ok(());
            }

            for template in templates {
                println!("{}", template.name);

                if *list_verbose {
                    // 显示变量详情
                    for var in &template.variables {
                        let required = if var.required { "必需" } else { "可选" };
                        match &var.default {
                            Some(default) => {
                                println!("  {} = {} ({})", var.name, default, required)
                            }
                            None => println!("  {} ({})", var.name, required),
                        }
                    }

                    // 显示继承关系
                    if !template.inherits.is_empty() {
                        println!("  继承: {}", template.inherits.join(", "));
                    }
                    println!();
                }
            }
            Ok(())
        }

        TemplateCommands::Render {
            name,
            var,
            interactive,
            output,
        } => {
            // 解析变量参数
            let mut variables = HashMap::new();
            for v in var {
                if let Some(pos) = v.find('=') {
                    let key = v[..pos].to_string();
                    let value = v[pos + 1..].to_string();
                    variables.insert(key, value);
                }
            }

            // 交互式模式：检查缺失变量
            if *interactive {
                let template = engine.get_template(name)?;
                for var_def in &template.variables {
                    if !variables.contains_key(&var_def.name) {
                        if var_def.required {
                            println!("请输入必需变量 {}: ", var_def.name);
                            let mut input = String::new();
                            std::io::stdin()
                                .read_line(&mut input)
                                .map_err(|e| EnvError::Io(std::io::Error::other(e)))?;
                            variables.insert(var_def.name.clone(), input.trim().to_string());
                        } else if let Some(default) = &var_def.default {
                            println!("变量 {} (默认: {}): ", var_def.name, default);
                            let mut input = String::new();
                            std::io::stdin()
                                .read_line(&mut input)
                                .map_err(|e| EnvError::Io(std::io::Error::other(e)))?;
                            let value = input.trim();
                            if !value.is_empty() {
                                variables.insert(var_def.name.clone(), value.to_string());
                            } else {
                                variables.insert(var_def.name.clone(), default.clone());
                            }
                        }
                    }
                }
            }

            // 渲染模板
            let result = engine.render_template(name, &variables)?;

            // 输出结果
            match output {
                Some(file_path) => {
                    // 写入文件
                    let path = std::path::Path::new(&file_path);
                    utils::paths::write_file_safe(path, &result)?;
                    if verbose {
                        println!("✓ 已渲染并保存到: {}", file_path);
                    }
                }
                None => {
                    // 输出到 stdout
                    println!("{}", result);
                }
            }
            Ok(())
        }

        TemplateCommands::Delete { name } => {
            let deleted = engine.delete_template(name)?;

            if deleted {
                if verbose {
                    println!("✓ 已删除模板: {}", name);
                }
                Ok(())
            } else {
                Err(EnvError::TemplateNotFound(name.to_string()))
            }
        }
    }
}

/// 处理缓存管理命令
fn handle_cache_commands(command: &CacheCommands, store: &Store, verbose: bool) -> Result<()> {
    match command {
        CacheCommands::Stats => {
            // 系统环境缓存统计
            let (sys_cached, sys_age) = utils::paths::get_system_env_cache_stats();
            println!("📋 缓存统计信息\n");

            println!("系统环境缓存:");
            if sys_cached {
                println!("  状态: ✓ 已缓存");
                println!("  存在时间: {:?}", sys_age);
                println!(
                    "  TTL 剩余: {:?}",
                    std::time::Duration::from_secs(60).saturating_sub(sys_age)
                );
            } else {
                println!("  状态: ✗ 未缓存");
            }

            // 文件缓存统计
            println!();
            println!("文件内容缓存:");
            if verbose {
                println!("  提示: 使用 'envcli get <key>' 多次来观察缓存效果");
                println!("  提示: 第一次较慢（读取文件），后续很快（命中缓存）");
            } else {
                println!("  使用 --verbose 查看详细统计信息");
            }

            println!();
            println!("💡 缓存说明:");
            println!("  - 系统环境缓存: 60秒 TTL");
            println!("  - 文件缓存: 基于文件修改时间自动失效");
            println!("  - 缓存可显著提升性能（减少 80-90% I/O）");
            Ok(())
        }

        CacheCommands::Clear { cache_type } => {
            match cache_type.as_str() {
                "file" => {
                    store.clear_cache();
                    if verbose {
                        println!("✓ 文件缓存已清除");
                    }
                }
                "system" => {
                    utils::paths::clear_system_env_cache();
                    if verbose {
                        println!("✓ 系统环境缓存已清除");
                    }
                }
                "all" => {
                    store.clear_cache();
                    utils::paths::clear_system_env_cache();
                    if verbose {
                        println!("✓ 所有缓存已清除");
                    }
                }
                _ => {
                    return Err(EnvError::InvalidArgument(
                        "缓存类型必须是: file/system/all".to_string(),
                    ));
                }
            }
            Ok(())
        }
    }
}
