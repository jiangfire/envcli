//! 环境变量合并器 (实现 12-factor 优先级规则)
//!
//! 优先级（从低到高）：
//! 1. 系统环境变量
//! 2. 用户级配置
//! 3. 项目级配置
//! 4. 本地级配置
//! 5. 临时变量（最高）

use crate::core::Store;
use crate::error::{EnvError, Result};
use crate::types::EnvSource;
use std::collections::HashMap;

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
                    // trim 键，trim 值的左边空格但保留右边空格
                    let trimmed_key = key.trim().to_string();
                    let trimmed_value = value.trim_start().to_string();
                    vars.push((trimmed_key, trimmed_value));
                }
                _ => {
                    return Err(EnvError::EnvParseError(format!(
                        "无效的环境变量格式 '{}'，应为 KEY=VALUE",
                        arg
                    )));
                }
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

        let content = std::fs::read_to_string(path).map_err(EnvError::Io)?;

        // 使用现有的 .env 解析器
        let vars =
            crate::config::format::dotenv::DotenvParser::parse(&content, &EnvSource::System)?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Config;
    use serial_test::serial;
    use std::fs;
    use tempfile::TempDir;

    // 辅助函数：创建临时测试环境
    fn create_test_store() -> (TempDir, Store) {
        let temp_dir = tempfile::tempdir().unwrap();

        // 创建配置
        let config = Config { verbose: false };
        let store = Store::new(config);

        (temp_dir, store)
    }

    /// 在临时目录中执行测试操作
    fn with_temp_store<F, R>(f: F) -> R
    where
        F: FnOnce(&TempDir, &Store) -> R,
    {
        let temp_dir = tempfile::tempdir().unwrap();
        let original_dir = std::env::current_dir().unwrap();

        // 切换到临时目录
        std::env::set_current_dir(&temp_dir).unwrap();

        let config = Config { verbose: false };
        let store = Store::new(config);

        let result = f(&temp_dir, &store);

        // 恢复原目录
        std::env::set_current_dir(original_dir).unwrap();

        result
    }

    mod parse_temp_vars_tests {
        use super::*;

        #[test]
        fn test_parse_temp_vars_single_var() {
            let args = vec!["KEY=VALUE".to_string()];
            let result = EnvMerger::parse_temp_vars(&args).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0], ("KEY".to_string(), "VALUE".to_string()));
        }

        #[test]
        fn test_parse_temp_vars_multiple_vars() {
            let args = vec![
                "DB_HOST=localhost".to_string(),
                "DB_PORT=5432".to_string(),
                "APP_ENV=development".to_string(),
            ];

            let result = EnvMerger::parse_temp_vars(&args).unwrap();

            assert_eq!(result.len(), 3);
            assert_eq!(result[0], ("DB_HOST".to_string(), "localhost".to_string()));
            assert_eq!(result[1], ("DB_PORT".to_string(), "5432".to_string()));
            assert_eq!(
                result[2],
                ("APP_ENV".to_string(), "development".to_string())
            );
        }

        #[test]
        fn test_parse_temp_vars_with_spaces() {
            let args = vec!["KEY = VALUE".to_string(), "KEY2=  VALUE2  ".to_string()];

            let result = EnvMerger::parse_temp_vars(&args).unwrap();

            assert_eq!(result.len(), 2);
            assert_eq!(result[0], ("KEY".to_string(), "VALUE".to_string()));
            assert_eq!(result[1], ("KEY2".to_string(), "VALUE2  ".to_string()));
        }

        #[test]
        fn test_parse_temp_vars_empty_key_error() {
            let args = vec!["=VALUE".to_string()];

            let result = EnvMerger::parse_temp_vars(&args);
            assert!(result.is_err());
        }

        #[test]
        fn test_parse_temp_vars_missing_equals() {
            let args = vec!["INVALID".to_string()];

            let result = EnvMerger::parse_temp_vars(&args);
            assert!(result.is_err());
        }

        #[test]
        fn test_parse_temp_vars_empty_value() {
            let args = vec!["KEY=".to_string()];

            let result = EnvMerger::parse_temp_vars(&args).unwrap();

            assert_eq!(result.len(), 1);
            assert_eq!(result[0], ("KEY".to_string(), "".to_string()));
        }

        #[test]
        fn test_parse_temp_vars_empty_array() {
            let args = vec![];

            let result = EnvMerger::parse_temp_vars(&args).unwrap();
            assert!(result.is_empty());
        }

        #[test]
        fn test_parse_temp_vars_special_chars() {
            let args = vec![
                "PATH=/usr/bin:/usr/local/bin".to_string(),
                "SPECIAL=hello@world#test".to_string(),
            ];

            let result = EnvMerger::parse_temp_vars(&args).unwrap();

            assert_eq!(result.len(), 2);
            assert_eq!(
                result[0],
                ("PATH".to_string(), "/usr/bin:/usr/local/bin".to_string())
            );
            assert_eq!(
                result[1],
                ("SPECIAL".to_string(), "hello@world#test".to_string())
            );
        }
    }

    mod parse_file_tests {
        use super::*;

        #[test]
        fn test_parse_file_success() {
            let temp_dir = tempfile::tempdir().unwrap();
            let test_file = temp_dir.path().join(".env");

            let content = r#"
DB_HOST=localhost
DB_PORT=5432
APP_ENV=development
            "#;

            fs::write(&test_file, content).unwrap();

            let result = EnvMerger::parse_file(test_file.to_str().unwrap()).unwrap();

            assert_eq!(result.len(), 3);

            let keys: Vec<&String> = result.iter().map(|(k, _)| k).collect();
            assert!(keys.contains(&&"DB_HOST".to_string()));
            assert!(keys.contains(&&"DB_PORT".to_string()));
            assert!(keys.contains(&&"APP_ENV".to_string()));
        }

        #[test]
        fn test_parse_file_with_comments() {
            let temp_dir = tempfile::tempdir().unwrap();
            let test_file = temp_dir.path().join(".env");

            let content = r#"
# Database configuration
DB_HOST=localhost
# Port number
DB_PORT=5432
            "#;

            fs::write(&test_file, content).unwrap();

            let result = EnvMerger::parse_file(test_file.to_str().unwrap()).unwrap();

            // 注释行应该被忽略
            assert_eq!(result.len(), 2);
        }

        #[test]
        fn test_parse_file_empty_values() {
            let temp_dir = tempfile::tempdir().unwrap();
            let test_file = temp_dir.path().join(".env");

            let content = "EMPTY=\nKEY=VALUE";

            fs::write(&test_file, content).unwrap();

            let result = EnvMerger::parse_file(test_file.to_str().unwrap()).unwrap();

            assert_eq!(result.len(), 2);

            let empty = result.iter().find(|(k, _)| k == "EMPTY").unwrap();
            assert_eq!(empty.1, "");
        }

        #[test]
        fn test_parse_file_not_found() {
            let result = EnvMerger::parse_file("/nonexistent/file.env");
            assert!(result.is_err());
        }

        #[test]
        fn test_parse_file_invalid_format() {
            let temp_dir = tempfile::tempdir().unwrap();
            let test_file = temp_dir.path().join(".env");

            // 包含无效行（没有 = 号）
            let content = "INVALID_LINE\nKEY=VALUE";

            fs::write(&test_file, content).unwrap();

            // 应该忽略无效行
            let result = EnvMerger::parse_file(test_file.to_str().unwrap()).unwrap();
            assert_eq!(result.len(), 1);
        }

        #[test]
        fn test_parse_file_multiline_values() {
            let temp_dir = tempfile::tempdir().unwrap();
            let test_file = temp_dir.path().join(".env");

            let content = "MULTI=line1\\\nline2\\\nline3\nKEY=VALUE";

            fs::write(&test_file, content).unwrap();

            let result = EnvMerger::parse_file(test_file.to_str().unwrap()).unwrap();

            // 应该正确处理多行值
            assert!(!result.is_empty());
        }
    }

    #[serial]
    mod merge_environment_tests {
        use super::*;

        #[test]
        fn test_merge_environment_temp_vars_only() {
            let (_temp_dir, store) = create_test_store();

            let temp_vars = vec![
                ("TEMP_KEY".to_string(), "temp_value".to_string()),
                ("ANOTHER".to_string(), "another_value".to_string()),
            ];

            let result = EnvMerger::merge_environment(&store, &temp_vars).unwrap();

            assert_eq!(result.get("TEMP_KEY"), Some(&"temp_value".to_string()));
            assert_eq!(result.get("ANOTHER"), Some(&"another_value".to_string()));
        }

        #[test]
        fn test_merge_environment_with_system_vars() {
            with_temp_store(|_temp_dir, store| {
                // 设置一个测试系统变量
                unsafe {
                    std::env::set_var("TEST_SYSTEM_VAR_UNIQUE_999", "system_value");
                }

                let temp_vars = vec![];

                let result = EnvMerger::merge_environment(store, &temp_vars).unwrap();

                // 应该包含系统变量
                assert_eq!(
                    result.get("TEST_SYSTEM_VAR_UNIQUE_999"),
                    Some(&"system_value".to_string())
                );

                // 清理
                unsafe {
                    std::env::remove_var("TEST_SYSTEM_VAR_UNIQUE_999");
                }
            });
        }

        #[test]
        fn test_merge_environment_priority() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            // 创建配置
            let config = Config { verbose: false };
            let store = Store::new(config);

            // 设置系统变量
            unsafe {
                std::env::set_var("TEST_VAR", "system_value");
            }

            // 设置本地变量（通过 store）
            store
                .set("TEST_VAR".to_string(), "local_value".to_string())
                .unwrap();

            // 临时变量
            let temp_vars = vec![
                ("TEST_VAR".to_string(), "temp_value".to_string()),
                ("TEMP_ONLY".to_string(), "temp_only".to_string()),
            ];

            let result = EnvMerger::merge_environment(&store, &temp_vars).unwrap();

            // 临时变量应该覆盖所有
            assert_eq!(result.get("TEST_VAR"), Some(&"temp_value".to_string()));
            assert_eq!(result.get("TEMP_ONLY"), Some(&"temp_only".to_string()));

            // 清理
            unsafe {
                std::env::remove_var("TEST_VAR");
            }
            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_merge_environment_empty_temp_vars() {
            let (_temp_dir, store) = create_test_store();

            let temp_vars = vec![];

            let result = EnvMerger::merge_environment(&store, &temp_vars).unwrap();

            // 应该至少包含系统变量
            assert!(!result.is_empty());
        }

        #[test]
        fn test_merge_environment_with_project_vars() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            // 创建项目级变量
            let project_dir = temp_dir.path().join(".envcli");
            fs::create_dir_all(&project_dir).unwrap();
            let project_file = project_dir.join("project.env");
            fs::write(&project_file, "PROJECT_VAR=project_value").unwrap();

            let temp_vars = vec![];

            let result = EnvMerger::merge_environment(&store, &temp_vars).unwrap();

            assert_eq!(
                result.get("PROJECT_VAR"),
                Some(&"project_value".to_string())
            );

            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_merge_environment_variable_override() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            // 设置相同变量在不同层级
            unsafe {
                std::env::set_var("OVERRIDE_VAR", "system");
            }

            // 创建用户级
            let home_dir = dirs::home_dir().unwrap();
            let user_dir = home_dir.join(".envcli");
            fs::create_dir_all(&user_dir).unwrap();
            let user_file = user_dir.join("user.env");
            fs::write(&user_file, "OVERRIDE_VAR=user\nUSER_VAR=user_only").unwrap();

            // 创建项目级
            let project_dir = temp_dir.path().join(".envcli");
            fs::create_dir_all(&project_dir).unwrap();
            let project_file = project_dir.join("project.env");
            fs::write(
                &project_file,
                "OVERRIDE_VAR=project\nPROJECT_VAR=project_only",
            )
            .unwrap();

            // 创建本地级
            let local_file = project_dir.join("local.env");
            fs::write(&local_file, "OVERRIDE_VAR=local\nLOCAL_VAR=local_only").unwrap();

            let temp_vars = vec![];

            let result = EnvMerger::merge_environment(&store, &temp_vars).unwrap();

            // 应该是本地级的值（最高优先级）
            assert_eq!(result.get("OVERRIDE_VAR"), Some(&"local".to_string()));

            // 其他变量也应该存在
            assert_eq!(result.get("USER_VAR"), Some(&"user_only".to_string()));
            assert_eq!(result.get("PROJECT_VAR"), Some(&"project_only".to_string()));
            assert_eq!(result.get("LOCAL_VAR"), Some(&"local_only".to_string()));

            // 清理
            unsafe {
                std::env::remove_var("OVERRIDE_VAR");
            }
            std::env::set_current_dir(original_dir).unwrap();
        }

        #[test]
        fn test_merge_environment_temp_overrides_all() {
            let temp_dir = tempfile::tempdir().unwrap();
            let original_dir = std::env::current_dir().unwrap();

            std::env::set_current_dir(&temp_dir).unwrap();

            let config = Config { verbose: false };
            let store = Store::new(config);

            // 设置所有层级的变量
            unsafe {
                std::env::set_var("ALL_LEVELS", "system");
            }

            let home_dir = dirs::home_dir().unwrap();
            let user_dir = home_dir.join(".envcli");
            fs::create_dir_all(&user_dir).unwrap();
            let user_file = user_dir.join("user.env");
            fs::write(&user_file, "ALL_LEVELS=user").unwrap();

            let project_dir = temp_dir.path().join(".envcli");
            fs::create_dir_all(&project_dir).unwrap();
            let project_file = project_dir.join("project.env");
            fs::write(&project_file, "ALL_LEVELS=project").unwrap();

            let local_file = project_dir.join("local.env");
            fs::write(&local_file, "ALL_LEVELS=local").unwrap();

            // 临时变量应该覆盖所有
            let temp_vars = vec![("ALL_LEVELS".to_string(), "temp".to_string())];

            let result = EnvMerger::merge_environment(&store, &temp_vars).unwrap();

            assert_eq!(result.get("ALL_LEVELS"), Some(&"temp".to_string()));

            // 清理
            unsafe {
                std::env::remove_var("ALL_LEVELS");
            }
            std::env::set_current_dir(original_dir).unwrap();
        }
    }
}
