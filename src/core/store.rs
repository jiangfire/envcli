//! 核心存储引擎 (模块原则：清晰分离的存储逻辑)

use crate::types::{Config, EnvSource, EnvVar};
use crate::error::{EnvError, Result};
use crate::utils::paths::{self, file_exists, read_file, write_file_safe, append_to_file_unique, get_system_env};
use crate::config::format::dotenv::DotenvParser;
use std::collections::HashMap;

/// 核心存储引擎 (遵循分离原则：接口与实现分离)
#[derive(Clone)]
pub struct Store {
    config: Config,
}

impl Store {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// 获取单个变量（按优先级合并）
    /// 优先级：Local > Project > User > System
    pub fn get(&self, key: &str) -> Result<Option<String>> {
        // 从高优先级向低优先级查找
        let sources = [
            EnvSource::Local,
            EnvSource::Project,
            EnvSource::User,
            EnvSource::System,
        ];

        for source in sources {
            if let Some(value) = self.get_from_source(key, &source)? {
                return Ok(Some(value));
            }
        }

        Ok(None)
    }

    /// 从指定源获取变量
    fn get_from_source(&self, key: &str, source: &EnvSource) -> Result<Option<String>> {
        // 系统层特殊处理
        if *source == EnvSource::System {
            let system_env = get_system_env()?;
            return Ok(system_env.get(key).cloned());
        }

        // 获取文件路径
        let path = paths::get_layer_path(source)?;

        // 文件不存在
        if !file_exists(&path) {
            return Ok(None);
        }

        // 读取并解析文件
        let content = read_file(&path)?;
        let vars = DotenvParser::parse(&content, &source)?;

        // 查找目标变量
        for var in vars {
            if var.key == key {
                return Ok(Some(var.value));
            }
        }

        Ok(None)
    }

    /// 设置变量（写入最高优先级可写层：Local）
    pub fn set(&self, key: String, value: String) -> Result<()> {
        // 确保本地目录存在
        paths::ensure_project_dir()?;

        // 获取本地层路径
        let path = paths::get_layer_path(&EnvSource::Local)?;

        // 文件格式：KEY=VALUE
        let line = format!("{}={}", key, value);

        // 使用安全写入，避免重复
        append_to_file_unique(&path, &line)?;

        if self.config.verbose {
            println!("✓ 设置变量 {} = {}", key, value);
        }

        Ok(())
    }

    /// 删除变量（从 Local 层中移除）
    pub fn unset(&self, key: &str) -> Result<bool> {
        let path = paths::get_layer_path(&EnvSource::Local)?;

        if !file_exists(&path) {
            return Ok(false);
        }

        let content = read_file(&path)?;
        let vars = DotenvParser::parse(&content, &EnvSource::Local)?;

        // 过滤掉目标变量
        let remaining: Vec<_> = vars.iter().filter(|v| v.key != key).cloned().collect();

        if remaining.len() == vars.len() {
            // 变量不存在
            return Ok(false);
        }

        // 重写文件
        if remaining.is_empty() {
            // 如果没有变量了，删除文件
            std::fs::remove_file(&path)?;
        } else {
            let new_content = DotenvParser::serialize(&remaining);
            write_file_safe(&path, &new_content)?;
        }

        if self.config.verbose {
            println!("✓ 删除变量 {}", key);
        }

        Ok(true)
    }

    /// 列出变量
    pub fn list(&self, source_filter: Option<EnvSource>) -> Result<Vec<EnvVar>> {
        match source_filter {
            Some(source) => self.list_from_source(&source),
            None => self.list_merged(),
        }
    }

    /// 列出指定源的变量
    fn list_from_source(&self, source: &EnvSource) -> Result<Vec<EnvVar>> {
        match source {
            EnvSource::System => {
                let env = get_system_env()?;

                Ok(env
                    .into_iter()
                    .map(|(k, v)| EnvVar::new(k, v, EnvSource::System))
                    .collect())
            }
            _ => {
                let path = paths::get_layer_path(source)?;

                if !file_exists(&path) {
                    return Ok(vec
![]);
                }

                let content = read_file(&path)?;
                let vars = DotenvParser::parse(&content, source)
?;
                Ok(vars)
            }
        }
    }

    /// 合并所有层级（应用优先级规则）
    fn list_merged(&self) -> Result<Vec<EnvVar>> {
        let mut map = HashMap::new();

        // 按优先级从低到高覆盖
        for source in [EnvSource::System, EnvSource::User, EnvSource::Project, EnvSource::Local] {
            let vars = self.list_from_source(&source)?;
            for var in vars {
                map.insert(var.key.clone(), var); // 高优先级覆盖低优先级
            }
        }

        Ok(map.into_values().collect())
    }

    /// 导入 .env 文件到指定层级
    pub fn import_file(&self, file_path: &str, target_source: &EnvSource) -> Result<usize> {
        if !target_source.is_writable() {
            return Err(EnvError::PermissionDenied(
                "目标层级不可写".to_string(),
            ));
        }

        let path = std::path::Path::new(file_path);

        if !file_exists(path) {
            return Err(EnvError::FileNotFound(path.to_path_buf()));
        }

        // 确保目录存在
        match target_source {
            EnvSource::User => paths::ensure_config_dir()?,
            EnvSource::Project | EnvSource::Local => paths::ensure_project_dir()?,
            _ => {}
        }

        // 读取并解析文件
        let content = read_file(path)?;
        let vars = DotenvParser::parse(&content, &EnvSource::System)?; // 临时标记为 System

        // 写入目标层级
        let target_path = paths::get_layer_path(target_source)?;
        let existing = if file_exists(&target_path) {
            let existing_content = read_file(&target_path)?;
            DotenvParser::parse(&existing_content, target_source)?
        } else {
            vec
![]
        };

        // 去重合并
        let mut final_vars = existing;
        let mut count = 0;

        for var in vars {
            if !final_vars.iter().any(|v| v.key == var.key)
 {
                final_vars.push(EnvVar::new(var.key, var.value, target_source.clone()));
                count += 1;
            }
        }

        // 写回文件
        let new_content = DotenvParser::serialize(&final_vars);
        write_file_safe(&target_path, &new_content)?;

        if self.config.verbose {
            println!("✓ 从 {} 导入了 {} 个变量到 {:?}", file_path, count, target_source);
        }

        Ok(count)
    }

    /// 导出变量到 .env 格式
    pub fn export(&self, source_filter: Option<EnvSource>) -> Result<String> {
        let vars = match source_filter {
            Some(ref source) => self.list_from_source(source)?,
            None => self.list_merged()?, // 默认合并所有
        };

        Ok(DotenvParser::serialize(&vars))
    }
}