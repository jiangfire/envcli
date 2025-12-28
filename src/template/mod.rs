//! 环境变量模板引擎
//!
//! 支持模板定义、变量填充和继承
//!
//! # 模板语法
//!
//! ```text
//! # .envcli/templates/db.env
//! DB_HOST={{DB_HOST}}
//! DB_PORT={{DB_PORT|5432}}
//! DB_USER={{DB_USER|admin}}
//!
//! # .envcli/templates/web.env
//! # @inherits db.env
//! APP_ENV={{APP_ENV|development}}
//! API_URL={{API_URL}}
//! ```
//!
//! # 使用示例
//!
//! ```bash
//! envcli template create db --vars DB_HOST DB_PORT DB_USER
//! envcli template render db --var DB_HOST=localhost -o .env
//! ```

pub mod parser;
pub mod renderer;

use crate::error::{EnvError, Result};
use crate::utils::paths;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 模板变量定义
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateVar {
    pub name: String,
    pub default: Option<String>,
    pub required: bool,
}

/// 模板结构
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Template {
    pub name: String,
    pub content: String,
    pub variables: Vec<TemplateVar>,
    pub inherits: Vec<String>,
}

/// 模板引擎
pub struct TemplateEngine {
    templates_dir: PathBuf,
}

impl TemplateEngine {
    /// 创建模板引擎实例
    pub fn new() -> Result<Self> {
        let templates_dir = paths::get_templates_dir()?;
        Ok(Self { templates_dir })
    }

    /// 创建模板
    pub fn create_template(
        &self,
        name: &str,
        vars: &[String],
        inherits: &[String],
    ) -> Result<Template> {
        // 确保模板目录存在
        if !paths::file_exists(&self.templates_dir) {
            std::fs::create_dir_all(&self.templates_dir)?;
        }

        // 生成模板内容
        let mut content = String::new();

        // 添加继承指令
        if !inherits.is_empty() {
            for inherit in inherits {
                content.push_str(&format!("# @inherits {}\n", inherit));
            }
            content.push('\n');
        }

        // 添加变量
        for var in vars {
            content.push_str(&format!("{}={{{{{}}}}}\n", var, var));
        }

        // 解析模板
        let template = parser::parse_template(name, &content)?;

        // 保存模板文件
        let template_path = self.templates_dir.join(format!("{}.env", name));
        paths::write_file_safe(&template_path, &content)?;

        Ok(template)
    }

    /// 获取模板
    pub fn get_template(&self, name: &str) -> Result<Template> {
        let template_path = self.templates_dir.join(format!("{}.env", name));

        if !paths::file_exists(&template_path) {
            return Err(EnvError::TemplateNotFound(name.to_string()));
        }

        let content = paths::read_file(&template_path)?;
        parser::parse_template(name, &content)
    }

    /// 列出所有模板
    pub fn list_templates(&self) -> Result<Vec<Template>> {
        // 检查目录是否存在（使用 exists() 而不是 file_exists()）
        if !self.templates_dir.exists() {
            return Ok(vec![]);
        }

        let mut templates = Vec::new();

        for entry in std::fs::read_dir(&self.templates_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && path.extension().is_some_and(|ext| ext == "env") {
                let name = path.file_stem().unwrap().to_string_lossy().to_string();
                let content = paths::read_file(&path)?;
                if let Ok(template) = parser::parse_template(&name, &content) {
                    templates.push(template);
                }
            }
        }

        Ok(templates)
    }

    /// 渲染模板
    pub fn render_template(
        &self,
        name: &str,
        variables: &std::collections::HashMap<String, String>,
    ) -> Result<String> {
        let template = self.get_template(name)?;
        renderer::render(&template, variables, self)
    }

    /// 删除模板
    pub fn delete_template(&self, name: &str) -> Result<bool> {
        let template_path = self.templates_dir.join(format!("{}.env", name));

        if !paths::file_exists(&template_path) {
            return Ok(false);
        }

        std::fs::remove_file(&template_path)?;
        Ok(true)
    }
}

impl Default for TemplateEngine {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_create_and_render_template() {
        let engine = TemplateEngine::new().unwrap();

        // 创建模板
        let template = engine
            .create_template("test", &["DB_HOST".to_string(), "DB_PORT".to_string()], &[])
            .unwrap();
        assert_eq!(template.name, "test");
        assert_eq!(template.variables.len(), 2);

        // 渲染模板
        let mut vars = HashMap::new();
        vars.insert("DB_HOST".to_string(), "localhost".to_string());
        vars.insert("DB_PORT".to_string(), "5432".to_string());

        let result = engine.render_template("test", &vars).unwrap();
        assert!(result.contains("DB_HOST=localhost"));
        assert!(result.contains("DB_PORT=5432"));

        // 清理
        engine.delete_template("test").unwrap();
    }

    #[test]
    fn test_list_templates() {
        let engine = TemplateEngine::new().unwrap();

        // 先清理可能存在的旧模板
        let _ = engine.delete_template("list1");
        let _ = engine.delete_template("list2");

        // 创建两个模板
        engine
            .create_template("list1", &["VAR1".to_string()], &[])
            .unwrap();
        engine
            .create_template("list2", &["VAR2".to_string()], &[])
            .unwrap();

        let templates = engine.list_templates().unwrap();

        // 至少应该有我们刚创建的两个
        let list1_found = templates.iter().any(|t| t.name == "list1");
        let list2_found = templates.iter().any(|t| t.name == "list2");

        assert!(list1_found, "list1 模板未找到");
        assert!(list2_found, "list2 模板未找到");

        // 清理
        engine.delete_template("list1").unwrap();
        engine.delete_template("list2").unwrap();
    }

    #[test]
    fn test_template_not_found() {
        let engine = TemplateEngine::new().unwrap();
        let result = engine.get_template("nonexistent");
        assert!(result.is_err());
    }
}
