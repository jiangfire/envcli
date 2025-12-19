//! 环境变量合并器 (实现 12-factor 优先级规则)
//!
//! 优先级（从低到高）：
//! 1. 系统环境变量
//! 2. 用户级配置
//! 3. 项目级配置
//! 4. 本地级配置
//! 5. 临时变量（最高）

use std::collections::HashMap;
use crate::types::EnvSource;
use crate::core::Store;
use crate::error::{EnvError, Result};

/// 环境变量合并器
pub struct EnvMerger;

impl EnvMerger {
    /// 解析临时环境变量参数
    ///
    /// # 输入
    /// `["DB_HOST=localhost", "DB_PORT=5432"]`
    ///
    /// # 输出
    /// `[("DB_HOST", "localhost"), ("DB_PORT", "5432")]`
    pub fn parse_temp_vars(env_args: &[String]) -> Result<Vec<(String, String)>> {
        let mut vars = Vec::new();

        for arg in env_args {
            match arg.split_once('=') {
                Some((key, value)) if !key.is_empty() => {
                    vars.push((key.trim().to_string(), value.trim().to_string()));
                }
                _ => return Err(EnvError::EnvParseError(format!(
                    "无效的环境变量格式 '{}'，应为 KEY=VALUE",
                    arg
                ))),
            }
        }

        Ok(vars)
    }

    /// 从 .env 文件解析临时变量
    pub fn parse_file(path: &str) -> Result<Vec<(String, String)>> {
        // 读取文件内容
        let path = std::path::Path::new(path);
        if !path.exists() {
            return Err(EnvError::FileNotFound(path.to_path_buf()));
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| EnvError::Io(e))?;

        // 使用现有的 .env 解析器
        let vars = crate::config::format::dotenv::DotenvParser::parse(&content, &EnvSource::System)?;

        Ok(vars.into_iter().map(|v| (v.key, v.value)).collect())
    }

    /// 构建完整环境变量映射（按优先级合并）
    ///
    /// # 优先级顺序
    /// 临时变量 > Local > Project > User > System
    pub fn merge_environment(
        store: &Store,
        temp_vars: &[(String, String)],
    ) -> Result<HashMap<String, String>> {
        let mut env = HashMap::new();

        // 1. 系统环境（最低优先级）
        env.extend(crate::utils::paths::get_system_env()?);

        // 2. User → 3. Project → 4. Local (按顺序覆盖)
        for source in [EnvSource::User, EnvSource::Project, EnvSource::Local] {
            let vars = store.list(Some(source))?;
            for var in vars {
                env.insert(var.key, var.value);
            }
        }

        // 5. 临时变量（最高优先级）
        for (key, value) in temp_vars {
            env.insert(key.clone(), value.clone());
        }

        Ok(env)
    }
}
