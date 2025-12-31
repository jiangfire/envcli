# EnvCLI 开发指南

> **插件开发与最佳实践** | **版本**: v1.0.0

---

## 📖 目录

1. [代码架构原则](#代码架构原则)
2. [插件开发指南](#插件开发指南)
3. [安全最佳实践](#安全最佳实践)
4. [配置管理策略](#配置管理策略)
5. [模板系统规范](#模板系统规范)
6. [团队协作流程](#团队协作流程)
7. [性能优化技巧](#性能优化技巧)
8. [测试与质量](#测试与质量)

---

## 代码架构原则

### KISS 原则（Keep It Simple, Stupid）
- **主函数**：保持在 50 行以内
- **单一职责**：每个函数只做一件事
- **清晰路由**：命令分组处理，逻辑一目了然

```rust
// ✅ 好的做法
fn main() -> Result<()> {
    let args = Cli::parse();
    run_command(args.command, args.verbose)
}

fn run_command(command: Commands, verbose: bool) -> Result<()> {
    match command {
        Commands::Get { key } => handle_get(key, verbose),
        Commands::Set { key, value } => handle_set(key, value, verbose),
        // ...
    }
}
```

### DRY 原则（Don't Repeat Yourself）
- **提取公共逻辑**：识别重复代码并提取为辅助函数
- **统一错误处理**：使用一致的错误处理模式
- **配置复用**：共享配置和常量

```rust
// ❌ 避免重复
fn handle_get(key: &str) -> Result<()> {
    let store = Store::new()?;
    let value = store.get(key)?;
    println!("{}", value);
    Ok(())
}

fn handle_set(key: &str, value: &str) -> Result<()> {
    let store = Store::new()?;
    store.set(key, value)?;
    println!("Set {}={}", key, value);
    Ok(())
}

// ✅ 使用辅助函数
fn handle_result<T: Display>(result: Result<T>, verbose: bool) -> Result<()> {
    match result {
        Ok(value) => {
            if verbose {
                println!("✓ Success: {}", value);
            } else {
                println!("{}", value);
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("✗ Error: {}", e);
            if verbose {
                eprintln!("{:?}", e);
            }
            Err(e)
        }
    }
}
```

### LOD 原则（Law of Demeter）
- **减少耦合**：通过辅助函数封装复杂交互
- **接口清晰**：模块间通过明确接口通信
- **易于测试**：函数可独立测试

```rust
// ❌ 违反 LOD - 过多的链式调用
fn process() -> Result<()> {
    let store = Store::new()?;
    let plugin_manager = PluginManager::new()?;
    let config = Config::load()?;

    store.get("key")?
         .and_then(|v| plugin_manager.transform(v))?
         .and_then(|v| config.validate(v))?;

    Ok(())
}

// ✅ 遵循 LOD - 使用辅助函数
fn process() -> Result<()> {
    let context = create_context()?;
    execute_pipeline(&context)
}

fn execute_pipeline(context: &Context) -> Result<()> {
    let value = get_from_store(context)?;
    let transformed = apply_plugins(value, context)?;
    validate_with_config(transformed, context)?;
    Ok(())
}
```

---

## 插件开发指南

### 插件架构概述

```
┌─────────────────────────────────────────┐
│              EnvCLI CLI                 │
└──────────────┬──────────────────────────┘
               │
               ▼
┌─────────────────────────────────────────┐
│          PluginManager                  │
│  - 加载/卸载插件                        │
│  - 管理插件生命周期                     │
│  - 执行钩子链                           │
│  - 配置管理                             │
└──────────────┬──────────────────────────┘
               │
       ┌───────┴───────┐
       ▼               ▼
┌─────────────┐  ┌──────────────┐
│ Hook        │  │ Plugin       │
│ Dispatcher  │  │ Config       │
└─────────────┘  └──────────────┘
       │               │
       ▼               ▼
┌─────────────────────────────┐
│      Plugin Types           │
│  - Dynamic Library (.dll)   │
│  - External Executable      │
│  - Script (Shell/Python)    │
└─────────────────────────────┘
```

### 插件类型对比

| 类型 | 说明 | 适用场景 | 性能 | 开发难度 |
|------|------|----------|------|----------|
| **Dynamic** | Rust 动态库 (.so/.dll) | 高性能、深度集成 | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ |
| **Executable** | 可执行文件 | 任何语言、易于开发 | ⭐⭐⭐ | ⭐⭐ |
| **Script** | Shell/Python 脚本 | 快速原型、简单逻辑 | ⭐⭐ | ⭐ |

### 钩子系统详解

| 钩子 | 触发时机 | 典型用途 | 返回值影响 |
|------|----------|----------|------------|
| **PreCommand** | 命令执行前 | 日志、验证、环境准备 | 可阻止命令执行 |
| **PostCommand** | 命令执行后 | 清理、通知、结果处理 | 不影响结果 |
| **Error** | 发生错误时 | 错误报告、恢复 | 可修改错误 |
| **PreSet** | 设置变量前 | 数据验证、转换 | 可修改/阻止设置 |
| **PostGet** | 获取变量后 | 数据解密、转换 | 可修改返回值 |
| **PreDelete** | 删除变量前 | 验证依赖 | 可阻止删除 |
| **PostDelete** | 删除变量后 | 清理关联数据 | 不影响结果 |

### 优先级系统

```rust
pub enum PluginPriority {
    Critical = 0,    // 关键插件，最先执行
    High = 1,        // 高优先级
    Normal = 2,      // 正常优先级（默认）
    Low = 3,         // 低优先级
    Background = 4,  // 后台任务
}
```

### 开发 Rust 动态库插件

#### 步骤 1: 项目设置
```bash
cargo new --lib my-plugin
cd my-plugin
```

#### 步骤 2: Cargo.toml 配置
```toml
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["dylib"]

[dependencies]
envcli = { path = "../envcli" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

#### 步骤 3: 实现插件
```rust
use std::collections::HashMap;
use envcli::plugin::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct MyPluginConfig {
    api_key: Option<String>,
    log_level: String,
}

#[derive(Clone)]
struct MyPlugin {
    metadata: PluginMetadata,
    config: PluginConfig,
}

impl Plugin for MyPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn config(&self) -> &PluginConfig {
        &self.config
    }

    fn config_mut(&mut self) -> &mut PluginConfig {
        &mut self.config
    }

    // 命令执行前钩子
    fn on_pre_command(&self, context: &HookContext) -> Result<HookResult> {
        match context.command {
            "set" | "system-set" => {
                // 验证 API 密钥是否存在
                if let Some(api_key) = self.config.get("api_key") {
                    if api_key.is_empty() {
                        return Ok(HookResult::Error(
                            "API key is required for set operations".to_string()
                        ));
                    }
                }

                // 记录操作日志
                if self.config.get("log_level") == Some(&"debug".to_string()) {
                    println!("[MyPlugin] Pre-command: {}", context.command);
                }

                Ok(HookResult::Continue)
            }
            _ => Ok(HookResult::Continue),
        }
    }

    // 获取变量后钩子
    fn on_post_get(&self, context: &HookContext, value: &str) -> Result<HookResult> {
        // 可以对值进行转换或解密
        if value.starts_with("encrypted:") {
            let decrypted = decrypt_value(value)?;
            return Ok(HookResult::Modify(decrypted));
        }

        Ok(HookResult::Continue)
    }

    // 错误处理钩子
    fn on_error(&self, context: &HookContext, error: &str) -> Result<HookResult> {
        // 发送错误到监控服务
        if let Some(api_key) = self.config.get("api_key") {
            self.send_error_to_monitoring(api_key, context, error)?;
        }

        Ok(HookResult::Continue)
    }
}

// 辅助函数
fn decrypt_value(value: &str) -> Result<String> {
    // 实现解密逻辑
    Ok(value.replace("encrypted:", ""))
}

impl MyPlugin {
    fn send_error_to_monitoring(
        &self,
        api_key: &str,
        context: &HookContext,
        error: &str,
    ) -> Result<()> {
        // 实现发送逻辑
        println!("[Monitoring] Error: {} in command {}", error, context.command);
        Ok(())
    }
}

// 插件入口函数
#[no_mangle]
pub fn create_plugin() -> Box<dyn Plugin> {
    let mut config = PluginConfig::new();
    config.set("log_level", "info");

    Box::new(MyPlugin {
        metadata: PluginMetadata {
            name: "my-plugin".to_string(),
            version: "0.1.0".to_string(),
            author: "Your Name".to_string(),
            description: "My custom plugin".to_string(),
            priority: PluginPriority::Normal,
            hooks: vec![
                HookType::PreCommand,
                HookType::PostGet,
                HookType::Error,
            ],
        },
        config,
    })
}
```

#### 步骤 4: 编译和测试
```bash
# 编译
cargo build --release

# 测试加载
envcli plugin load target/release/libmy_plugin.so

# 查看插件列表
envcli plugin list

# 启用插件
envcli plugin enable my-plugin

# 配置插件
envcli plugin config my-plugin set api_key "your-api-key"
envcli plugin config my-plugin set log_level "debug"
```

### 外部可执行插件

#### Shell 脚本示例
```bash
#!/bin/bash
# my-plugin.sh

# 读取配置
CONFIG_FILE="$HOME/.envcli/plugins/my-plugin/config.json"
API_KEY=$(jq -r '.api_key // empty' "$CONFIG_FILE" 2>/dev/null)

case "$1" in
    "pre_command")
        COMMAND="$2"

        if [[ "$COMMAND" == "set" || "$COMMAND" == "system-set" ]]; then
            if [[ -z "$API_KEY" ]]; then
                echo "ERROR: API key required"
                exit 1
            fi
            echo "OK: Pre-command validation passed"
        fi
        ;;

    "post_get")
        VALUE="$2"

        # 解密逻辑
        if [[ "$VALUE" == encrypted:* ]]; then
            echo "${VALUE#encrypted:}"
        else
            echo "$VALUE"
        fi
        ;;

    "error")
        ERROR="$2"
        COMMAND="$3"

        # 发送到监控
        if [[ -n "$API_KEY" ]]; then
            curl -X POST https://monitoring.example.com/api/errors \
                -H "Authorization: Bearer $API_KEY" \
                -d "{\"error\":\"$ERROR\",\"command\":\"$COMMAND\"}"
        fi
        ;;

    *)
        echo "Unknown command: $1"
        exit 1
        ;;
esac
```

#### Python 插件示例
```python
#!/usr/bin/env python3
# my_plugin.py

import json
import sys
import os
from pathlib import Path

class MyPlugin:
    def __init__(self):
        self.config_path = Path.home() / ".envcli" / "plugins" / "my-plugin" / "config.json"
        self.config = self.load_config()

    def load_config(self):
        if self.config_path.exists():
            return json.loads(self.config_path.read_text())
        return {"api_key": "", "log_level": "info"}

    def pre_command(self, command):
        if command in ["set", "system-set"]:
            if not self.config.get("api_key"):
                print("ERROR: API key required", file=sys.stderr)
                sys.exit(1)

        if self.config.get("log_level") == "debug":
            print(f"[MyPlugin] Pre-command: {command}")

        print("OK")

    def post_get(self, value):
        if value.startswith("encrypted:"):
            return value[10:]  # Remove "encrypted:" prefix
        return value

    def error(self, error_msg, command):
        api_key = self.config.get("api_key")
        if api_key:
            # Send to monitoring service
            print(f"[Monitoring] Error: {error_msg} in {command}")

        print("OK")

if __name__ == "__main__":
    plugin = MyPlugin()
    command = sys.argv[1]

    if command == "pre_command":
        plugin.pre_command(sys.argv[2])
    elif command == "post_get":
        print(plugin.post_get(sys.argv[2]))
    elif command == "error":
        plugin.error(sys.argv[2], sys.argv[3])
```

### 插件签名验证

```rust
// 签名验证系统
use ring::signature::{Ed25519KeyPair, Signature, UnparsedPublicKey, ED25519};
use sha2::{Sha256, Digest};

pub struct SignatureVerifier {
    public_key: Vec<u8>,
}

impl SignatureVerifier {
    pub fn new(public_key: Vec<u8>) -> Self {
        Self { public_key }
    }

    pub fn verify(&self, plugin_path: &str, signature: &str) -> Result<bool> {
        // 读取插件文件
        let plugin_data = std::fs::read(plugin_path)?;

        // 计算哈希
        let mut hasher = Sha256::new();
        hasher.update(&plugin_data);
        let hash = hasher.finalize();

        // 验证签名
        let public_key = UnparsedPublicKey::new(&ED25519, &self.public_key);
        let signature_bytes = hex::decode(signature)?;

        match public_key.verify(&hash, &signature_bytes) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}
```

### 热重载系统

```rust
use notify::{Watcher, RecursiveMode, Result as NotifyResult};
use std::sync::mpsc::channel;
use std::time::Duration;

pub struct PluginWatcher {
    watcher: notify::RecommendedWatcher,
}

impl PluginWatcher {
    pub fn new(plugin_dir: &str) -> NotifyResult<Self> {
        let (tx, rx) = channel();

        let mut watcher = notify::RecommendedWatcher::new(tx)?;
        watcher.watch(plugin_dir.as_ref(), RecursiveMode::NonRecursive)?;

        // 防抖处理
        std::thread::spawn(move || {
            let mut last_event = std::time::Instant::now();

            while let Ok(event) = rx.recv() {
                if last_event.elapsed() > Duration::from_millis(500) {
                    // 处理插件变化
                    println!("Plugin changed, reloading...");
                    // 重新加载逻辑
                    last_event = std::time::Instant::now();
                }
            }
        });

        Ok(Self { watcher })
    }
}
```

---

## 安全最佳实践

### 1. 敏感数据加密

**推荐做法**：
```bash
# 使用 SOPS 加密敏感配置
env encrypt --backend age secrets.env

# 配置 SOPS 使用 Age 密钥
export SOPS_AGE_KEY_FILE=~/.config/sops/age/keys.txt
```

**配置示例**：
```yaml
# .sops.yaml
creation_rules:
  - path_regex: secrets\\.env$
    age: age1ql3z7hjy54pw3hyww5ayyfg7zqgvc7w3j2elw8zmrj2kg5sfn9aqmcac8p
```

### 2. 密钥管理

**✅ 正确做法**：
- 密钥文件权限设置为 `600`
- 密钥不提交到版本控制
- 使用环境变量或密钥管理服务
- 定期轮换密钥

**❌ 避免做法**：
- 硬编码密钥
- 提交到 Git 仓库
- 使用弱加密算法
- 共享密钥文件

### 3. 权限控制

```bash
# 默认使用用户级变量（无需管理员权限）
envcli set API_KEY secret --source=user

# 系统级变量需要显式指定
envcli system-set API_KEY secret --scope=global

# Unix 系统限制机器级操作
# Windows 需要管理员权限
```

### 4. 插件安全

```rust
// 插件签名验证
pub fn load_plugin_with_verification(path: &str, public_key: &str) -> Result<Box<dyn Plugin>> {
    let verifier = SignatureVerifier::new(hex::decode(public_key)?);

    // 验证签名
    let signature = read_signature_file(path)?;
    if !verifier.verify(path, &signature)? {
        return Err(Error::InvalidSignature);
    }

    // 验证通过后加载
    load_plugin(path)
}

// 插件沙箱限制
pub struct PluginSandbox {
    allowed_paths: Vec<PathBuf>,
    network_access: bool,
}

impl PluginSandbox {
    pub fn check_access(&self, path: &Path) -> Result<()> {
        if !self.allowed_paths.iter().any(|p| path.starts_with(p)) {
            return Err(Error::AccessDenied);
        }
        Ok(())
    }
}
```

---

## 配置管理策略

### 层级使用策略

| 层级 | 用途 | 示例 | Git 跟踪 |
|------|------|------|----------|
| **Local** | 本地开发配置 | `DEBUG=true` | ❌ 忽略 |
| **Project** | 团队共享配置 | `DB_HOST=localhost` | ✅ 提交 |
| **User** | 个人敏感信息 | `API_KEY=secret` | ❌ 忽略 |
| **System** | 机器全局配置 | `PATH=/usr/bin` | ❌ 不适用 |

### 版本控制配置

```bash
# .gitignore
.envcli/local.env
.envcli/user.env
.envcli/*.local.env

# 但保留
!.envcli/project.env
!.envcli/.gitkeep
```

### 配置模板

```bash
# .env.example (提交到 Git)
DB_HOST=localhost
DB_PORT=5432
DB_USER=dev_user
# API_KEY=your_key_here  # 个人配置，不提交
```

---

## 模板系统规范

### 语法规范

```bash
# 基础变量替换
DB_URL={{DB_HOST}}:{{DB_PORT}}/{{DB_NAME}}

# 默认值
API_URL={{API_URL|http://localhost:3000}}

# 环境变量
SECRET={{SECRET_KEY}}
```

### 继承策略

```rust
// 模板解析器
pub struct TemplateParser {
    variables: HashMap<String, String>,
    defaults: HashMap<String, String>,
}

impl TemplateParser {
    pub fn render(&self, template: &str) -> Result<String> {
        let mut result = template.to_string();

        // 替换变量
        for (key, value) in &self.variables {
            let placeholder = format!("{{{{{}}}}}", key);
            result = result.replace(&placeholder, value);
        }

        // 处理默认值
        for (key, default) in &self.defaults {
            let placeholder = format!("{{{{{}}}|{}}}", key, default);
            result = result.replace(&placeholder, default);
        }

        // 检测未替换的变量
        if result.contains("{{") {
            return Err(Error::MissingVariables);
        }

        Ok(result)
    }
}
```

### 循环依赖检测

```rust
pub fn detect_cycle(
    template: &str,
    visited: &mut HashSet<String>,
) -> Result<()> {
    let variables = extract_variables(template);

    for var in variables {
        if !visited.insert(var.clone()) {
            return Err(Error::CircularDependency(var));
        }

        // 递归检查依赖
        if let Some(value) = self.variables.get(&var) {
            self.detect_cycle(value, visited)?;
        }

        visited.remove(&var);
    }

    Ok(())
}
```

---

## 团队协作流程

### 1. 配置分层策略

```bash
# 项目结构
project/
├── .envcli/
│   ├── project.env      # ✅ 提交 - 团队共享
│   ├── local.env        # ❌ 忽略 - 本地开发
│   └── .gitkeep
├── .env.example         # ✅ 提交 - 配置模板
├── README.md            # ✅ 提交 - 环境说明
└── .gitignore           # ✅ 提交 - 忽略规则
```

### 2. 文档化要求

**README.md 环境部分**：
```markdown
## 环境配置

1. 复制 `.env.example` 到 `.envcli/local.env`
2. 设置必要的变量：
   ```bash
   envcli set DB_HOST localhost --source=local
   envcli set API_KEY your_key --source=local
   ```
3. 运行健康检查：
   ```bash
   envcli doctor
   ```
```

### 3. 加密协作流程

```bash
# 1. 生成团队加密密钥
age-keygen -o team-keys.txt

# 2. 导出公钥
age-keygen -y team-keys.txt > team-public.key

# 3. 加密共享配置
env encrypt --backend age team-secrets.env

# 4. 提交加密文件
git add team-secrets.env.envenc
git commit -m "Add encrypted team secrets"

# 5. 团队成员配置
export SOPS_AGE_KEY_FILE=~/.config/sops/age/team-keys.txt
```

### 4. 代码审查清单

- [ ] 配置文件是否正确忽略？
- [ ] 敏感信息是否加密？
- [ ] 文档是否更新？
- [ ] 测试是否通过？
- [ ] 向后兼容性？

---

## 性能优化技巧

### 1. 缓存策略

```rust
// 系统环境缓存（60秒 TTL）
static SYSTEM_ENV_CACHE: OnceLock<Mutex<Option<SystemEnvCache>>> = OnceLock::new();

// 文件内容缓存（基于修改时间）
static FILE_CACHE: OnceLock<RwLock<HashMap<PathBuf, FileCacheEntry>>> = OnceLock::new();
```

### 2. 算法优化

```rust
// ❌ 低效：4次遍历 + 4次文件读取
for source in [System, User, Project, Local] {
    let vars = store.list(Some(source))?;
    // ...
}

// ✅ 高效：1次遍历，利用缓存
let all_vars = store.list(None)?;
```

### 3. I/O 优化

```rust
// 使用 RwLock 优化读多写少场景
pub fn get_file_cache() -> &'static RwLock<HashMap<PathBuf, FileCacheEntry>> {
    FILE_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

// 批量操作
pub fn batch_get(&self, keys: &[&str]) -> Result<Vec<Option<String>>> {
    let cache = get_file_cache().read().unwrap();
    // 批量从缓存读取
}
```

### 4. 性能监控

```bash
# 查看缓存统计
envcli cache stats --verbose

# 性能对比测试
time envcli get TEST_VAR1
time envcli cache clear all && envcli get TEST_VAR1
```

---

## 测试与质量

### 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_parser() {
        let mut parser = TemplateParser::new();
        parser.set("DB_HOST", "localhost");
        parser.set("DB_PORT", "5432");

        let result = parser.render("{{DB_HOST}}:{{DB_PORT}}").unwrap();
        assert_eq!(result, "localhost:5432");
    }

    #[test]
    fn test_circular_dependency_detection() {
        let mut parser = TemplateParser::new();
        parser.set("A", "{{B}}");
        parser.set("B", "{{A}}");

        let result = parser.render("{{A}}");
        assert!(result.is_err());
    }
}
```

### 集成测试

```rust
#[test]
fn test_plugin_lifecycle() {
    let manager = PluginManager::new();

    // 加载插件
    let plugin = manager.load("test_plugin.so").unwrap();
    assert_eq!(plugin.metadata().name, "test-plugin");

    // 启用插件
    manager.enable("test-plugin").unwrap();
    assert!(manager.is_enabled("test-plugin"));

    // 执行钩子
    let context = HookContext::new("set");
    let result = manager.execute_hooks(HookType::PreCommand, &context);
    assert!(result.is_ok());
}
```

### 代码质量检查

**Clippy 规则**：
```toml
# .cargo/config
[build]
rustflags = ["-Dwarnings"]

[clippy]
avoid-breaking-exported-api = "allow"
cognitive-complexity = "15"
```

**代码审查要点**：
- [ ] **KISS**: 函数是否简单清晰？（主函数 50 行以内）
- [ ] **DRY**: 是否有重复代码？（已提取 27+ 个辅助函数）
- [ ] **LOD**: 模块耦合是否合理？（通过辅助函数封装）
- [ ] **测试**: 1000+ 行测试代码，100% 通过
- [ ] **文档**: 公共 API 有文档注释
- [ ] **错误处理**: 所有错误都被处理（统一错误处理链）
- [ ] **性能**: 无明显性能瓶颈（已优化缓存）
- [ ] **编译**: 0 错误，0 Clippy 警告

---

## 部署和发布

### 发布前检查清单

```bash
# 1. 运行完整测试套件
cargo test --all-features
cargo clippy -- -D warnings

# 2. 检查代码格式
cargo fmt -- --check

# 3. 构建发布版本
cargo build --release

# 4. 验证二进制文件
./target/release/envcli --version
./target/release/envcli doctor

# 5. 更新版本号
# 修改 Cargo.toml 和 CHANGELOG.md

# 6. 创建 Git 标签
git tag v0.1.0
git push origin v0.1.0
```

### CI/CD 配置

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  release:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]

    steps:
      - uses: actions/checkout@v3

      - name: Build
        run: cargo build --release

      - name: Test
        run: cargo test --release

      - name: Upload Release
        uses: softprops/action-gh-release@v1
        with:
          files: target/release/envcli*
```

---

## 📚 相关资源

- **项目概览**: [project-overview.md](./project-overview.md)
- **用户指南**: [user-guide.md](./user-guide.md)
- **变更日志**: [CHANGELOG.md](./CHANGELOG.md)

---

**文档版本**: v1.0.0
**最后更新**: 2025-12-31
**维护者**: EnvCLI 团队
