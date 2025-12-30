//! CLI 参数解析 (使用 clap，遵循传统原则)

use crate::error::EnvError;
use crate::types::{EnvSource, OutputFormat};
use clap::{Parser, Subcommand};

/// EnvCLI - 跨平台环境变量管理工具
///
/// 设计哲学：默认安静，细节可见，组合优先
/// 支持管道操作和文本流
#[derive(Parser)]
#[command(
    name = "envcli",
    version = "0.1.0",
    about = "跨平台环境变量管理工具",
    long_about = "一个简单、高效、跨平台的环境变量管理工具，支持多层级配置和格式转换"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// 详细输出模式 (支持安静/详细错误切换)
    #[arg(short, long, global = true, help = "详细输出模式")]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    // ==================== 核心操作 ====================

    /// 获取环境变量
    ///
    /// # 示例
    /// envcli get DB_HOST
    /// envcli get DB_HOST --verbose  # 详细错误信息
    Get {
        /// 变量名称
        key: String,
    },

    /// 设置环境变量 (写入 Local 层)
    ///
    /// # 示例
    /// envcli set DB_HOST localhost
    /// envcli set DB_PORT 5432
    Set {
        /// 变量名称
        key: String,

        /// 变量值
        value: String,
    },

    /// 删除环境变量 (从 Local 层)
    ///
    /// # 示例
    /// envcli unset DB_HOST
    Unset {
        /// 变量名称
        key: String,
    },

    /// 列出环境变量
    ///
    /// # 示例
    /// envcli list                           # 所有层级合并
    /// envcli list --source=project          # 仅项目级
    /// envcli list --format=json             # JSON输出
    List {
        /// 指定来源：system/user/project/local
        #[arg(short, long, value_name = "LEVEL")]
        source: Option<String>,

        /// 输出格式：env/json
        #[arg(short, long, default_value = "env")]
        format: String,
    },

    // ==================== 系统级操作 ====================

    /// 设置系统级永久环境变量
    ///
    /// # 示例
    /// envcli system-set DB_HOST localhost
    /// envcli system-set API_KEY secret --scope machine
    SystemSet {
        /// 变量名称
        key: String,

        /// 变量值
        value: String,

        /// 作用域：global (用户级) 或 machine (系统级，需要管理员权限)
        #[arg(short, long, default_value = "global")]
        scope: String,
    },

    /// 删除系统级环境变量
    ///
    /// # 示例
    /// envcli system-unset DB_HOST
    /// envcli system-unset API_KEY --scope machine
    SystemUnset {
        /// 变量名称
        key: String,

        /// 作用域
        #[arg(short, long, default_value = "global")]
        scope: String,
    },

    // ==================== 导入导出 ====================

    /// 导入 .env 文件
    ///
    /// # 示例
    /// envcli import .env                    # 导入到 Local 层
    /// envcli import config/env --target=project
    Import {
        /// 文件路径
        file: String,

        /// 目标层级：user/project/local (默认: local)
        #[arg(short, long, default_value = "local", value_name = "LEVEL")]
        target: String,
    },

    /// 导出环境变量
    ///
    /// # 示例
    /// envcli export > backup.env           # 导出所有
    /// envcli export --source=project       # 仅项目级
    /// envcli export --format=json          # JSON输出
    Export {
        /// 指定来源，不指定则合并所有
        #[arg(short, long, value_name = "LEVEL")]
        source: Option<String>,

        /// 输出格式：env/json
        #[arg(short, long, default_value = "env")]
        format: String,
    },

    // ==================== 加密解密 ====================

    /// 设置变量（支持加密选项）
    ///
    /// # 示例
    /// envcli set DB_PASS secret --encrypt
    /// envcli set DB_HOST localhost  # 明文
    SetEncrypt {
        /// 变量名称
        key: String,

        /// 变量值
        value: String,

        /// 使用 SOPS 加密存储
        #[arg(short, long)]
        encrypt: bool,
    },

    /// 加密环境变量（使用 SOPS）
    ///
    /// # 示例
    /// envcli encrypt DB_PASS my_secret_password
    /// envcli encrypt API_KEY secret_key --target=project
    Encrypt {
        /// 变量名称
        key: String,

        /// 变量值
        value: String,

        /// 目标层级：user/project/local (默认: local)
        #[arg(short, long, default_value = "local", value_name = "LEVEL")]
        target: String,
    },

    /// 解密环境变量
    ///
    /// # 示例
    /// envcli decrypt DB_PASS
    /// envcli decrypt API_KEY --source=project
    Decrypt {
        /// 变量名称
        key: String,

        /// 来源层级：system/user/project/local (默认: 按优先级查找)
        #[arg(short, long, value_name = "LEVEL")]
        source: Option<String>,
    },

    /// 检查 SOPS 是否可用
    ///
    /// # 示例
    /// envcli check-sops
    CheckSops,

    // ==================== 运行时 ====================

    /// 运行命令并注入临时环境变量 (12-factor 风格)
    ///
    /// # 示例
    /// envcli run DB_HOST=localhost DB_PORT=5432 -- python app.py
    /// envcli run API_KEY=secret -- cargo run
    /// envcli run --from-file .env.production -- npm start
    Run {
        /// 临时环境变量 (KEY=VALUE)
        #[arg(short, long, value_name = "KEY=VALUE", action = clap::ArgAction::Append)]
        env: Vec<String>,

        /// 从 .env 文件加载变量
        #[arg(short, long)]
        from_file: Option<String>,

        /// 要执行的命令和参数
        #[arg(required = true, last = true)]
        command: Vec<String>,
    },

    // ==================== 模板管理 ====================

    /// 模板管理命令组
    Template {
        #[command(subcommand)]
        command: TemplateCommands,
    },

    // ==================== 插件管理 ====================

    /// 插件管理命令组
    ///
    /// # 示例
    /// envcli plugin list
    /// envcli plugin load /path/to/plugin.so
    /// envcli plugin enable my-plugin
    Plugin {
        #[command(subcommand)]
        command: PluginCommands,
    },

    // ==================== 系统工具 ====================

    /// 配置管理命令组
    ///
    /// # 示例
    /// envcli config validate
    /// envcli config init
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },

    /// 显示当前状态
    ///
    /// 显示配置目录、层级文件存在状态等
    Status,

    /// 诊断问题
    ///
    /// 检查常见问题并提供建议
    Doctor,
}

/// 配置管理子命令
#[derive(Subcommand)]
pub enum ConfigCommands {
    /// 验证配置文件格式和完整性
    ///
    /// # 示例
    /// envcli config validate
    /// envcli config validate --verbose
    Validate {
        /// 显示详细信息
        #[arg(short, long)]
        verbose: bool,
    },

    /// 初始化配置目录和默认文件
    ///
    /// # 示例
    /// envcli config init
    /// envcli config init --force
    Init {
        /// 强制重新初始化（覆盖现有文件）
        #[arg(short, long)]
        force: bool,
    },

    /// 显示配置信息
    ///
    /// # 示例
    /// envcli config info
    Info,
}

/// 模板管理子命令
#[derive(Subcommand)]
pub enum TemplateCommands {
    /// 创建新模板
    ///
    /// # 示例
    /// envcli template create db --vars DB_HOST DB_PORT DB_USER DB_PASS
    /// envcli template create web --inherits db --vars APP_ENV API_URL
    Create {
        /// 模板名称
        name: String,

        /// 变量列表
        #[arg(short = 's', long, value_delimiter = ',', num_args = 1..)]
        vars: Vec<String>,

        /// 继承的父模板
        #[arg(short = 'i', long, value_delimiter = ',', num_args = 1..)]
        inherits: Vec<String>,
    },

    /// 列出所有模板
    ///
    /// # 示例
    /// envcli template list
    /// envcli template list --verbose
    List {
        /// 显示详细信息
        #[arg(short, long)]
        verbose: bool,
    },

    /// 查看模板详情
    ///
    /// # 示例
    /// envcli template show db
    Show {
        /// 模板名称
        name: String,
    },

    /// 渲染模板
    ///
    /// # 示例
    /// envcli template render db --var DB_HOST=localhost -o .env
    /// envcli template render web --interactive
    Render {
        /// 模板名称
        name: String,

        /// 变量值 (KEY=VALUE)
        #[arg(long, value_name = "KEY=VALUE", action = clap::ArgAction::Append)]
        var: Vec<String>,

        /// 交互式模式（提示缺失变量）
        #[arg(short = 'i', long)]
        interactive: bool,

        /// 输出文件（默认输出到 stdout）
        #[arg(short = 'o', long)]
        output: Option<String>,
    },

    /// 删除模板
    ///
    /// # 示例
    /// envcli template delete db
    Delete {
        /// 模板名称
        name: String,
    },
}

/// 插件管理子命令
#[derive(Subcommand)]
pub enum PluginCommands {
    /// 列出所有插件
    ///
    /// # 示例
    /// envcli plugin list
    /// envcli plugin list --verbose
    /// envcli plugin list --show-disabled
    List {
        /// 显示详细信息
        #[arg(short, long)]
        verbose: bool,

        /// 显示已禁用的插件
        #[arg(short, long)]
        show_disabled: bool,
    },

    /// 查看插件详情
    ///
    /// # 示例
    /// envcli plugin show my-plugin
    Show {
        /// 插件 ID
        plugin_id: String,
    },

    /// 启用插件
    ///
    /// # 示例
    /// envcli plugin enable my-plugin
    Enable {
        /// 插件 ID
        plugin_id: String,
    },

    /// 禁用插件
    ///
    /// # 示例
    /// envcli plugin disable my-plugin
    Disable {
        /// 插件 ID
        plugin_id: String,
    },

    /// 加载插件
    ///
    /// # 示例
    /// envcli plugin load /path/to/plugin.so
    /// envcli plugin load /path/to/plugin.py --config /path/to/config.toml
    Load {
        /// 插件路径 (动态库或可执行文件)
        path: String,

        /// 配置文件 (可选)
        #[arg(short, long)]
        config: Option<String>,
    },

    /// 卸载插件
    ///
    /// # 示例
    /// envcli plugin unload my-plugin
    Unload {
        /// 插件 ID
        plugin_id: String,
    },

    /// 热重载插件
    ///
    /// # 示例
    /// envcli plugin reload my-plugin
    Reload {
        /// 插件 ID
        plugin_id: String,
    },

    /// 查看插件状态
    ///
    /// # 示例
    /// envcli plugin status
    /// envcli plugin status my-plugin
    Status {
        /// 插件 ID (可选)
        plugin_id: Option<String>,
    },

    /// 测试插件钩子
    ///
    /// # 示例
    /// envcli plugin test my-plugin --hook PreCommand
    Test {
        /// 插件 ID
        plugin_id: String,

        /// 钩子类型 (可选，默认测试所有)
        #[arg(short, long)]
        hook: Option<String>,
    },

    /// 检查插件依赖
    ///
    /// # 示例
    /// envcli plugin check-deps my-plugin
    /// envcli plugin check-deps --all
    CheckDeps {
        /// 插件 ID (可选，检查所有)
        plugin_id: Option<String>,
    },

    /// 加载插件及其依赖
    ///
    /// # 示例
    /// envcli plugin load-deps /path/to/plugin1.dll /path/to/plugin2.dll
    LoadDeps {
        /// 插件路径列表
        paths: Vec<String>,
    },

    /// 配置插件
    #[command(subcommand)]
    Config(PluginConfigCommands),

    /// 生成密钥对（用于签名）
    ///
    /// # 示例
    /// envcli plugin generate-key-pair
    GenerateKeyPair,

    /// 为插件生成签名
    ///
    /// # 示例
    /// envcli plugin sign my-plugin --key <private_key_hex> --algorithm Ed25519
    /// envcli plugin sign my-plugin --key <private_key_hex> --output signature.json
    Sign {
        /// 插件 ID
        plugin_id: String,

        /// 私钥（十六进制编码）
        #[arg(short, long)]
        key: String,

        /// 签名算法 (默认: Ed25519)
        #[arg(short, long, default_value = "Ed25519")]
        algorithm: String,

        /// 输出到文件
        #[arg(short, long)]
        output: Option<String>,
    },

    /// 验证插件签名
    ///
    /// # 示例
    /// envcli plugin verify my-plugin
    /// envcli plugin verify my-plugin --trust-unsigned
    Verify {
        /// 插件 ID
        plugin_id: String,

        /// 信任未签名的插件
        #[arg(short, long)]
        trust_unsigned: bool,
    },

    /// 验证所有插件签名
    ///
    /// # 示例
    /// envcli plugin verify-all
    /// envcli plugin verify-all --trust-unsigned
    VerifyAll {
        /// 信任未签名的插件
        #[arg(short, long)]
        trust_unsigned: bool,
    },

    /// 显示公钥指纹
    ///
    /// # 示例
    /// envcli plugin fingerprint <public_key_hex>
    Fingerprint {
        /// 公钥（十六进制编码）
        public_key: String,
    },
}

/// 插件配置子命令
#[derive(Subcommand)]
pub enum PluginConfigCommands {
    /// 设置插件配置项
    ///
    /// # 示例
    /// envcli plugin config set my-plugin timeout 60
    /// envcli plugin config set my-plugin webhook_url "https://..."
    Set {
        /// 插件 ID
        plugin_id: String,

        /// 配置键
        key: String,

        /// 配置值
        value: String,
    },

    /// 获取插件配置
    ///
    /// # 示例
    /// envcli plugin config get my-plugin
    /// envcli plugin config get my-plugin timeout
    Get {
        /// 插件 ID
        plugin_id: String,

        /// 配置键 (可选)
        key: Option<String>,
    },

    /// 重置插件配置
    ///
    /// # 示例
    /// envcli plugin config reset my-plugin
    Reset {
        /// 插件 ID
        plugin_id: String,
    },

    /// 导出插件配置
    ///
    /// # 示例
    /// envcli plugin config export > plugins.toml
    Export,

    /// 导入插件配置
    ///
    /// # 示例
    /// envcli plugin config import plugins.toml
    Import {
        /// 配置文件路径
        file: String,
    },
}

/// 解析输出格式
pub fn parse_format(format_str: &str) -> OutputFormat {
    OutputFormat::from(format_str)
}

/// 验证来源是否有效且可写（针对写入操作）
pub fn validate_writable_source(source_str: &str) -> Result<EnvSource, EnvError> {
    let source = EnvSource::from_str(source_str)
        .ok_or_else(|| EnvError::InvalidSource(source_str.to_string()))?;

    if !source.is_writable() {
        return Err(EnvError::PermissionDenied(format!(
            "层级 {} 不可写",
            source
        )));
    }

    Ok(source)
}

/// 为 list 命令解析 source，允许为 None
pub fn parse_list_source(source_str: Option<&str>) -> Result<Option<EnvSource>, EnvError> {
    match source_str {
        Some(s) => {
            let source =
                EnvSource::from_str(s).ok_or_else(|| EnvError::InvalidSource(s.to_string()))?;
            Ok(Some(source))
        }
        None => Ok(None),
    }
}

/// 验证作用域参数
pub fn validate_scope(scope: &str) -> Result<(), EnvError> {
    if scope != "global" && scope != "machine" {
        return Err(EnvError::InvalidArgument(
            "scope 必须是 'global' 或 'machine'".to_string(),
        ));
    }
    Ok(())
}
