# EnvCLI 插件系统快速开始指南

> 5分钟内创建你的第一个插件！

## 前置准备

```bash
# 确保 EnvCLI 已安装
envcli --version

# 查看插件系统是否可用
envcli plugin list
```

## 方案 1: Rust 动态库插件（推荐用于性能敏感场景）

### 步骤 1: 创建项目

```bash
# 创建新项目
cargo new --lib hello-plugin
cd hello-plugin

# 配置 Cargo.toml
cat > Cargo.toml << 'EOF'
[package]
name = "hello-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["dylib"]

[dependencies]
envcli = { path = "../envcli" }
EOF
```

### 步骤 2: 编写插件代码

编辑 `src/lib.rs`:

```rust
use std::collections::HashMap;
use envcli::plugin::*;

#[derive(Clone)]
struct HelloPlugin {
    metadata: PluginMetadata,
    config: PluginConfig,
}

impl HelloPlugin {
    fn new() -> Self {
        Self {
            metadata: PluginMetadata {
                id: "hello-plugin".to_string(),
                name: "Hello Plugin".to_string(),
                version: "1.0.0".to_string(),
                description: Some("一个简单的问候插件".to_string()),
                author: Some("You".to_string()),
                plugin_type: PluginType::DynamicLibrary,
                hooks: vec![HookType::PreCommand],
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

impl Plugin for HelloPlugin {
    fn metadata(&self) -> PluginMetadata {
        self.metadata.clone()
    }

    fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        self.config = config.clone();
        Ok(())
    }

    fn execute_hook(&self, hook_type: HookType, context: &HookContext) -> Result<HookResult, PluginError> {
        match hook_type {
            HookType::PreCommand => {
                println!("[HelloPlugin] 你好！正在执行命令: {}", context.command);

                let mut modified_env = HashMap::new();
                modified_env.insert("HELLO_PLUGIN".to_string(), "active".to_string());

                Ok(HookResult {
                    modified_env,
                    plugin_data: HashMap::new(),
                    continue_execution: true,
                    message: Some("Hello from plugin!".to_string()),
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

// 工厂函数
#[no_mangle]
pub extern "C" fn create_plugin() -> *mut dyn Plugin {
    let plugin = Box::new(HelloPlugin::new());
    Box::into_raw(plugin)
}

#[no_mangle]
pub extern "C" fn destroy_plugin(plugin: *mut dyn Plugin) {
    if !plugin.is_null() {
        unsafe {
            let _ = Box::from_raw(plugin);
        }
    }
}
```

### 步骤 3: 编译和加载

```bash
# 编译
cargo build --release

# 加载插件
envcli plugin load ./target/release/hello_plugin.dll

# 查看插件列表
envcli plugin list --verbose

# 测试插件
envcli plugin test hello-plugin

# 使用插件（在支持的命令中会自动触发）
envcli get DB_HOST
```

## 方案 2: Python 外部插件（推荐用于快速开发）

### 步骤 1: 创建 Python 脚本

创建 `hello_plugin.py`:

```python
#!/usr/bin/env python3
import json
import sys

def get_metadata():
    return {
        "id": "hello-python",
        "name": "Hello Python Plugin",
        "version": "1.0.0",
        "description": "Python 问候插件",
        "author": "You",
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
    command = context.get("command", "unknown")

    if hook_type == "PreCommand":
        print(f"[HelloPython] 你好！命令: {command}", file=sys.stderr)
        return {
            "modified_env": {"PYTHON_HELLO": "active"},
            "plugin_data": {},
            "continue_execution": True,
            "message": "Python plugin executed"
        }

    return {"modified_env": {}, "plugin_data": {}, "continue_execution": True, "message": None}

if __name__ == "__main__":
    try:
        request = json.load(sys.stdin)
        action = request.get("action")

        if action == "metadata":
            response = {"success": True, "metadata": get_metadata()}
        elif action == "execute_hook":
            result = execute_hook(request["hook_type"], request.get("context", {}))
            response = {"success": True, "result": result}
        elif action == "initialize":
            config = request.get("config", {})
            print(f"[HelloPython] 初始化配置: {config}", file=sys.stderr)
            response = {"success": True}
        elif action == "shutdown":
            print("[HelloPython] 关闭", file=sys.stderr)
            response = {"success": True}
        else:
            response = {"success": False, "error": f"Unknown action: {action}"}

        json.dump(response, sys.stdout)
        sys.stdout.flush()
    except Exception as e:
        json.dump({"success": False, "error": str(e)}, sys.stdout)
        sys.stdout.flush()
        sys.exit(1)
```

### 步骤 2: 加载和测试

```bash
# 赋予执行权限（Linux/macOS）
chmod +x hello_plugin.py

# 加载插件
envcli plugin load ./hello_plugin.py

# 查看插件
envcli plugin list --verbose

# 测试插件
envcli plugin test hello-python

# 测试特定钩子
envcli plugin test hello-python --hook precommand
```

## 插件管理命令速查

```bash
# 查看所有插件
envcli plugin list
envcli plugin list --verbose

# 查看插件详情
envcli plugin show <plugin-id>

# 启用/禁用插件
envcli plugin enable <plugin-id>
envcli plugin disable <plugin-id>

# 卸载插件
envcli plugin unload <plugin-id>

# 查看状态
envcli plugin status
envcli plugin status --plugin <plugin-id>

# 测试钩子
envcli plugin test <plugin-id>
envcli plugin test <plugin-id> --hook precommand

# 配置管理
envcli plugin config set <plugin-id> timeout 30
envcli plugin config get <plugin-id>
envcli plugin config reset <plugin-id>
```

## 调试技巧

### 1. 查看详细日志

```bash
# 使用 verbose 模式
envcli plugin list --verbose
envcli plugin test <id> --verbose
```

### 2. 在插件中添加日志

**Rust:**
```rust
eprintln!("[MyPlugin] 调试信息: {:?}", data);
```

**Python:**
```python
print(f"[MyPlugin] 调试信息: {data}", file=sys.stderr)
```

### 3. 检查错误

```bash
# 查看插件状态
envcli plugin status --plugin <id>

# 查看详细错误
envcli plugin status --plugin <id> --verbose
```

## 常见问题

### Q: 编译动态库时出现链接错误？

**解决：**
```bash
# 确保使用正确的 crate-type
# Cargo.toml 中添加：
[lib]
crate-type = ["dylib"]

# 或者使用 rustc 直接编译：
rustc --crate-type dylib src/lib.rs -o hello_plugin.dll
```

### Q: Python 插件不工作？

**检查：**
1. 文件是否有执行权限：`chmod +x plugin.py`
2. Python 版本是否 >= 3.7
3. 脚本第一行是否正确：`#!/usr/bin/env python3`

### Q: 插件加载但钩子不触发？

**检查：**
1. 插件是否启用：`envcli plugin list --verbose`
2. 钩子类型是否在 metadata 中注册
3. 使用 `plugin test` 直接测试钩子

## 下一步

1. **阅读完整文档**: [PLUGIN_SYSTEM.md](PLUGIN_SYSTEM.md)
2. **查看示例**: [examples/plugin/](examples/plugin/)
3. **查看源码**: [src/plugin/](src/plugin/)

## 提示

- 开发时先用 Python 插件快速验证想法
- 性能关键场景使用 Rust 动态库
- 使用 `plugin test` 命令快速调试
- 记得处理错误，返回 Result 类型
- 钩子执行应快速完成，避免阻塞

---

**祝你开发愉快！** 🚀
