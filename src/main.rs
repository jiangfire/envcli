//! EnvCLI 主程序入口
//!
//! 设计原则：
//! - 模块化：入口代码简洁，逻辑委托给各模块
//! - 安静模式：默认无输出，成功静默
//! - 错误处理：详细/安静错误模式，通过 --verbose 切换

use clap::Parser;
use envcli::{
    cli::{self, Cli, Commands, PluginCommands, PluginConfigCommands, TemplateCommands},
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

/// 运行具体命令（带插件钩子集成）
fn run_command(command: Commands, store: Store, verbose: bool) -> Result<()> {
    // 创建插件管理器（如果失败则使用空管理器）
    let plugin_manager = PluginManager::new().unwrap_or_else(|_| PluginManager::empty());

    // 准备钩子上下文
    let command_name = match &command {
        Commands::Get { .. } => "get",
        Commands::Set { .. } => "set",
        Commands::Unset { .. } => "unset",
        Commands::List { .. } => "list",
        Commands::Import { .. } => "import",
        Commands::Export { .. } => "export",
        Commands::Status => "status",
        Commands::Doctor => "doctor",
        Commands::Run { .. } => "run",
        Commands::Template { .. } => "template",
        Commands::Encrypt { .. } => "encrypt",
        Commands::Decrypt { .. } => "decrypt",
        Commands::SetEncrypt { .. } => "set-encrypt",
        Commands::CheckSops => "check-sops",
        Commands::Plugin { .. } => "plugin",
        Commands::SystemSet { .. } => "system-set",
        Commands::SystemUnset { .. } => "system-unset",
    };

    // 执行 PreCommand 钩子
    let pre_context = HookContext {
        command: command_name,
        args: &[],
        env: HashMap::new(),
        plugin_data: HashMap::new(),
        continue_execution: true,
        error: None,
    };

    let pre_results = plugin_manager.execute_hooks(HookType::PreCommand, &pre_context)?;

    // 检查是否继续执行
    for result in &pre_results {
        if !result.continue_execution {
            if verbose {
                println!("⚠️  插件阻止了命令执行: {:?}", result.message);
            }
            return Ok(());
        }
    }

    // 合并 PreCommand 钩子修改的环境变量
    let mut merged_env = HashMap::new();
    for result in &pre_results {
        for (k, v) in &result.modified_env {
            merged_env.insert(k.clone(), v.clone());
        }
    }

    // 执行命令
    let result = match command {
        // 读取系列
        Commands::Get { key } => {
            // 检查是否有插件修改的环境变量
            if let Some(value) = merged_env.get(&key) {
                println!("{}", value);
                Ok(())
            } else {
                match store.get(&key)? {
                    Some(value) => {
                        println!("{}", value);
                        Ok(())
                    }
                    None => Err(EnvError::NotFound(key)),
                }
            }
        }

        // 写入系列
        Commands::Set { key, value } => {
            // 应用插件修改
            if let Some(plugin_value) = merged_env.get(&key) {
                store.set(key.clone(), plugin_value.clone())?;
            } else {
                store.set(key, value)?;
            }
            Ok(())
        }

        Commands::Unset { key } => {
            let deleted = store.unset(&key)?;
            if verbose && deleted {
                println!("✓ 已删除");
            } else if !deleted {
                return Err(EnvError::NotFound(key));
            }
            Ok(())
        }

        // 列出系列
        Commands::List { source, format } => {
            let source_filter = cli::parse_list_source(source.as_deref())?;
            let output_format = cli::parse_format(&format);
            let mut vars = store.list(source_filter)?;

            // 合并插件添加的环境变量
            for (k, v) in &merged_env {
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

        // 导入系列
        Commands::Import { file, target } => {
            let target_source = cli::validate_writable_source(&target)?;
            let count = store.import_file(&file, &target_source)?;
            if verbose {
                println!("✓ 成功导入 {} 个变量", count);
            }
            Ok(())
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
            Ok(())
        }

        // 状态显示
        Commands::Status => {
            show_status(&store, verbose)
        }

        // 问题诊断
        Commands::Doctor => {
            diagnose(&store, verbose)
        }

        // 运行命令注入环境变量
        Commands::Run {
            env,
            from_file,
            command: cmd,
        } => {
            // 执行 PreRun 钩子
            let pre_run_context = HookContext {
                command: "run",
                args: &[],
                env: HashMap::new(),
                plugin_data: HashMap::new(),
                continue_execution: true,
                error: None,
            };
            let pre_run_results = plugin_manager.execute_hooks(HookType::PreRun, &pre_run_context)?;

            // 合并 PreRun 钩子的环境变量
            let mut run_env = HashMap::new();
            for result in &pre_run_results {
                for (k, v) in &result.modified_env {
                    run_env.insert(k.clone(), v.clone());
                }
            }

            // 1. 解析临时环境变量
            let mut temp_vars = utils::env_merge::EnvMerger::parse_temp_vars(&env)?;

            // 2. 如果指定了文件，从文件加载
            if let Some(file_path) = from_file {
                let file_vars = utils::env_merge::EnvMerger::parse_file(&file_path)?;
                temp_vars.extend(file_vars);
            }

            // 3. 合并插件的环境变量
            temp_vars.extend(run_env);

            // 4. 构建完整环境（按优先级合并）
            let final_env = utils::env_merge::EnvMerger::merge_environment(&store, &temp_vars)?;

            // 5. 执行命令
            let exit_code = utils::executor::CommandExecutor::exec_with_env(&cmd, &final_env)?;

            // 6. 执行 PostRun 钩子
            let post_run_context = HookContext {
                command: "run",
                args: &[],
                env: final_env,
                plugin_data: HashMap::new(),
                continue_execution: true,
                error: None,
            };
            let _ = plugin_manager.execute_hooks(HookType::PostRun, &post_run_context)?;

            // 7. 退出码透传
            std::process::exit(exit_code);
        }

        // 模板管理
        Commands::Template { command } => {
            let engine = template::TemplateEngine::new()?;
            run_template_command(command, &engine, verbose)
        }

        // 加密相关命令
        Commands::Encrypt { key, value, target } => {
            let target_source = cli::validate_writable_source(&target)?;

            // 检查 SOPS
            store.check_sops()?;

            if target_source == EnvSource::Local {
                store.set_encrypted(key.clone(), value)?;
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
                let source_filter = cli::parse_list_source(Some(&source_str))?;
                let vars = store.list_encrypted(source_filter)?;
                if let Some(var) = vars.iter().find(|v| v.key == key) {
                    if var.is_encrypted() {
                        encryptor.decrypt(&var.value)?
                    } else {
                        var.value.clone()
                    }
                } else {
                    return Err(EnvError::NotFound(key));
                }
            } else {
                match store.get_decrypted(&key)? {
                    Some(v) => v,
                    None => return Err(EnvError::NotFound(key)),
                }
            };

            println!("{}", value);
            Ok(())
        }

        Commands::SetEncrypt { key, value, encrypt } => {
            if encrypt {
                store.check_sops()?;
                store.set_encrypted(key.clone(), value)?;
                if verbose {
                    println!("✓ 已加密并存储变量: {}", key);
                }
            } else {
                store.set(key, value)?;
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

        // 插件管理
        Commands::Plugin { command } => {
            run_plugin_command(command, verbose)
        }

        // 系统环境变量设置
        Commands::SystemSet { key, value, scope } => {
            // 验证作用域
            cli::validate_scope(&scope)?;

            // 权限提示
            if scope == "machine" {
                eprintln!("⚠️  警告: 设置机器级变量需要管理员权限");
                eprintln!("   Windows: 可能需要 UAC 提升");
                eprintln!("   Unix/Linux: 不支持机器级变量");
            }

            // 执行设置
            store.set_system(key.clone(), value.clone(), &scope)?;

            if verbose {
                println!("✓ 已设置系统环境变量 {} = {} (scope: {})", key, value, scope);

                // Unix 额外提示
                #[cfg(not(target_os = "windows"))]
                if scope == "global" {
                    eprintln!("   请运行 'source ~/.bashrc' 或重新打开终端使更改生效");
                }
            }
            Ok(())
        }

        // 系统环境变量删除
        Commands::SystemUnset { key, scope } => {
            // 验证作用域
            cli::validate_scope(&scope)?;

            // 执行删除
            store.unset_system(key.clone(), &scope)?;

            if verbose {
                println!("✓ 已删除系统环境变量 {} (scope: {})", key, scope);

                // Unix 额外提示
                #[cfg(not(target_os = "windows"))]
                if scope == "global" {
                    eprintln!("   请运行 'source ~/.bashrc' 或重新打开终端使更改生效");
                }
            }
            Ok(())
        }
    };

    // 执行 PostCommand 钩子（仅在成功时）
    if result.is_ok() {
        let post_context = HookContext {
            command: command_name,
            args: &[],
            env: HashMap::new(),
            plugin_data: HashMap::new(),
            continue_execution: true,
            error: None,
        };
        let _ = plugin_manager.execute_hooks(HookType::PostCommand, &post_context)?;
    }

    // 如果有错误，执行 Error 钩子
    if let Err(e) = &result {
        let error_context = HookContext {
            command: command_name,
            args: &[],
            env: HashMap::new(),
            plugin_data: HashMap::new(),
            continue_execution: true,
            error: Some(e.to_string()),
        };
        let _ = plugin_manager.execute_hooks(HookType::Error, &error_context)?;
    }

    result
}

/// 处理模板子命令
fn run_template_command(
    command: TemplateCommands,
    engine: &template::TemplateEngine,
    verbose: bool,
) -> Result<()> {
    match command {
        TemplateCommands::Create { name, vars, inherits } => {
            let template = engine.create_template(&name, &vars, &inherits)?;

            if verbose {
                println!("✓ 已创建模板: {}", template.name);
                println!("  变量: {}", template.variables.len());
                if !template.inherits.is_empty() {
                    println!("  继承: {}", template.inherits.join(", "));
                }
            }
        }

        TemplateCommands::List { verbose: list_verbose } => {
            let templates = engine.list_templates()?;

            if templates.is_empty() {
                println!("暂无模板");
                return Ok(());
            }

            for template in templates {
                println!("{}", template.name);

                if list_verbose {
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
        }

        TemplateCommands::Show { name } => {
            let template = engine.get_template(&name)?;

            println!("模板: {}", template.name);
            println!("\n变量:");

            for var in &template.variables {
                let required = if var.required { "必需" } else { "可选" };
                match &var.default {
                    Some(default) => println!("  {} = {} ({})", var.name, default, required),
                    None => println!("  {} ({})", var.name, required),
                }
            }

            if !template.inherits.is_empty() {
                println!("\n继承: {}", template.inherits.join(", "));
            }

            println!("\n内容:");
            println!("{}", template.content);
        }

        TemplateCommands::Render { name, var, interactive, output } => {
            // 解析变量参数
            let mut variables = HashMap::new();
            for v in &var {
                if let Some(pos) = v.find('=') {
                    let key = v[..pos].to_string();
                    let value = v[pos + 1..].to_string();
                    variables.insert(key, value);
                }
            }

            // 交互式模式：检查缺失变量
            if interactive {
                let template = engine.get_template(&name)?;
                for var_def in &template.variables {
                    if !variables.contains_key(&var_def.name) {
                        if var_def.required {
                            println!("请输入必需变量 {}: ", var_def.name);
                            let mut input = String::new();
                            std::io::stdin().read_line(&mut input).map_err(|e| {
                                EnvError::Io(std::io::Error::other(e))
                            })?;
                            variables.insert(var_def.name.clone(), input.trim().to_string());
                        } else if let Some(default) = &var_def.default {
                            println!("变量 {} (默认: {}): ", var_def.name, default);
                            let mut input = String::new();
                            std::io::stdin().read_line(&mut input).map_err(|e| {
                                EnvError::Io(std::io::Error::other(e))
                            })?;
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
            let result = engine.render_template(&name, &variables)?;

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
        }

        TemplateCommands::Delete { name } => {
            let deleted = engine.delete_template(&name)?;

            if deleted {
                if verbose {
                    println!("✓ 已删除模板: {}", name);
                }
            } else {
                return Err(EnvError::TemplateNotFound(name));
            }
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
    if let Ok(system_vars) = utils::paths::get_system_env()
        && system_vars.len() > 100
    {
        println!("ℹ️  系统环境变量较多 ({}), 建议使用 --source 过滤", system_vars.len());
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

/// 处理插件子命令
fn run_plugin_command(command: PluginCommands, verbose: bool) -> Result<()> {
    match command {
        // 列出插件
        PluginCommands::List { verbose: list_verbose, show_disabled } => {
            let manager = PluginManager::new()?;
            let plugins = manager.list_plugins(show_disabled);

            if plugins.is_empty() {
                println!("暂无插件");
                return Ok(());
            }

            for plugin_info in plugins {
                let status = if plugin_info.metadata.enabled { "✓" } else { "✗" };
                println!("{} {} ({})", status, plugin_info.metadata.name, plugin_info.metadata.id);

                if list_verbose {
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
        }

        // 查看插件详情
        PluginCommands::Show { plugin_id } => {
            let manager = PluginManager::new()?;
            let plugin_info = manager
                .get_plugin_info(&plugin_id)
                .ok_or_else(|| EnvError::PluginNotFound(plugin_id.clone()))?;

            println!("插件 ID: {}", plugin_info.metadata.id);
            println!("名称: {}", plugin_info.metadata.name);
            println!("版本: {}", plugin_info.metadata.version);
            println!("类型: {:?}", plugin_info.metadata.plugin_type);
            println!("状态: {}", if plugin_info.metadata.enabled { "已启用" } else { "已禁用" });

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
                    println!("  {} ({:?}): {}{}", field.name, field.field_type, required, default);
                    if let Some(desc) = &field.description {
                        println!("    {}", desc);
                    }
                }
            }
        }

        // 启用插件
        PluginCommands::Enable { plugin_id } => {
            let mut manager = PluginManager::new()?;
            manager
                .enable_plugin(&plugin_id)
                .map_err(|e| EnvError::PluginExecutionFailed(e.to_string()))?;

            if verbose {
                println!("✓ 已启用插件: {}", plugin_id);
            }
        }

        // 禁用插件
        PluginCommands::Disable { plugin_id } => {
            let mut manager = PluginManager::new()?;
            manager
                .disable_plugin(&plugin_id)
                .map_err(|e| EnvError::PluginExecutionFailed(e.to_string()))?;

            if verbose {
                println!("✓ 已禁用插件: {}", plugin_id);
            }
        }

        // 加载插件
        PluginCommands::Load { path, config: _ } => {
            let mut manager = PluginManager::new()?;
            let path_buf = PathBuf::from(&path);

            // 加载插件
            manager
                .load_from_path(&path_buf)
                .map_err(|e| EnvError::PluginLoadFailed(e.to_string()))?;

            if verbose {
                println!("✓ 已加载插件: {}", path);
            }
        }

        // 卸载插件
        PluginCommands::Unload { plugin_id } => {
            let mut manager = PluginManager::new()?;
            manager
                .unload_plugin(&plugin_id)
                .map_err(|e| EnvError::PluginExecutionFailed(e.to_string()))?;

            if verbose {
                println!("✓ 已卸载插件: {}", plugin_id);
            }
        }

        // 热重载插件
        PluginCommands::Reload { plugin_id } => {
            let mut manager = PluginManager::new()?;
            let new_id = manager
                .reload(&plugin_id)
                .map_err(|e| EnvError::PluginExecutionFailed(e.to_string()))?;

            if verbose {
                if new_id == plugin_id {
                    println!("✓ 已重载插件: {}", plugin_id);
                } else {
                    println!("✓ 已重载插件: {} -> {}", plugin_id, new_id);
                }
            }
        }

        // 查看插件状态
        PluginCommands::Status { plugin_id } => {
            let manager = PluginManager::new()?;

            match plugin_id {
                Some(id) => {
                    // 显示单个插件状态
                    let info = manager
                        .get_plugin_info(&id)
                        .ok_or_else(|| EnvError::PluginNotFound(id.clone()))?;

                    println!("插件: {}", info.metadata.name);
                    println!("状态: {}", if info.metadata.enabled { "已启用" } else { "已禁用" });
                    println!("已加载: {}", manager.is_loaded(&id));

                    let stats = manager.get_stats();
                    println!("执行次数: {}", stats.total_executions);
                    println!("错误次数: {}", stats.total_errors);
                }
                None => {
                    // 显示所有插件状态统计
                    let stats = manager.get_stats();
                    let plugins = manager.list_plugins(true);

                    println!("插件总数: {}", plugins.len());
                    println!("已启用: {}", plugins.iter().filter(|p| p.metadata.enabled).count());
                    println!("已加载: {}", stats.loaded_plugins);
                    println!("总执行次数: {}", stats.total_executions);
                    println!("错误次数: {}", stats.total_errors);

                    if verbose && !plugins.is_empty() {
                        println!("\n详细状态:");
                        for plugin in plugins {
                            let status = if plugin.metadata.enabled { "✓" } else { "✗" };
                            let loaded = if manager.is_loaded(&plugin.metadata.id) {
                                "已加载"
                            } else {
                                "未加载"
                            };
                            println!("  {} {} - {} ({})", status, plugin.metadata.name, loaded, plugin.metadata.id);
                        }
                    }
                }
            }
        }

        // 测试插件钩子
        PluginCommands::Test { plugin_id, hook } => {
            let manager = PluginManager::new()?;

            // 获取插件信息
            let info = manager
                .get_plugin_info(&plugin_id)
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
        }

        // 配置管理子命令（简化版：仅占位实现）
        PluginCommands::Config(config_cmd) => match config_cmd {
            // 设置配置（简化：仅显示提示）
            PluginConfigCommands::Set { plugin_id, key, value } => {
                if verbose {
                    println!("⚠️  配置管理功能暂未完全实现");
                    println!("   插件: {}, 配置: {} = {}", plugin_id, key, value);
                }
            }

            // 获取配置（简化：显示提示）
            PluginConfigCommands::Get { plugin_id, key } => {
                if verbose {
                    println!("⚠️  配置管理功能暂未完全实现");
                    println!("   插件: {}, 配置项: {:?}", plugin_id, key);
                }
            }

            // 重置配置（简化：显示提示）
            PluginConfigCommands::Reset { plugin_id } => {
                if verbose {
                    println!("⚠️  配置管理功能暂未完全实现");
                    println!("   插件: {}", plugin_id);
                }
            }

            // 导出配置（简化：显示提示）
            PluginConfigCommands::Export => {
                println!("⚠️  配置管理功能暂未完全实现");
            }

            // 导入配置（简化：显示提示）
            PluginConfigCommands::Import { file } => {
                if verbose {
                    println!("⚠️  配置管理功能暂未完全实现");
                    println!("   文件: {}", file);
                }
            }
        },

        // 检查插件依赖
        PluginCommands::CheckDeps { plugin_id } => {
            let manager = PluginManager::new()?;

            match plugin_id {
                Some(id) => {
                    // 检查单个插件
                    let (satisfied, missing) = manager.check_dependencies(&id);

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
                }
                None => {
                    // 检查所有插件
                    match manager.validate_all_dependencies() {
                        Ok(()) => println!("✓ 所有插件依赖关系有效"),
                        Err(e) => println!("✗ 依赖验证失败: {}", e),
                    }
                }
            }
        }

        // 加载插件及其依赖
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
                }
                Err(e) => return Err(EnvError::PluginExecutionFailed(e.to_string())),
            }
        }

        // 生成密钥对
        PluginCommands::GenerateKeyPair => {
            match PluginManager::generate_key_pair() {
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
                }
                Err(e) => return Err(EnvError::PluginExecutionFailed(e.to_string())),
            }
        }

        // 为插件生成签名
        PluginCommands::Sign { plugin_id, key, algorithm, output } => {
            let manager = PluginManager::new()?;

            // 解析算法
            let sig_algorithm = match algorithm.as_str() {
                "Ed25519" => SignatureAlgorithm::Ed25519,
                _ => return Err(EnvError::PluginExecutionFailed("不支持的签名算法，仅支持 Ed25519".to_string())),
            };

            match manager.sign_plugin(&plugin_id, &key, sig_algorithm) {
                Ok(signature) => {
                    let signature_json = serde_json::to_string_pretty(&signature)
                        .map_err(|e| EnvError::PluginExecutionFailed(e.to_string()))?;

                    if let Some(output_path) = output {
                        std::fs::write(&output_path, &signature_json)
                            .map_err(EnvError::Io)?;
                        println!("✓ 签名已保存到 {}", output_path);
                    } else {
                        println!("✓ 签名生成成功:");
                        println!("{}", signature_json);
                    }
                }
                Err(e) => return Err(EnvError::PluginExecutionFailed(e.to_string())),
            }
        }

        // 验证插件签名
        PluginCommands::Verify { plugin_id, trust_unsigned } => {
            let manager = PluginManager::new()?;

            match manager.verify_plugin_signature(&plugin_id, trust_unsigned) {
                Ok(()) => {
                    println!("✓ 插件 {} 签名验证通过", plugin_id);
                }
                Err(e) => {
                    println!("✗ 插件 {} 签名验证失败: {}", plugin_id, e);
                    return Err(EnvError::PluginExecutionFailed(e.to_string()));
                }
            }
        }

        // 验证所有插件签名
        PluginCommands::VerifyAll { trust_unsigned } => {
            let manager = PluginManager::new()?;

            match manager.verify_all_signatures(trust_unsigned) {
                Ok(()) => {
                    println!("✓ 所有插件签名验证通过");
                }
                Err(e) => {
                    println!("✗ 签名验证失败: {}", e);
                    return Err(EnvError::PluginExecutionFailed(e.to_string()));
                }
            }
        }

        // 显示公钥指纹
        PluginCommands::Fingerprint { public_key } => {
            let fingerprint = PluginManager::fingerprint(&public_key);
            println!("公钥指纹: {}", fingerprint);
        }
    }

    Ok(())
}
