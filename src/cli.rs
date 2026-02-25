//! CLI 参数定义

use clap::{Parser, Subcommand};

/// EnvCLI - 跨平台环境变量管理工具
#[derive(Parser)]
#[command(
    name = "envcli",
    version = "0.3.0",
    about = "跨平台环境变量管理工具",
    long_about = "一个简单、高效、跨平台的环境变量管理工具，支持多层级配置和格式转换"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// 详细输出模式
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 获取环境变量
    Get {
        /// 变量名称
        key: String,
    },

    /// 设置环境变量
    Set {
        /// 变量名称
        key: String,
        /// 变量值
        value: String,
        /// 目标层级 (user/project/local)
        #[arg(short, long, default_value = "local")]
        target: String,
    },

    /// 删除环境变量
    Unset {
        /// 变量名称
        key: String,
        /// 目标层级
        #[arg(short, long, default_value = "local")]
        target: String,
    },

    /// 列出环境变量
    List {
        /// 指定来源
        #[arg(short, long)]
        source: Option<String>,
        /// 输出格式 (env/json)
        #[arg(short, long, default_value = "env")]
        format: String,
    },

    /// 导出环境变量
    Export {
        /// 指定来源
        #[arg(short, long)]
        source: Option<String>,
        /// 输出格式
        #[arg(short, long, default_value = "env")]
        format: String,
    },

    /// 导入 .env 文件
    Import {
        /// 文件路径
        file: String,
        /// 目标层级
        #[arg(short, long, default_value = "local")]
        target: String,
    },

    /// 运行命令并注入环境变量
    Run {
        /// 临时环境变量 (KEY=VALUE)
        #[arg(short, long)]
        env: Vec<String>,
        /// 从文件加载变量
        #[arg(short, long)]
        from_file: Option<String>,
        /// 要执行的命令
        #[arg(required = true, last = true)]
        command: Vec<String>,
    },

    /// 显示当前状态
    Status,

    /// 诊断问题
    Doctor,

    /// 设置系统级环境变量
    SystemSet {
        /// 变量名称
        key: String,
        /// 变量值
        value: String,
        /// 作用域 (global/machine)
        #[arg(short, long, default_value = "global")]
        scope: String,
    },

    /// 删除系统级环境变量
    SystemUnset {
        /// 变量名称
        key: String,
        /// 作用域
        #[arg(short, long, default_value = "global")]
        scope: String,
    },

    /// 缓存管理
    #[command(subcommand)]
    Cache(CacheCommands),

    /// 配置管理
    #[command(subcommand)]
    Config(ConfigCommands),
}

#[derive(Subcommand)]
pub enum CacheCommands {
    /// 显示缓存统计
    Stats,
    /// 清除缓存
    Clear {
        /// 缓存类型 (file/system/all)
        cache_type: String,
    },
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// 验证配置
    Validate {
        #[arg(short, long)]
        verbose: bool,
    },
    /// 初始化配置
    Init {
        #[arg(short, long)]
        force: bool,
    },
    /// 显示配置信息
    Info,
}

/// 解析来源参数（可写）
pub fn parse_writable_source(source: &str) -> anyhow::Result<crate::domain::models::EnvSource> {
    use crate::domain::models::EnvSource;

    let s =
        EnvSource::parse(source).ok_or_else(|| anyhow::anyhow!("无效的环境层级: {}", source))?;

    if !s.is_writable() {
        return Err(anyhow::anyhow!("层级 {} 不可写", s));
    }

    Ok(s)
}

/// 解析来源参数
pub fn parse_source(source: Option<&str>) -> Option<crate::domain::models::EnvSource> {
    use crate::domain::models::EnvSource;
    source.and_then(EnvSource::parse)
}
