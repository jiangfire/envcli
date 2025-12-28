# EnvCLI 插件系统文档

> **版本**: v0.3.0 | **更新**: 2025-12-27

## 概述

EnvCLI 插件系统提供了一个可扩展的架构，允许开发者通过插件扩展 EnvCLI 的功能。

### 核心特性

- **插件类型**：动态库插件（Rust）、外部可执行文件插件（Python/Shell/Node.js）
- **钩子系统**：7种钩子类型，覆盖命令生命周期
- **优先级系统**：5级优先级，控制钩子执行顺序
- **配置管理**：每个插件可配置独立的设置
- **平台兼容性**：支持 Windows, Linux, macOS
- **类型安全**：Rust trait 系统保证接口正确性

### 架构设计

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
│  - WASM (未来)              │
└─────────────────────────────┘
```

## 快速开始

### 1. 查看插件列表

```bash
envcli plugin list
envcli plugin list --verbose  # 显示详细信息
```

### 2. 查看插件详情

```bash
envcli plugin show <plugin-id>
```

### 3. 管理插件

```bash
# 启用/禁用插件
envcli plugin enable <plugin-id>
envcli plugin disable <plugin-id>

# 加载插件
envcli plugin load <path>

# 卸载插件
envcli plugin unload <plugin-id>
```

### 4. 查看插件状态

```bash
# 查看所有插件状态
envcli plugin status

# 查看单个插件状态
envcli plugin status --plugin <plugin-id>
```

### 5. 测试插件钩子

```bash
# 测试所有钩子
envcli plugin test <plugin-id>

# 测试特定钩子
envcli plugin test <plugin-id> --hook precommand
```

### 6. 插件配置管理

```bash
# 设置配置
envcli plugin config set <plugin-id> <key> <value>

# 获取配置
envcli plugin config get <plugin-id> [key]

# 重置配置
envcli plugin config reset <plugin-id>

# 导出/导入配置
envcli plugin config export
envcli plugin config import <file>
```

## 插件开发

### 插件元数据

每个插件需要一个元数据定义：

```rust
use envcli::plugin::{PluginMetadata, PluginType, HookType};

pub fn get_metadata() -> PluginMetadata {
    PluginMetadata {
        id: "my-plugin".to_string(),
        name: "My Plugin".to_string(),
        version: "1.0.0".to_string(),
        description: Some("一个示例插件".to_string()),
        author: Some("Your Name".to_string()),
        plugin_type: PluginType::DynamicLibrary,
        hooks: vec![HookType::PreCommand, HookType::PostCommand],
        extensions: vec![],
        config_schema: None,
        enabled: true,
        dependencies: vec![],
        platforms: vec![],
        envcli_version: None,
    }
}
```

### 钩子处理器

```rust
use envcli::plugin::{HookType, HookContext, HookResult, PluginError};

pub fn handle_pre_command(context: &HookContext) -> Result<HookResult, PluginError> {
    // 修改环境变量
    let mut modified_env = std::collections::HashMap::new();
    modified_env.insert("MY_VAR".to_string(), "value".to_string());

    Ok(HookResult {
        modified_env,
        plugin_data: std::collections::HashMap::new(),
        continue_execution: true,
        message: Some("Pre-command hook executed".to_string()),
    })
}
```

### 插件 Trait 实现

```rust
use envcli::plugin::{Plugin, PluginConfig, PluginMetadata, HookType, HookContext, HookResult, ExtensionPoint, PluginError};

struct MyPlugin {
    metadata: PluginMetadata,
    config: PluginConfig,
}

impl Plugin for MyPlugin {
    fn metadata(&self) -> PluginMetadata {
        self.metadata.clone()
    }

    fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        self.config = config.clone();
        Ok(())
    }

    fn execute_hook(&self, hook_type: HookType, context: &HookContext) -> Result<HookResult, PluginError> {
        match hook_type {
            HookType::PreCommand => self.handle_pre_command(context),
            HookType::PostCommand => self.handle_post_command(context),
            _ => Ok(HookResult::default()),
        }
    }

    fn supports_extension(&self, _extension: ExtensionPoint) -> bool {
        false
    }

    fn execute_extension(&self, _extension: ExtensionPoint, _input: &[u8]) -> Result<Vec<u8>, PluginError> {
        Err(PluginError::Unsupported("No extensions".to_string()))
    }

    fn shutdown(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}
```

## API 参考

### 核心类型

#### PluginMetadata
```rust
pub struct PluginMetadata {
    pub id: String,                    // 唯一标识符
    pub name: String,                  // 显示名称
    pub version: String,               // 版本号
    pub description: Option<String>,   // 描述
    pub author: Option<String>,        // 作者
    pub plugin_type: PluginType,       // 插件类型
    pub hooks: Vec<HookType>,          // 支持的钩子
    pub extensions: Vec<ExtensionPoint>, // 扩展点
    pub config_schema: Option<ConfigSchema>, // 配置模式
    pub enabled: bool,                 // 是否启用
    pub dependencies: Vec<String>,     // 依赖的插件
    pub platforms: Vec<Platform>,      // 支持的平台
    pub envcli_version: Option<String>, // EnvCLI 版本要求
}
```

#### HookType
- `PreCommand` - 命令执行前
- `PostCommand` - 命令执行后
- `Error` - 错误处理
- `PreRun` - run 命令执行前
- `PostRun` - run 命令执行后
- `ConfigLoad` - 配置加载时
- `ConfigSave` - 配置保存时

#### HookPriority
- `CRITICAL` (10) - 关键优先级，失败会中断
- `HIGH` (50) - 高优先级
- `NORMAL` (100) - 正常优先级
- `LOW` (150) - 低优先级
- `BACKGROUND` (200) - 后台优先级

#### HookContext
```rust
pub struct HookContext<'a> {
    pub command: &'a str,              // 命令名称
    pub args: &'a [String],            // 命令参数
    pub env: HashMap<String, String>,  // 环境变量
    pub plugin_data: HashMap<String, String>, // 插件间共享数据
    pub continue_execution: bool,      // 是否继续执行
    pub error: Option<String>,         // 错误信息（仅 Error 钩子）
}
```

#### HookResult
```rust
pub struct HookResult {
    pub modified_env: HashMap<String, String>,  // 修改后的环境变量
    pub plugin_data: HashMap<String, String>,   // 插件数据更新
    pub continue_execution: bool,               // 是否继续执行
    pub message: Option<String>,                // 消息（用于日志）
}
```

### 错误类型

```rust
pub enum PluginError {
    NotFound(String),           // 插件未找到
    LoadFailed(String),         // 插件加载失败
    ExecutionFailed(String),    // 插件执行失败
    ConfigError(String),        // 插件配置错误
    DependencyMissing(String),  // 插件依赖缺失
    Incompatible(String),       // 插件不兼容
    Timeout(String),            // 超时错误
    AlreadyExists(String),      // 插件已存在
    Unsupported(String),        // 不支持的操作
    Io(std::io::Error),         // IO 错误
    Json(serde_json::Error),    // JSON 错误
    Toml(toml::de::Error),      // TOML 错误
}
```

## 示例插件

### 示例 1: 环境变量注入插件

```rust
// src/lib.rs
use envcli::plugin::*;
use std::collections::HashMap;

#[derive(Clone)]
struct EnvInjectorPlugin {
    metadata: PluginMetadata,
    config: PluginConfig,
}

impl EnvInjectorPlugin {
    fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                id: "env-injector".to_string(),
                name: "Environment Injector".to_string(),
                version: "1.0.0".to_string(),
                description: Some("注入开发环境变量".to_string()),
                author: Some("Dev Team".to_string()),
                plugin_type: PluginType::DynamicLibrary,
                hooks: vec![HookType::PreRun],
                extensions: vec![],
                config_schema: None,
                enabled: true,
                dependencies: vec![],
                platforms: vec![],
                envcli_version: None,
            },
            config: PluginConfig::default(),
        }
    }
}

impl Plugin for EnvInjectorPlugin {
    fn metadata(&self) -> PluginMetadata {
        self.metadata.clone()
    }

    fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        self.config = config.clone();
        Ok(())
    }

    fn execute_hook(&self, hook_type: HookType, _context: &HookContext) -> Result<HookResult, PluginError> {
        match hook_type {
            HookType::PreRun => {
                let mut modified_env = HashMap::new();
                modified_env.insert("DEV_MODE".to_string(), "true".to_string());
                modified_env.insert("LOG_LEVEL".to_string(), "debug".to_string());

                Ok(HookResult {
                    modified_env,
                    plugin_data: HashMap::new(),
                    continue_execution: true,
                    message: Some("Development environment variables injected".to_string()),
                })
            }
            _ => Ok(HookResult::default()),
        }
    }

    fn supports_extension(&self, _extension: ExtensionPoint) -> bool {
        false
    }

    fn execute_extension(&self, _extension: ExtensionPoint, _input: &[u8]) -> Result<Vec<u8>, PluginError> {
        Err(PluginError::Unsupported("No extensions".to_string()))
    }

    fn shutdown(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}

// 工厂函数（动态库需要）
#[no_mangle]
pub extern "C" fn create_plugin() -> *mut dyn Plugin {
    let plugin = Box::new(EnvInjectorPlugin::new());
    Box::into_raw(plugin)
}

#[no_mangle]
pub extern "C" fn destroy_plugin(plugin: *mut dyn Plugin) {
    unsafe {
        if !plugin.is_null() {
            let _ = Box::from_raw(plugin);
        }
    }
}
```

### 示例 2: 外部可执行文件插件

创建一个可执行脚本，通过 JSON 与 EnvCLI 通信：

```python
#!/usr/bin/env python3
# plugin.py
import json
import sys

def get_metadata():
    return {
        "id": "python-plugin",
        "name": "Python Plugin",
        "version": "1.0.0",
        "description": "Python 外部插件",
        "author": "Python Dev",
        "plugin_type": "ExternalExecutable",
        "hooks": ["PreCommand", "PostCommand"],
        "extensions": [],
        "config_schema": None,
        "enabled": True,
        "dependencies": [],
        "platforms": [],
        "envcli_version": None
    }

def execute_hook(hook_type, context):
    if hook_type == "PreCommand":
        return {
            "modified_env": {"PYTHON_PLUGIN": "active"},
            "plugin_data": {},
            "continue_execution": True,
            "message": "Python plugin executed"
        }
    return {
        "modified_env": {},
        "plugin_data": {},
        "continue_execution": True,
        "message": None
    }

if __name__ == "__main__":
    # 读取请求
    request = json.load(sys.stdin)

    if request["action"] == "metadata":
        response = {"success": True, "metadata": get_metadata()}
    elif request["action"] == "execute_hook":
        hook_type = request["hook_type"]
        context = request.get("context", {})
        result = execute_hook(hook_type, context)
        response = {"success": True, "result": result}
    else:
        response = {"success": False, "error": "Unknown action"}

    json.dump(response, sys.stdout)
```

## 最佳实践

### 1. 错误处理
- 始终返回 `Result<HookResult, PluginError>`
- 非关键错误不要中断执行
- 记录详细的错误信息

### 2. 环境变量修改
- 只修改需要的环境变量
- 避免覆盖系统关键变量
- 在消息中说明修改内容

### 3. 性能考虑
- 钩子执行应快速完成
- 避免阻塞操作
- 使用异步操作时考虑超时

### 4. 依赖管理
- 明确声明依赖
- 检查依赖是否存在
- 提供清晰的错误信息

## 调试技巧

### 1. 使用 verbose 模式
```bash
envcli plugin list --verbose
envcli plugin test <id> --verbose
```

### 2. 查看日志
插件执行的详细信息会在 verbose 模式下输出。

### 3. 测试钩子
```bash
# 测试单个钩子
envcli plugin test my-plugin --hook precommand

# 测试所有钩子
envcli plugin test my-plugin
```

## 插件开发快速模板

### Rust 动态库插件

```bash
# 1. 创建项目
cargo new --lib my-plugin
cd my-plugin

# 2. 配置 Cargo.toml
cat > Cargo.toml << 'EOF'
[package]
name = "my-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["dylib"]

[dependencies]
envcli = { path = "../envcli" }
EOF

# 3. 编写插件代码（参考 examples/plugin/example_plugin.rs）
# 4. 编译
cargo build --release

# 5. 加载并测试
envcli plugin load ./target/release/my_plugin.dll
envcli plugin test my-plugin
```

### Python 外部插件

```bash
# 1. 创建脚本
cat > my_plugin.py << 'EOF'
#!/usr/bin/env python3
import json
import sys

def get_metadata():
    return {
        "id": "my-python-plugin",
        "name": "My Python Plugin",
        "version": "1.0.0",
        "description": "Python 插件示例",
        "author": "Your Name",
        "plugin_type": "ExternalExecutable",
        "hooks": ["PreCommand", "PostCommand"],
        "extensions": [],
        "config_schema": None,
        "enabled": True,
        "dependencies": [],
        "platforms": ["Windows", "Linux", "MacOS"],
        "envcli_version": None
    }

def execute_hook(hook_type, context):
    if hook_type == "PreCommand":
        return {
            "modified_env": {"PYTHON_VAR": "active"},
            "plugin_data": {},
            "continue_execution": True,
            "message": "Python hook executed"
        }
    return {"modified_env": {}, "plugin_data": {}, "continue_execution": True, "message": None}

if __name__ == "__main__":
    request = json.load(sys.stdin)
    action = request.get("action")

    if action == "metadata":
        response = {"success": True, "metadata": get_metadata()}
    elif action == "execute_hook":
        result = execute_hook(request["hook_type"], request.get("context", {}))
        response = {"success": True, "result": result}
    else:
        response = {"success": False, "error": "Unknown action"}

    json.dump(response, sys.stdout)
EOF

# 2. 赋予执行权限
chmod +x my_plugin.py

# 3. 加载并测试
envcli plugin load ./my_plugin.py
envcli plugin test my-python-plugin
```

## 常见问题

### Q: 插件加载失败？

**检查清单：**
1. 文件路径是否正确
2. 文件是否有执行权限（Python 插件）
3. 动态库是否与当前平台兼容（Windows .dll, Linux .so, macOS .dylib）
4. 插件元数据是否正确

### Q: 钩子没有执行？

**检查清单：**
1. 插件是否已启用：`envcli plugin list --verbose`
2. 钩子类型是否在 metadata 的 hooks 列表中
3. 使用 `envcli plugin test <id> --hook <type>` 测试单个钩子

### Q: 如何调试插件？

**调试方法：**
1. 在插件代码中添加 `eprintln!`（Rust）或 `print(..., file=sys.stderr)`（Python）
2. 使用 `--verbose` 参数查看详细日志
3. 查看 `plugin test` 的输出
4. 检查 `plugin status` 中的错误信息

### Q: 插件配置不生效？

**检查清单：**
1. 使用 `envcli plugin config get <id>` 查看当前配置
2. 确认配置键名正确
3. 检查插件是否读取了配置（查看插件代码）

## 性能考虑

### 钩子执行性能
- **目标**: 每个钩子 < 10ms
- **建议**:
  - 避免阻塞操作
  - 使用缓存减少重复计算
  - 异步操作需考虑超时

### 插件加载性能
- **首次加载**: ~50-100ms（动态库链接）
- **后续执行**: < 5ms
- **建议**:
  - 只在需要时加载插件
  - 使用 `enable/disable` 而非 `load/unload` 频繁切换

## 安全最佳实践

### 1. 插件来源验证
```bash
# 只加载可信来源的插件
envcli plugin load ./trusted-plugin.dll
```

### 2. 权限控制
- 插件不应访问敏感文件系统区域
- 限制插件执行外部命令
- 使用沙箱环境（未来支持）

### 3. 配置隔离
- 每个插件独立配置空间
- 避免插件间配置冲突
- 敏感配置使用加密存储

## 未来扩展

插件系统计划支持：

- **WASM 插件** - WebAssembly 安全沙箱
- **签名验证** - 插件安全性检查
- **热重载** - 运行时插件更新
- **插件市场** - 在线插件分发
- **依赖解析** - 自动依赖管理

## 版本信息

- **插件系统版本**: v0.3.0
- **EnvCLI 版本**: v0.3.0+
- **更新日期**: 2025-12-27
- **测试状态**: ✅ 17/17 通过

## 许可证

MIT License

---

**相关资源：**
- [示例代码](examples/plugin/)
- [源码参考](src/plugin/)

