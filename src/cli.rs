//! CLI 参数解析 (使用 clap，遵循传统原则)

use clap::{Parser, Subcommand};
use crate::types::{EnvSource, OutputFormat};
use crate::error::EnvError;

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

    /// 显示当前状态
    ///
    /// 显示配置目录、层级文件存在状态等
    Status,

    /// 诊断问题
    ///
    /// 检查常见问题并提供建议
    Doctor,

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
        return Err(EnvError::PermissionDenied(format!("层级 {} 不可写", source)));
    }

    Ok(source)
}

/// 为 list 命令解析 source，允许为 None
pub fn parse_list_source(source_str: Option<&str>) -> Result<Option<EnvSource>, EnvError> {
    match source_str {
        Some(s) => {
            let source = EnvSource::from_str(s)
                .ok_or_else(|| EnvError::InvalidSource(s.to_string()))?;
            Ok(Some(source))
        }
        None => Ok(None),
    }
}