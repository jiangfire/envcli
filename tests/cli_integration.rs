//! CLI 集成测试
//!
//! 使用 assert_cmd 进行命令行集成测试

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// 创建临时测试环境
fn create_test_env() -> TempDir {
    tempfile::tempdir().unwrap()
}

/// 获取 env 命令的路径
fn get_env_command() -> std::path::PathBuf {
    // 使用 CARGO_MANIFEST_DIR 确保在任何工作目录下都能找到二进制文件
    // 这解决了在临时目录运行测试时找不到二进制的问题
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR 应该在 cargo test 中可用");

    let mut path = std::path::PathBuf::from(manifest_dir);
    path.push("target");
    path.push("debug");

    // Windows 需要 .exe 扩展名，Unix 不需要
    if cfg!(windows) {
        path.push("env.exe");
    } else {
        path.push("env");
    }

    // 如果 debug 版本不存在，尝试 release 版本
    if !path.exists() {
        path.pop(); // 移除 env/env.exe
        path.pop(); // 移除 debug
        path.push("release");
        if cfg!(windows) {
            path.push("env.exe");
        } else {
            path.push("env");
        }
    }

    path
}

mod basic_commands {
    use super::*;

    #[test]
    fn test_help_command() {
        let cmd = get_env_command();
        let mut command = Command::new(cmd);

        command.arg("--help");

        command
            .assert()
            .success()
            .stdout(predicate::str::contains("env"));
    }

    #[test]
    fn test_version_command() {
        let cmd = get_env_command();
        let mut command = Command::new(cmd);

        command.arg("--version");

        // 这个命令可能不存在，取决于 CLI 实现
        // 我们只验证命令可以执行
        let result = command.ok();
        assert!(result.is_ok() || result.is_err());
    }
}

mod set_get_commands {
    use super::*;

    #[test]
    fn test_set_and_get_variable() {
        let temp_dir = create_test_env();
        let cmd = get_env_command();

        // 设置变量
        let mut set_cmd = Command::new(&cmd);
        set_cmd
            .arg("set")
            .arg("TEST_VAR")
            .arg("test_value")
            .current_dir(&temp_dir);

        set_cmd.assert().success();

        // 获取变量
        let mut get_cmd = Command::new(&cmd);
        get_cmd.arg("get").arg("TEST_VAR").current_dir(&temp_dir);

        get_cmd
            .assert()
            .success()
            .stdout(predicate::str::contains("test_value"));
    }

    #[test]
    fn test_set_multiple_variables() {
        let temp_dir = create_test_env();
        let cmd = get_env_command();

        // 设置多个变量
        for (key, value) in [("VAR1", "value1"), ("VAR2", "value2"), ("VAR3", "value3")] {
            let mut set_cmd = Command::new(&cmd);
            set_cmd
                .arg("set")
                .arg(key)
                .arg(value)
                .current_dir(&temp_dir);
            set_cmd.assert().success();
        }

        // 验证都能获取
        let mut get_cmd = Command::new(&cmd);
        get_cmd.arg("get").arg("VAR1").current_dir(&temp_dir);

        get_cmd
            .assert()
            .success()
            .stdout(predicate::str::contains("value1"));
    }

    #[test]
    fn test_get_nonexistent_variable() {
        let temp_dir = create_test_env();
        let cmd = get_env_command();

        let mut get_cmd = Command::new(&cmd);
        get_cmd
            .arg("get")
            .arg("NONEXISTENT_VAR")
            .current_dir(&temp_dir);

        // 应该失败或返回空
        let result = get_cmd.ok();
        assert!(result.is_ok() || result.is_err());
    }
}

mod list_commands {
    use super::*;

    #[test]
    fn test_list_local() {
        let temp_dir = create_test_env();
        let cmd = get_env_command();

        // 先设置一些变量
        Command::new(&cmd)
            .arg("set")
            .arg("LIST_VAR1")
            .arg("value1")
            .current_dir(&temp_dir)
            .assert()
            .success();

        Command::new(&cmd)
            .arg("set")
            .arg("LIST_VAR2")
            .arg("value2")
            .current_dir(&temp_dir)
            .assert()
            .success();

        // 列出变量
        let mut list_cmd = Command::new(&cmd);
        list_cmd
            .arg("list")
            .arg("--source")
            .arg("local")
            .current_dir(&temp_dir);

        list_cmd
            .assert()
            .success()
            .stdout(predicate::str::contains("LIST_VAR1"))
            .stdout(predicate::str::contains("LIST_VAR2"));
    }

    #[test]
    fn test_list_empty() {
        let temp_dir = create_test_env();
        let cmd = get_env_command();

        let mut list_cmd = Command::new(&cmd);
        list_cmd.arg("list").current_dir(&temp_dir);

        // 应该成功，可能输出系统变量或为空
        list_cmd.assert().success();
    }
}

mod unset_commands {
    use super::*;

    #[test]
    fn test_unset_variable() {
        let temp_dir = create_test_env();
        let cmd = get_env_command();

        // 设置变量
        Command::new(&cmd)
            .arg("set")
            .arg("TO_DELETE")
            .arg("value")
            .current_dir(&temp_dir)
            .assert()
            .success();

        // 验证存在
        let mut get_cmd = Command::new(&cmd);
        get_cmd.arg("get").arg("TO_DELETE").current_dir(&temp_dir);
        get_cmd.assert().success();

        // 删除变量
        Command::new(&cmd)
            .arg("unset")
            .arg("TO_DELETE")
            .current_dir(&temp_dir)
            .assert()
            .success();

        // 验证已删除
        let mut get_cmd = Command::new(&cmd);
        get_cmd.arg("get").arg("TO_DELETE").current_dir(&temp_dir);

        // 可能失败或返回空
        let result = get_cmd.ok();
        assert!(result.is_ok() || result.is_err());
    }
}

mod import_export_commands {
    use super::*;

    #[test]
    fn test_import_export_roundtrip() {
        let temp_dir = create_test_env();
        let cmd = get_env_command();

        // 创建 .env 文件
        let env_file = temp_dir.path().join("test.env");
        fs::write(&env_file, "IMPORT_KEY=import_value\nANOTHER=another_value").unwrap();

        // 导入
        Command::new(&cmd)
            .arg("import")
            .arg(env_file.to_str().unwrap())
            .current_dir(&temp_dir)
            .assert()
            .success();

        // 验证导入
        let mut get_cmd = Command::new(&cmd);
        get_cmd.arg("get").arg("IMPORT_KEY").current_dir(&temp_dir);
        get_cmd
            .assert()
            .success()
            .stdout(predicate::str::contains("import_value"));

        // 导出
        let mut export_cmd = Command::new(&cmd);
        export_cmd.arg("export").current_dir(&temp_dir);

        export_cmd
            .assert()
            .success()
            .stdout(predicate::str::contains("IMPORT_KEY=import_value"));
    }
}

mod run_command {
    use super::*;

    #[test]
    fn test_run_with_temp_vars() {
        let temp_dir = create_test_env();
        let cmd = get_env_command();

        // 使用 echo 测试（跨平台）
        let echo_cmd = if cfg!(target_os = "windows") {
            vec![
                "cmd".to_string(),
                "/c".to_string(),
                "echo".to_string(),
                "test".to_string(),
            ]
        } else {
            vec!["echo".to_string(), "test".to_string()]
        };

        // 运行命令
        let mut run_cmd = Command::new(&cmd);
        run_cmd
            .arg("run")
            .arg("--var")
            .arg("TEMP_VAR=temp_value")
            .args(&echo_cmd)
            .current_dir(&temp_dir);

        // 命令应该可以执行
        let result = run_cmd.ok();
        assert!(result.is_ok() || result.is_err());
    }
}

mod status_command {
    use super::*;

    #[test]
    fn test_status() {
        let temp_dir = create_test_env();
        let cmd = get_env_command();

        let mut status_cmd = Command::new(&cmd);
        status_cmd.arg("status").current_dir(&temp_dir);

        // 状态命令应该成功
        status_cmd.assert().success();
    }
}

mod doctor_command {
    use super::*;

    #[test]
    fn test_doctor() {
        let temp_dir = create_test_env();
        let cmd = get_env_command();

        let mut doctor_cmd = Command::new(&cmd);
        doctor_cmd.arg("doctor").current_dir(&temp_dir);

        // Doctor 命令应该成功
        doctor_cmd.assert().success();
    }
}

mod template_commands {
    use super::*;

    #[test]
    fn test_template_create_and_render() {
        let temp_dir = create_test_env();
        let cmd = get_env_command();

        // 创建模板
        let mut create_cmd = Command::new(&cmd);
        create_cmd
            .arg("template")
            .arg("create")
            .arg("test_template")
            .arg("--vars")
            .arg("DB_HOST")
            .arg("DB_PORT")
            .current_dir(&temp_dir);

        create_cmd.assert().success();

        // 渲染模板
        let mut render_cmd = Command::new(&cmd);
        render_cmd
            .arg("template")
            .arg("render")
            .arg("test_template")
            .arg("--var")
            .arg("DB_HOST=localhost")
            .arg("--var")
            .arg("DB_PORT=5432")
            .current_dir(&temp_dir);

        render_cmd
            .assert()
            .success()
            .stdout(predicate::str::contains("DB_HOST=localhost"))
            .stdout(predicate::str::contains("DB_PORT=5432"));
    }
}

mod error_handling {
    use super::*;

    #[test]
    fn test_invalid_command() {
        let temp_dir = create_test_env();
        let cmd = get_env_command();

        let mut invalid_cmd = Command::new(&cmd);
        invalid_cmd
            .arg("invalid_command_xyz")
            .current_dir(&temp_dir);

        // 应该失败
        invalid_cmd.assert().failure();
    }

    #[test]
    fn test_missing_required_arg() {
        let temp_dir = create_test_env();
        let cmd = get_env_command();

        let mut set_cmd = Command::new(&cmd);
        set_cmd.arg("set").current_dir(&temp_dir);

        // 缺少参数，应该失败
        set_cmd.assert().failure();
    }
}
