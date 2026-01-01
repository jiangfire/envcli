//! 加密的 .env 格式解析器
//!
//! 支持解析和序列化包含加密变量的 .env 文件格式
//! 加密变量格式：KEY=ENC[SOPS:v1:...]

use crate::error::{EnvError, Result};
use crate::types::{EncryptedEnvVar, EncryptionType, EnvSource};
use crate::utils::encryption::SopsEncryptor;

/// 加密 .env 格式解析器
pub struct EncryptedDotenvParser;

impl EncryptedDotenvParser {
    /// 解析加密的 .env 文件内容
    ///
    /// 支持：
    /// - 明文变量：KEY=value
    /// - 加密变量：KEY=ENC[SOPS:v1:...]
    /// - 注释：# comment
    /// - 空行
    ///
    /// # Errors
    ///
    /// Returns parsing errors for invalid format.
    pub fn parse(content: &str, source: &EnvSource) -> Result<Vec<EncryptedEnvVar>> {
        let mut vars = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();

            // 跳过空行和注释
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // 解析 KEY=VALUE
            if let Some((key, value)) = trimmed.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                if key.is_empty() {
                    return Err(EnvError::Parse(format!(
                        "空的键名在第 {} 行: '{}'",
                        line_num + 1,
                        trimmed
                    )));
                }

                // 检测加密类型
                let encryption_type = if SopsEncryptor::is_encrypted(value) {
                    EncryptionType::Sops
                } else {
                    EncryptionType::None
                };

                vars.push(EncryptedEnvVar::new(
                    key.to_string(),
                    value.to_string(),
                    source.clone(),
                    encryption_type,
                ));
            } else {
                // 不是 KEY=VALUE 格式，跳过
            }
        }

        Ok(vars)
    }

    /// 序列化 `EncryptedEnvVar` 列表为 .env 格式
    ///
    /// 保持加密状态不变
    #[must_use]
    pub fn serialize(vars: &[EncryptedEnvVar]) -> String {
        vars.iter()
            .map(|v| format!("{}={}", v.key, v.value))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 加密明文变量并序列化
    ///
    /// 将所有标记为 Sops 的明文变量加密
    ///
    /// # Errors
    ///
    /// Returns encryption errors if encryption fails.
    pub fn serialize_and_encrypt(vars: &[EncryptedEnvVar]) -> Result<String> {
        let encryptor = SopsEncryptor::new();
        let mut lines = Vec::new();

        for var in vars {
            if var.encryption_type == EncryptionType::Sops && !var.is_encrypted() {
                // 需要加密
                let encrypted = encryptor.encrypt(&var.value)?;
                lines.push(format!("{}={}", var.key, encrypted));
            } else {
                // 已加密或不需要加密
                lines.push(format!("{}={}", var.key, var.value));
            }
        }

        Ok(lines.join("\n"))
    }

    /// 检测内容是否包含加密变量
    #[must_use]
    pub fn has_encrypted(content: &str) -> bool {
        content.lines().any(|line| {
            line.trim()
                .split_once('=')
                .is_some_and(|(_, value)| SopsEncryptor::is_encrypted(value.trim()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_mixed_content() {
        let content = r"
# 数据库配置
DB_HOST=localhost
DB_PASS=ENC[SOPS:v1:abc123]
# API 密钥
API_KEY=secret_key
        ";

        let vars = EncryptedDotenvParser::parse(content, &EnvSource::Local).unwrap();

        assert_eq!(vars.len(), 3);

        // 检查第一个变量（明文）
        assert_eq!(vars[0].key, "DB_HOST");
        assert_eq!(vars[0].value, "localhost");
        assert_eq!(vars[0].encryption_type, EncryptionType::None);
        assert!(!vars[0].is_encrypted());

        // 检查第二个变量（加密）
        assert_eq!(vars[1].key, "DB_PASS");
        assert!(vars[1].value.starts_with("ENC[SOPS:"));
        assert_eq!(vars[1].encryption_type, EncryptionType::Sops);
        assert!(vars[1].is_encrypted());

        // 检查第三个变量（明文）
        assert_eq!(vars[2].key, "API_KEY");
        assert_eq!(vars[2].value, "secret_key");
        assert_eq!(vars[2].encryption_type, EncryptionType::None);
    }

    #[test]
    fn test_serialize() {
        let vars = vec![
            EncryptedEnvVar::new(
                "DB_HOST".to_string(),
                "localhost".to_string(),
                EnvSource::Local,
                EncryptionType::None,
            ),
            EncryptedEnvVar::new(
                "DB_PASS".to_string(),
                "ENC[SOPS:v1:abc]".to_string(),
                EnvSource::Local,
                EncryptionType::Sops,
            ),
        ];

        let serialized = EncryptedDotenvParser::serialize(&vars);
        let lines: Vec<&str> = serialized.lines().collect();

        assert_eq!(lines.len(), 2);
        assert!(lines.contains(&"DB_HOST=localhost"));
        assert!(lines.contains(&"DB_PASS=ENC[SOPS:v1:abc]"));
    }

    #[test]
    fn test_has_encrypted() {
        let content1 = "DB_HOST=localhost\nDB_PASS=secret";
        assert!(!EncryptedDotenvParser::has_encrypted(content1));

        let content2 = "DB_HOST=localhost\nDB_PASS=ENC[SOPS:v1:abc]";
        assert!(EncryptedDotenvParser::has_encrypted(content2));
    }

    #[test]
    fn test_parse_empty_value() {
        let content = "EMPTY=\nKEY=value";
        let vars = EncryptedDotenvParser::parse(content, &EnvSource::Local).unwrap();

        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0].value, "");
        assert_eq!(vars[1].value, "value");
    }

    #[test]
    fn test_parse_invalid_format() {
        let content = "INVALID_LINE\nKEY=value";
        let vars = EncryptedDotenvParser::parse(content, &EnvSource::Local).unwrap();

        // 应该跳过无效行
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].key, "KEY");
    }
}
