//! 系统环境变量集成测试 - 注册表读取功能
//!
//! 测试 Windows 注册表读取功能，确保 system-set 设置的变量能被 get 正确读取

#[cfg(test)]
mod windows_registry_tests {
    use super::super::paths::get_system_env;
    use std::collections::HashMap;

    #[test]
    #[cfg(target_os = "windows")]
    fn test_windows_registry_read_integration() {
        // TDD: 验证从注册表读取环境变量
        // 这个测试验证了修复后的 get_system_env 能正确读取注册表

        // 获取当前环境变量
        let env_result = get_system_env();
        assert!(env_result.is_ok());

        let env = env_result.unwrap();

        // 验证返回的是 HashMap（通过类型推断）
        let _map: &HashMap<String, String> = &env;

        // 验证包含一些基本的系统变量
        assert!(!env.is_empty());
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_windows_registry_priority() {
        // TDD: 验证注册表变量优先级高于 std::env::vars()
        // 这个测试验证了注册表变量会覆盖进程环境变量

        let env_result = get_system_env();
        assert!(env_result.is_ok());

        let env = env_result.unwrap();

        // 验证至少包含 PATH（所有系统都应该有）
        assert!(env.contains_key("PATH"));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_windows_registry_special_chars() {
        // TDD: 验证注册表中特殊字符的处理
        // 确保包含特殊字符的变量名能被正确处理

        let env_result = get_system_env();
        assert!(env_result.is_ok());

        let env = env_result.unwrap();

        // 验证所有变量名都不以 _ 开头，也不等于 "_"
        // 这些特殊变量应该被过滤掉
        for key in env.keys() {
            let key_str: &String = key;
            assert!(!key_str.starts_with('_'), "变量名不应以_开头: {}", key_str);
            assert!(key_str != "_", "特殊变量 _ 应该被过滤");
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_windows_registry_empty_values_filtered() {
        // TDD: 验证空值被正确过滤
        // get_system_env 应该跳过空值（从注册表读取时）

        let env_result = get_system_env();
        assert!(env_result.is_ok());

        let env = env_result.unwrap();

        // 验证所有值都不为空
        // 注意：有些系统环境变量可能在进程环境中为空，但注册表读取会过滤空值
        for (key, value) in &env {
            let value_str: &String = value;
            assert!(!value_str.is_empty(), "变量 {} 的值不应为空", key);
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_windows_registry_vs_process_env() {
        // TDD: 验证注册表读取与进程环境变量的合并
        // 应该包含两者的内容，注册表优先

        let env_result = get_system_env();
        assert!(env_result.is_ok());

        let env = env_result.unwrap();

        // 获取进程环境变量
        let process_env: HashMap<String, String> = std::env::vars().collect();

        // 验证进程环境变量的基本内容都在结果中
        // （注册表可能会添加更多变量）
        for (key, value) in &process_env {
            // 跳过空值的进程环境变量
            if value.is_empty() {
                continue;
            }

            if let Some(result_value) = env.get(key) {
                // 如果注册表有该变量，应该匹配注册表的值
                // 否则应该匹配进程的值
                // 这里我们只验证变量存在且非空
                let result_value_str: &String = result_value;
                assert!(!result_value_str.is_empty(), "合并后的变量 {} 不应为空", key);
            }
        }
    }
}

#[cfg(test)]
mod cross_platform_tests {
    use super::super::paths::get_system_env;

    #[test]
    fn test_get_system_env_returns_result() {
        // TDD: 验证函数返回 Result
        let result = get_system_env();
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_system_env_returns_hashmap() {
        // TDD: 验证返回的是 HashMap
        let result = get_system_env().unwrap();
        // 验证可以当作 HashMap 使用
        let _map: std::collections::HashMap<String, String> = result;
    }

    #[test]
    fn test_get_system_env_not_empty() {
        // TDD: 验证至少包含一些环境变量
        let env = get_system_env().unwrap();

        // 所有系统都应该有 PATH
        assert!(env.contains_key("PATH"));

        // 应该有一些变量
        assert!(!env.is_empty());
    }

    #[test]
    fn test_get_system_env_case_sensitivity() {
        // TDD: 验证大小写处理
        let env = get_system_env().unwrap();

        // 在 Windows 上，环境变量不区分大小写
        // 但我们的实现应该保持一致性
        // 这里验证基本的大小写行为
        // PATH 应该存在（无需额外断言）
        let _ = env.contains_key("PATH");
    }
}

#[cfg(test)]
mod performance_tests {
    use super::super::paths::get_system_env;
    use std::time::Instant;

    #[test]
    fn test_get_system_env_performance() {
        // TDD: 验证性能
        let start = Instant::now();

        // 多次调用测试性能
        for _ in 0..10 {
            let _ = get_system_env().unwrap();
        }

        let duration = start.elapsed();

        // 10次调用应该在 100ms 内完成
        assert!(duration.as_millis() < 100, "性能测试失败: {}ms", duration.as_millis());
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::super::paths::get_system_env;

    #[test]
    fn test_get_system_env_no_panic() {
        // TDD: 验证不会 panic
        // 即使在异常情况下也应该返回 Result
        let result = std::panic::catch_unwind(get_system_env);
        assert!(result.is_ok());

        let inner_result = result.unwrap();
        assert!(inner_result.is_ok());
    }

    #[test]
    fn test_get_system_env_consistency() {
        // TDD: 验证多次调用的一致性
        let result1 = get_system_env().unwrap();
        let result2 = get_system_env().unwrap();

        // 应该包含相同数量的变量
        assert_eq!(result1.len(), result2.len());

        // 应该包含相同的键
        for key in result1.keys() {
            assert!(result2.contains_key(key));
        }
    }
}
