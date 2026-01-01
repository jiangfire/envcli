//! .env 格式解析器 (简单原则：透明的文本解析)

use crate::error::{EnvError, Result};
use crate::types::{EnvSource, EnvVar};

/// .env 格式解析器
pub struct DotenvParser;

impl DotenvParser {
    /// 解析 .env 文件内容
    ///
    /// 规则：
    /// - 忽略空行和以 # 开头的注释行
    /// - 格式：KEY=VALUE
    /// - VALUE 可以包含空格，无需引号
    /// - 支持多行值（以 \ 结尾）
    ///
    /// # Errors
    ///
    /// Returns parsing errors for invalid format.
    pub fn parse(content: &str, source: &EnvSource) -> Result<Vec<EnvVar>> {
        let mut vars = Vec::new();

        let lines: Vec<&str> = content.lines().collect();
        let mut line_num = 0;

        while line_num < lines.len() {
            let line = lines[line_num].trim();

            // 跳过空行和注释
            if line.is_empty() || line.starts_with('#') {
                line_num += 1;
                continue;
            }

            // 检查是否有多行值（以 \ 结尾）
            let mut value_end = line_num;
            let mut complete_line = line.to_string();

            while complete_line.ends_with('\\') && value_end + 1 < lines.len() {
                // 移除结尾的 \ 并拼接下一行
                complete_line.pop();
                value_end += 1;
                complete_line.push_str(lines[value_end].trim());
            }

            line_num = value_end + 1;

            // 解析 KEY=VALUE
            if let Some((key, value)) = complete_line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                if key.is_empty() {
                    return Err(EnvError::Parse(format!("空的键名在行 '{complete_line}'")));
                }

                vars.push(EnvVar::new(
                    key.to_string(),
                    value.to_string(),
                    source.clone(),
                ));
            } else {
                // 不是 KEY=VALUE 格式，忽略或根据严格性决定
                // 这里选择跳过，保持兼容性
            }
        }

        Ok(vars)
    }

    /// 序列化 `EnvVar` 列表为 .env 格式
    #[must_use]
    pub fn serialize(vars: &[EnvVar]) -> String {
        vars.iter()
            .map(|v| format!("{}={}", v.key, v.value))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// 这种简化的解析器处理常见场景
///
/// # 示例
/// ```ignore
/// # Database configuration
/// DB_HOST=localhost
/// DB_PORT=5432
/// DB_USER=admin
/// EMPTY_VALUE=
/// VALUE_WITH_SPACES = hello world
/// ```
impl Default for DotenvParser {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic() {
        let content = r"
# 注释会被忽略
KEY1=value1
KEY2=value2
        ";

        let result = DotenvParser::parse(content, &EnvSource::Local).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].key, "KEY1");
        assert_eq!(result[0].value, "value1");
    }

    #[test]
    fn test_parse_empty_value() {
        let content = "KEY=\nKEY2=value";
        let result = DotenvParser::parse(content, &EnvSource::Local).unwrap();
        assert_eq!(result[0].value, "");
    }
}
