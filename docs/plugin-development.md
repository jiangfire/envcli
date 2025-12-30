# EnvCLI 插件开发教程

**从零开始开发你的第一个 EnvCLI 插件**

---

## 📖 目录

1. [插件架构概述](#插件架构概述)
2. [开发环境准备](#开发环境准备)
3. [创建第一个插件](#创建第一个插件)
4. [钩子系统详解](#钩子系统详解)
5. [插件配置管理](#插件配置管理)
6. [高级功能](#高级功能)
7. [测试与调试](#测试与调试)
8. [发布插件](#发布插件)

---

## 插件架构概述

### 插件类型

EnvCLI 支持多种插件类型：

| 类型 | 说明 | 适用场景 |
|------|------|----------|
| **Dynamic** | Rust 动态库 (.so/.dll) | 高性能、深度集成 |
| **Executable** | 可执行文件 | 任何语言、易于开发 |
| **Script** | Shell/Python 脚本 | 快速原型、简单逻辑 |

### 钩子类型

插件可以通过钩子响应特定事件：

| 钩子 | 触发时机 | 典型用途 |
|------|----------|----------|
| **PreCommand** | 命令执行前 | 日志、验证、环境准备 |
| **PostCommand** | 命令执行后 | 清理、通知、结果处理 |
| **Error** | 发生错误时 | 错误报告、恢复 |
| **PreSet** | 设置变量前 | 数据验证、转换 |
| **PostGet** | 获取变量后 | 数据解密、转换 |

---

## 开发环境准备

### 1. Rust 插件（动态库）

#### 依赖配置

在 `Cargo.toml` 中添加：

```toml
[package]
name = "my-envcli-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]  # 动态库

[dependencies]
envcli = { version = "0.1.0", features = ["plugin-sdk"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

#### 插件入口

```rust
// src/lib.rs
use envcli::plugin::{
    Plugin, PluginMetadata, PluginInfo, PluginType,
    HookType, HookContext, HookResult,
    create_plugin_entry
};

// 1. 定义插件结构体
#[derive(Debug, Clone)]
pub struct MyPlugin {
    metadata: PluginMetadata,
}

// 2. 实现 Plugin trait
impl Plugin for MyPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    // 处理钩子
    fn execute_hook(&self, hook_type: HookType, context: &HookContext) -> Result<HookResult, String> {
        match hook_type {
            HookType::PreCommand => self.handle_pre_command(context),
            HookType::PostCommand => self.handle_post_command(context),
            HookType::Error => self.handle_error(context),
            _ => Ok(HookResult::default()),
        }
    }

    // 插件初始化
    fn initialize(&self) -> Result<(), String> {
        println!("MyPlugin initialized");
        Ok(())
    }

    // 插件清理
    fn shutdown(&self) -> Result<(), String> {
        println!("MyPlugin shutdown");
        Ok(())
    }
}

// 3. 实现具体钩子处理
impl MyPlugin {
    fn handle_pre_command(&self, context: &HookContext) -> Result<HookResult, String> {
        println!("执行命令前: {}", context.command_name);

        // 可以修改环境变量
        let mut result = HookResult::default();
        result.modified_env.insert(
            "PLUGIN_PRE_COMMAND".to_string(),
            "true".to_string()
        );

        Ok(result)
    }

    fn handle_post_command(&self, context: &HookContext) -> Result<HookResult, String> {
        println!("执行命令后: {}", context.command_name);
        Ok(HookResult::default())
    }

    fn handle_error(&self, context: &HookContext) -> Result<HookResult, String> {
        if let Some(error) = &context.error {
            eprintln!("插件捕获错误: {}", error);
        }
        Ok(HookResult::default())
    }
}

// 4. 创建插件入口点
create_plugin_entry!(MyPlugin, || {
    MyPlugin {
        metadata: PluginMetadata {
            id: "my-plugin".to_string(),
            name: "My First Plugin".to_string(),
            version: "0.1.0".to_string(),
            author: "Your Name".to_string(),
            description: "一个示例插件".to_string(),
            plugin_type: PluginType::Dynamic,
            enabled: true,
        },
    }
});
```

---

### 2. 可执行插件

#### 创建可执行文件

```bash
#!/bin/bash
# my-plugin.sh

# 读取 JSON 输入
INPUT=$(cat)

# 解析命令
COMMAND=$(echo "$INPUT" | jq -r '.command_name')
HOOK_TYPE=$(echo "$INPUT" | jq -r '.hook_type')

# 处理钩子
case "$HOOK_TYPE" in
    "PreCommand")
        echo "执行命令前: $COMMAND" >&2

        # 输出结果（JSON 格式）
        cat <<EOF
{
  "modified_env": {
    "PLUGIN_PRE_COMMAND": "true"
  },
  "blocked": false
}
EOF
        ;;

    "PostCommand")
        echo "执行命令后: $COMMAND" >&2
        cat <<EOF
{
  "modified_env": {},
  "blocked": false
}
EOF
        ;;

    "Error")
        ERROR=$(echo "$INPUT" | jq -r '.error // empty')
        echo "错误发生: $ERROR" >&2
        cat <<EOF
{
  "modified_env": {},
  "blocked": false
}
EOF
        ;;

    *)
        echo "未知钩子类型: $HOOK_TYPE" >&2
        cat <<EOF
{
  "modified_env": {},
  "blocked": false
}
EOF
        ;;
esac
```

#### 使脚本可执行

```bash
chmod +x my-plugin.sh
```

---

### 3. Python 插件

```python
#!/usr/bin/env python3
# my_plugin.py

import json
import sys

def main():
    # 读取输入
    input_data = sys.stdin.read()
    context = json.loads(input_data)

    command = context.get('command_name', '')
    hook_type = context.get('hook_type', '')

    # 处理钩子
    if hook_type == 'PreCommand':
        print(f"执行命令前: {command}", file=sys.stderr)

        result = {
            "modified_env": {
                "PLUGIN_PRE_COMMAND": "true"
            },
            "blocked": False
        }

    elif hook_type == 'PostCommand':
        print(f"执行命令后: {command}", file=sys.stderr)
        result = {"modified_env": {}, "blocked": False}

    elif hook_type == 'Error':
        error = context.get('error', '')
        print(f"错误发生: {error}", file=sys.stderr)
        result = {"modified_env": {}, "blocked": False}

    else:
        result = {"modified_env": {}, "blocked": False}

    # 输出 JSON
    print(json.dumps(result))

if __name__ == '__main__':
    main()
```

---

## 钩子系统详解

### HookContext 结构

```rust
pub struct HookContext {
    pub command_name: String,      // 命令名称
    pub timestamp: DateTime<Utc>,  // 时间戳
    pub env_vars: HashMap<String, String>,  // 当前环境变量
    pub error: Option<String>,     // 错误信息（仅 Error 钩子）
}
```

### HookResult 结构

```rust
pub struct HookResult {
    pub modified_env: HashMap<String, String>,  // 修改的环境变量
    pub blocked: bool,                          // 是否阻止命令执行
}
```

### 钩子执行流程

```
用户执行命令
    ↓
[PreCommand 钩子] ← 插件可以修改环境、阻止执行
    ↓
执行核心命令
    ↓
[PostCommand 钩子] ← 插件可以处理结果
    ↓
如果发生错误
    ↓
[Error 钩子] ← 插件可以处理错误
```

---

## 插件配置管理

### 1. 定义配置结构

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PluginConfig {
    #[serde(default = "default_timeout")]
    pub timeout: u64,

    #[serde(default = "default_webhook")]
    pub webhook_url: String,

    #[serde(default)]
    pub enabled_hooks: Vec<String>,
}

fn default_timeout() -> u64 {
    60
}

fn default_webhook() -> String {
    "https://example.com/webhook".to_string()
}

impl Default for PluginConfig {
    fn default() -> Self {
        PluginConfig {
            timeout: default_timeout(),
            webhook_url: default_webhook(),
            enabled_hooks: vec![],
        }
    }
}
```

### 2. 在插件中使用配置

```rust
use envcli::plugin::{PluginConfigManager, PluginConfig};

pub struct ConfigurablePlugin {
    metadata: PluginMetadata,
    config_manager: PluginConfigManager,
}

impl Plugin for ConfigurablePlugin {
    // ... 其他方法

    fn execute_hook(&self, hook_type: HookType, context: &HookContext) -> Result<HookResult, String> {
        // 获取配置
        let config: PluginConfig = self.config_manager.get_config()?;

        // 使用配置
        if config.timeout > 0 {
            println!("超时设置: {}s", config.timeout);
        }

        if !config.webhook_url.is_empty() {
            println!("Webhook: {}", config.webhook_url);
        }

        // ... 处理逻辑

        Ok(HookResult::default())
    }
}
```

### 3. 配置管理命令

```bash
# 设置配置
envcli plugin config set my-plugin timeout 120
envcli plugin config set my-plugin webhook_url "https://my-webhook.com"

# 获取配置
envcli plugin config get my-plugin
envcli plugin config get my-plugin timeout

# 重置配置
envcli plugin config reset my-plugin
```

---

## 高级功能

### 1. 环境变量转换

```rust
impl MyPlugin {
    fn transform_value(&self, value: &str) -> String {
        // 示例：加密敏感值
        if value.contains("secret") || value.contains("password") {
            return format!("***REDACTED***");
        }
        value.to_string()
    }

    fn handle_pre_set(&self, context: &HookContext) -> Result<HookResult, String> {
        // 在设置变量前进行转换
        if let Some(key) = context.env_vars.get("KEY") {
            if let Some(value) = context.env_vars.get("VALUE") {
                let transformed = self.transform_value(value);

                let mut result = HookResult::default();
                result.modified_env.insert(
                    format!("{}_TRANSFORMED", key),
                    transformed
                );

                return Ok(result);
            }
        }

        Ok(HookResult::default())
    }
}
```

### 2. 外部 API 集成

```rust
use reqwest;

impl MyPlugin {
    async fn call_webhook(&self, url: &str, data: &str) -> Result<(), String> {
        let client = reqwest::Client::new();

        let response = client
            .post(url)
            .body(data.to_string())
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("Webhook failed: {}", response.status()))
        }
    }

    fn handle_post_command(&self, context: &HookContext) -> Result<HookResult, String> {
        let config: PluginConfig = self.config_manager.get_config()?;

        if !config.webhook_url.is_empty() {
            // 异步调用（需要运行时支持）
            let data = serde_json::json!({
                "command": context.command_name,
                "timestamp": context.timestamp
            }).to_string();

            // 注意：实际使用需要异步运行时
            // tokio::spawn(async move {
            //     let _ = self.call_webhook(&config.webhook_url, &data).await;
            // });
        }

        Ok(HookResult::default())
    }
}
```

### 3. 错误处理和恢复

```rust
impl MyPlugin {
    fn handle_error(&self, context: &HookContext) -> Result<HookResult, String> {
        if let Some(error) = &context.error {
            // 记录错误日志
            eprintln!("插件错误日志: {}", error);

            // 尝试恢复
            if error.contains("permission") {
                let mut result = HookResult::default();
                result.modified_env.insert(
                    "PERMISSION_ERROR".to_string(),
                    "true".to_string()
                );
                return Ok(result);
            }
        }

        Ok(HookResult::default())
    }
}
```

---

## 测试与调试

### 1. 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use envcli::plugin::test_helpers::*;

    #[test]
    fn test_pre_command_hook() {
        let plugin = MyPlugin {
            metadata: PluginMetadata {
                id: "test-plugin".to_string(),
                name: "Test Plugin".to_string(),
                version: "0.1.0".to_string(),
                author: "Test".to_string(),
                description: "Test plugin".to_string(),
                plugin_type: PluginType::Dynamic,
                enabled: true,
            },
        };

        let context = create_test_context("get", HookType::PreCommand);
        let result = plugin.execute_hook(HookType::PreCommand, &context).unwrap();

        assert!(!result.blocked);
        assert!(result.modified_env.contains_key("PLUGIN_PRE_COMMAND"));
    }

    #[test]
    fn test_error_hook() {
        let plugin = MyPlugin { /* ... */ };

        let mut context = create_test_context("get", HookType::Error);
        context.error = Some("Test error".to_string());

        let result = plugin.execute_hook(HookType::Error, &context).unwrap();
        assert!(!result.blocked);
    }
}
```

### 2. 集成测试

```bash
# 1. 编译插件
cargo build --release

# 2. 加载插件
envcli plugin load target/release/libmy_plugin.so

# 3. 启用插件
envcli plugin enable my-plugin

# 4. 测试命令
envcli get DB_HOST --verbose

# 5. 检查日志
envcli doctor --verbose
```

### 3. 调试技巧

```rust
// 使用 println! 调试
println!("调试: command={}, hook={}", context.command_name, hook_type);

// 使用 eprintln 输出到 stderr
eprintln!("错误: {}", error);

// 打印完整上下文
println!("上下文: {:?}", context);
```

---

## 发布插件

### 1. 准备发布

```toml
# Cargo.toml
[package]
name = "envcli-plugin-my"
version = "0.1.0"
edition = "2021"
description = "My EnvCLI Plugin"
authors = ["Your Name <your@email.com>"]
license = "MIT"
repository = "https://github.com/your-repo/envcli-plugin-my"

[lib]
crate-type = ["cdylib"]

[dependencies]
envcli = { version = "0.1.0", features = ["plugin-sdk"] }
```

### 2. 构建发布版本

```bash
# 构建 release
cargo build --release

# 生成的文件
# target/release/libmy_plugin.so (Linux)
# target/release/libmy_plugin.dylib (macOS)
# target/release/my_plugin.dll (Windows)
```

### 3. 创建插件清单

```json
{
  "id": "my-plugin",
  "name": "My Plugin",
  "version": "0.1.0",
  "author": "Your Name",
  "description": "A helpful plugin for EnvCLI",
  "type": "dynamic",
  "platforms": ["linux", "macos", "windows"],
  "dependencies": {
    "envcli": ">=0.1.0"
  },
  "hooks": ["PreCommand", "PostCommand", "Error"],
  "config": {
    "timeout": 60,
    "webhook_url": "https://example.com"
  }
}
```

### 4. 发布到插件仓库

```bash
# 打包插件
tar -czf my-plugin-0.1.0.tar.gz libmy_plugin.so plugin.json README.md

# 发布到 GitHub Releases
# 或提交到官方插件仓库
```

---

## 完整示例：日志插件

```rust
// src/lib.rs
use envcli::plugin::*;
use std::fs::OpenOptions;
use std::io::Write;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct LoggerPlugin {
    metadata: PluginMetadata,
    log_file: String,
}

impl Plugin for LoggerPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn execute_hook(&self, hook_type: HookType, context: &HookContext) -> Result<HookResult, String> {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let log_entry = format!(
            "[{}] {} - {} - {:?}\n",
            timestamp,
            hook_type,
            context.command_name,
            context.env_vars.keys().collect::<Vec<_>>()
        );

        // 写入日志文件
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file)
        {
            let _ = file.write_all(log_entry.as_bytes());
        }

        Ok(HookResult::default())
    }

    fn initialize(&self) -> Result<(), String> {
        println!("LoggerPlugin initialized, logging to: {}", self.log_file);
        Ok(())
    }
}

create_plugin_entry!(LoggerPlugin, || {
    LoggerPlugin {
        metadata: PluginMetadata {
            id: "logger".to_string(),
            name: "Logger Plugin".to_string(),
            version: "0.1.0".to_string(),
            author: "EnvCLI Team".to_string(),
            description: "记录所有命令执行日志".to_string(),
            plugin_type: PluginType::Dynamic,
            enabled: true,
        },
        log_file: "/tmp/envcli.log".to_string(),
    }
});
```

---

## 最佳实践

### 1. 错误处理
```rust
// 使用 Result 和 ? 操作符
fn handle_hook(&self, context: &HookContext) -> Result<HookResult, String> {
    // 验证输入
    if context.command_name.is_empty() {
        return Err("命令名称不能为空".to_string());
    }

    // 处理逻辑
    let result = do_something()?;

    Ok(result)
}
```

### 2. 性能优化
```rust
// 避免阻塞操作
use std::thread;
use std::time::Duration;

fn handle_post_command(&self, context: &HookContext) -> Result<HookResult, String> {
    // 使用线程处理耗时操作
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(1));
        // 执行耗时任务
    });

    Ok(HookResult::default())
}
```

### 3. 配置验证
```rust
impl Plugin for MyPlugin {
    fn initialize(&self) -> Result<(), String> {
        let config: PluginConfig = self.config_manager.get_config()?;

        // 验证配置
        if config.timeout == 0 {
            return Err("timeout 不能为 0".to_string());
        }

        if config.webhook_url.is_empty() {
            return Err("webhook_url 必须设置".to_string());
        }

        Ok(())
    }
}
```

### 4. 日志记录
```rust
// 使用 eprintln 输出到 stderr
eprintln!("[Plugin {}] {}", self.metadata.id, message);

// 或使用日志库
use log::{info, warn, error};

info!("插件已初始化");
warn!("配置项缺失，使用默认值");
error!("无法连接到 webhook: {}", e);
```

---

## 故障排除

### 1. 插件无法加载

```bash
# 检查文件权限
ls -l target/release/libmy_plugin.so
chmod +x target/release/libmy_plugin.so

# 检查依赖
ldd target/release/libmy_plugin.so  # Linux
otool -L target/release/libmy_plugin.dylib  # macOS
```

### 2. 插件崩溃

```bash
# 启用详细日志
envcli plugin list --verbose

# 检查系统日志
journalctl -xe | grep envcli  # Linux
# 或查看系统事件查看器（Windows）
```

### 3. 配置不生效

```bash
# 重置配置
envcli plugin config reset my-plugin

# 重新加载插件
envcli plugin disable my-plugin
envcli plugin enable my-plugin
```

---

## 下一步

- **API 参考**: 查看 `envcli::plugin` 模块文档
- **示例插件**: 查看官方示例仓库
- **社区支持**: 加入 Discord/Gitter 讨论

---

**文档版本**: v0.1.0
**最后更新**: 2025-12-30
