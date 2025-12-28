//! 模板解析器
//!
//! 解析模板语法：{{VAR}} 或 {{VAR|default}}

use crate::error::{EnvError, Result};
use crate::template::{Template, TemplateVar};
use regex::Regex;
use std::collections::HashSet;

/// 解析模板内容
pub fn parse_template(name: &str, content: &str) -> Result<Template> {
    let mut variables = Vec::new();
    let mut inherits = Vec::new();
    let mut required_vars = HashSet::new();

    // 正则表达式：匹配 {{VAR}} 或 {{VAR|default}}
    let var_pattern = Regex::new(r"\{\{([^}]+)\}\}").unwrap();

    // 解析每一行
    for line in content.lines() {
        let trimmed = line.trim();

        // 跳过空行
        if trimmed.is_empty() {
            continue;
        }

        // 检查继承指令
        if trimmed.starts_with("# @inherits") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 3 {
                inherits.push(parts[2].to_string());
            }
            continue;
        }

        // 跳过普通注释
        if trimmed.starts_with('#') {
            continue;
        }

        // 提取变量
        for cap in var_pattern.captures_iter(trimmed) {
            let full_match = cap.get(0).unwrap().as_str();
            let var_expr = cap.get(1).unwrap().as_str().trim();

            // 解析变量表达式：VAR 或 VAR|default
            let (var_name, default_value) = if let Some(pos) = var_expr.find('|') {
                let name = var_expr[..pos].trim();
                let default = var_expr[pos + 1..].trim();
                (name, Some(default.to_string()))
            } else {
                (var_expr, None)
            };

            // 检查变量名是否有效
            if var_name.is_empty() {
                return Err(EnvError::ParseError(format!(
                    "空的变量名在模板 '{}'",
                    full_match
                )));
            }

            // 检查重复定义（同一行多次出现同一变量）
            if !required_vars.contains(var_name) {
                let required = default_value.is_none();
                variables.push(TemplateVar {
                    name: var_name.to_string(),
                    default: default_value.clone(),
                    required,
                });

                if required {
                    required_vars.insert(var_name.to_string());
                }
            }
        }
    }

    // 去重变量（保留第一个定义）
    let mut seen = HashSet::new();
    let unique_vars: Vec<TemplateVar> = variables
        .into_iter()
        .filter(|v| seen.insert(v.name.clone()))
        .collect();

    Ok(Template {
        name: name.to_string(),
        content: content.to_string(),
        variables: unique_vars,
        inherits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_variable() {
        let content = "DB_HOST={{DB_HOST}}";
        let template = parse_template("test", content).unwrap();

        assert_eq!(template.name, "test");
        assert_eq!(template.variables.len(), 1);
        assert_eq!(template.variables[0].name, "DB_HOST");
        assert_eq!(template.variables[0].default, None);
        assert!(template.variables[0].required);
    }

    #[test]
    fn test_parse_variable_with_default() {
        let content = "DB_PORT={{DB_PORT|5432}}";
        let template = parse_template("test", content).unwrap();

        assert_eq!(template.variables.len(), 1);
        assert_eq!(template.variables[0].name, "DB_PORT");
        assert_eq!(template.variables[0].default, Some("5432".to_string()));
        assert!(!template.variables[0].required);
    }

    #[test]
    fn test_parse_multiple_variables() {
        let content = r#"
DB_HOST={{DB_HOST}}
DB_PORT={{DB_PORT|5432}}
DB_USER={{DB_USER|admin}}
DB_PASS={{DB_PASS}}
        "#;

        let template = parse_template("test", content).unwrap();

        assert_eq!(template.variables.len(), 4);

        // 验证每个变量
        let host = &template.variables[0];
        assert_eq!(host.name, "DB_HOST");
        assert!(host.required);

        let port = &template.variables[1];
        assert_eq!(port.name, "DB_PORT");
        assert_eq!(port.default, Some("5432".to_string()));
        assert!(!port.required);

        let user = &template.variables[2];
        assert_eq!(user.name, "DB_USER");
        assert_eq!(user.default, Some("admin".to_string()));

        let pass = &template.variables[3];
        assert_eq!(pass.name, "DB_PASS");
        assert!(pass.required);
    }

    #[test]
    fn test_parse_inherits_directive() {
        let content = r#"
# @inherits db.env
# @inherits cache.env
APP_ENV={{APP_ENV}}
        "#;

        let template = parse_template("test", content).unwrap();

        assert_eq!(template.inherits.len(), 2);
        assert_eq!(template.inherits[0], "db.env");
        assert_eq!(template.inherits[1], "cache.env");
    }

    #[test]
    fn test_parse_with_comments() {
        let content = r#"
# 数据库配置
DB_HOST={{DB_HOST}}
# 端口，默认 5432
DB_PORT={{DB_PORT|5432}}
        "#;

        let template = parse_template("test", content).unwrap();

        assert_eq!(template.variables.len(), 2);
        assert_eq!(template.variables[0].name, "DB_HOST");
        assert_eq!(template.variables[1].name, "DB_PORT");
    }

    #[test]
    fn test_parse_empty_value() {
        let content = "EMPTY={{EMPTY|}}";
        let template = parse_template("test", content).unwrap();

        assert_eq!(template.variables.len(), 1);
        assert_eq!(template.variables[0].name, "EMPTY");
        assert_eq!(template.variables[0].default, Some("".to_string()));
    }

    #[test]
    fn test_parse_invalid_syntax() {
        // 测试 {{|default}} - 变量名为空
        let content = "DB={{|default}}";
        let result = parse_template("test", content);
        assert!(result.is_err());

        // 测试 {{   |default}} - 变量名只有空格
        let content2 = "DB={{   |default}}";
        let result2 = parse_template("test", content2);
        assert!(result2.is_err());
    }

    #[test]
    fn test_parse_duplicate_variables_same_line() {
        // 同一行重复变量，应该只保留一个
        let content = "DB={{DB}} and {{DB}}";
        let template = parse_template("test", content).unwrap();

        assert_eq!(template.variables.len(), 1);
        assert_eq!(template.variables[0].name, "DB");
    }

    #[test]
    fn test_parse_complex_template() {
        let content = r#"
# @inherits base.env
# @inherits db.env

# Web 配置
APP_ENV={{APP_ENV|development}}
API_URL={{API_URL}}

# 数据库（继承的模板可能已定义，这里覆盖）
DB_HOST={{DB_HOST|localhost}}

# 缓存配置
REDIS_URL={{REDIS_URL|redis://localhost:6379}}
        "#;

        let template = parse_template("web", content).unwrap();

        assert_eq!(template.name, "web");
        assert_eq!(template.inherits.len(), 2);
        assert_eq!(template.variables.len(), 4);

        // 验证变量
        let var_names: Vec<String> = template.variables.iter().map(|v| v.name.clone()).collect();
        assert!(var_names.contains(&"APP_ENV".to_string()));
        assert!(var_names.contains(&"API_URL".to_string()));
        assert!(var_names.contains(&"DB_HOST".to_string()));
        assert!(var_names.contains(&"REDIS_URL".to_string()));

        // 验证必需性
        let api_url = template
            .variables
            .iter()
            .find(|v| v.name == "API_URL")
            .unwrap();
        assert!(api_url.required);

        let app_env = template
            .variables
            .iter()
            .find(|v| v.name == "APP_ENV")
            .unwrap();
        assert!(!app_env.required);
        assert_eq!(app_env.default, Some("development".to_string()));
    }

    #[test]
    fn test_parse_whitespace_handling() {
        let content = "DB_HOST = {{ DB_HOST | localhost }}";
        let template = parse_template("test", content).unwrap();

        assert_eq!(template.variables.len(), 1);
        assert_eq!(template.variables[0].name, "DB_HOST");
        assert_eq!(template.variables[0].default, Some("localhost".to_string()));
    }
}
