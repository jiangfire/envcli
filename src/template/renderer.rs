//! 模板渲染器

use crate::error::{EnvError, Result};
use crate::template::{Template, TemplateEngine};
use regex::Regex;
use std::collections::HashMap;

/// 渲染模板
pub fn render(
    template: &Template,
    variables: &HashMap<String, String>,
    engine: &TemplateEngine,
) -> Result<String> {
    // 1. 处理继承链
    let mut merged_content = String::new();
    let mut processed_inherits = Vec::new();

    // 递归处理继承
    fn process_inherits(
        template: &Template,
        engine: &TemplateEngine,
        processed: &mut Vec<String>,
        content: &mut String,
    ) -> Result<()> {
        // 检测循环继承
        if processed.contains(&template.name) {
            return Err(EnvError::CircularInheritance(format!(
                "检测到循环继承: {}",
                processed.join(" -> ")
            )));
        }

        processed.push(template.name.clone());

        // 处理父模板
        for inherit in &template.inherits {
            let parent = engine.get_template(inherit)?;
            process_inherits(&parent, engine, processed, content)?;
        }

        // 添加当前模板内容
        content.push_str(&template.content);
        content.push('\n');

        Ok(())
    }

    process_inherits(
        template,
        engine,
        &mut processed_inherits,
        &mut merged_content,
    )?;

    // 2. 填充变量
    let mut result = merged_content.clone();

    // 正则表达式：匹配 {{VAR}} 或 {{VAR|default}}
    let var_pattern = Regex::new(r"\{\{([^}]+)\}\}").unwrap();

    // 遍历所有匹配的占位符
    for caps in var_pattern.captures_iter(&merged_content) {
        let full_match = caps.get(0).unwrap().as_str();
        let var_expr = caps.get(1).unwrap().as_str().trim();

        // 解析变量名和默认值
        let (var_name, default_value) = if let Some(pos) = var_expr.find('|') {
            let name = var_expr[..pos].trim();
            let default = var_expr[pos + 1..].trim();
            (name, Some(default.to_string()))
        } else {
            (var_expr, None)
        };

        // 获取实际值
        let value = if let Some(v) = variables.get(var_name) {
            v.clone()
        } else if let Some(default) = default_value {
            default
        } else {
            return Err(EnvError::MissingVariable(var_name.to_string()));
        };

        // 替换占位符
        result = result.replace(full_match, &value);
    }

    // 3. 清理注释和空行
    let cleaned: Vec<String> = result
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with("# @inherits")
        })
        .map(|s| s.to_string())
        .collect();

    Ok(cleaned.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template::parser;

    #[test]
    fn test_render_with_all_vars() {
        let content = "DB_HOST={{DB_HOST}}\nDB_PORT={{DB_PORT}}";
        let template = parser::parse_template("test", content).unwrap();

        let mut vars = HashMap::new();
        vars.insert("DB_HOST".to_string(), "localhost".to_string());
        vars.insert("DB_PORT".to_string(), "5432".to_string());

        let engine = TemplateEngine::new().unwrap();
        let result = render(&template, &vars, &engine).unwrap();

        assert!(result.contains("DB_HOST=localhost"));
        assert!(result.contains("DB_PORT=5432"));
    }

    #[test]
    fn test_render_with_defaults() {
        let content = "DB_HOST={{DB_HOST}}\nDB_PORT={{DB_PORT|5432}}";
        let template = parser::parse_template("test", content).unwrap();

        let mut vars = HashMap::new();
        vars.insert("DB_HOST".to_string(), "localhost".to_string());
        // 不提供 DB_PORT，应该使用默认值

        let engine = TemplateEngine::new().unwrap();
        let result = render(&template, &vars, &engine).unwrap();

        assert!(result.contains("DB_HOST=localhost"));
        assert!(result.contains("DB_PORT=5432"));
    }

    #[test]
    fn test_render_missing_required() {
        let content = "DB_HOST={{DB_HOST}}\nDB_PASS={{DB_PASS}}";
        let template = parser::parse_template("test", content).unwrap();

        let mut vars = HashMap::new();
        vars.insert("DB_HOST".to_string(), "localhost".to_string());
        // 缺少 DB_PASS

        let engine = TemplateEngine::new().unwrap();
        let result = render(&template, &vars, &engine);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("DB_PASS"));
    }

    #[test]
    fn test_render_inheritance_chain() {
        // 创建基础模板
        let engine = TemplateEngine::new().unwrap();
        engine
            .create_template("base", &["BASE_VAR".to_string()], &[])
            .unwrap();
        engine
            .create_template("middle", &["MIDDLE_VAR".to_string()], &["base".to_string()])
            .unwrap();

        // 渲染中间模板
        let middle_template = engine.get_template("middle").unwrap();
        let mut vars = HashMap::new();
        vars.insert("BASE_VAR".to_string(), "base_value".to_string());
        vars.insert("MIDDLE_VAR".to_string(), "middle_value".to_string());

        let result = render(&middle_template, &vars, &engine).unwrap();

        assert!(result.contains("BASE_VAR=base_value"));
        assert!(result.contains("MIDDLE_VAR=middle_value"));

        // 清理
        engine.delete_template("base").unwrap();
        engine.delete_template("middle").unwrap();
    }

    #[test]
    fn test_circular_inheritance_detection() {
        let engine = TemplateEngine::new().unwrap();
        engine
            .create_template("a", &["A".to_string()], &["b".to_string()])
            .unwrap();
        engine
            .create_template("b", &["B".to_string()], &["a".to_string()])
            .unwrap();

        let a_template = engine.get_template("a").unwrap();
        let vars = HashMap::new();

        let result = render(&a_template, &vars, &engine);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("循环继承"));

        // 清理
        engine.delete_template("a").unwrap();
        engine.delete_template("b").unwrap();
    }
}
