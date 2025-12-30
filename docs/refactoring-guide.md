# EnvCLI 重构指南：main.rs 模块化

## 🎯 重构目标

将 42KB 的 `main.rs` 拆分为清晰的模块结构，提升代码可维护性和可读性。

### 重构前后对比

**重构前**：
```
main.rs (42KB)
├── CLI 参数定义
├── 所有命令处理逻辑
├── 业务逻辑
├── 工具函数
└── 程序入口
```

**重构后**：
```
src/
├── main.rs (< 100 行) - 程序入口
├── cli.rs - CLI 参数定义
├── commands/ - 命令处理层
│   ├── mod.rs
│   ├── get.rs
│   ├── set.rs
│   └── ...
├── handlers/ - 业务逻辑层
│   ├── mod.rs
│   ├── env_handler.rs
│   ├── plugin_handler.rs
│   └── ...
└── utils/
    └── cli_utils.rs - CLI 工具函数
```

---

## 📋 重构步骤

### Step 1: 创建目录结构

```bash
# 创建命令处理模块目录
mkdir -p src/commands
mkdir -p src/handlers

# 创建模块文件
touch src/commands/mod.rs
touch src/handlers/mod.rs
```

### Step 2: 分析 main.rs 结构

**主要组成部分**：
1. **CLI 参数定义** (clap derive macros)
2. **命令枚举** (Commands)
3. **命令处理函数** (每个子命令的处理逻辑)
4. **工具函数** (路径处理、格式化等)
5. **main() 函数** (入口点)

### Step 3: 迁移 CLI 参数定义

**保持在 cli.rs**：
```rust
// src/cli.rs
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "env")]
#[command(about = "跨平台环境变量管理工具", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 获取环境变量
    Get {
        #[arg(short, long)]
        key: String,
        #[arg(short, long)]
        source: Option<String>,
    },

    /// 设置环境变量
    Set {
        #[arg(short, long)]
        key: String,
        #[arg(short, long)]
        value: String,
        #[arg(short, long)]
        source: Option<String>,
    },

    /// 列出环境变量
    List {
        #[arg(short, long)]
        source: Option<String>,
    },

    // ... 其他命令
}
```

### Step 4: 创建命令处理模块

**命令模块结构**：
```rust
// src/commands/mod.rs
pub mod get;
pub mod set;
pub mod list;
pub mod init;
pub mod export;
pub mod plugin;
pub mod template;
pub mod encrypt;

use crate::handlers::env_handler;
use crate::handlers::plugin_handler;
use crate::handlers::template_handler;

// 命令执行结果类型
pub type CommandResult = Result<(), Box<dyn std::error::Error>>;
```

**单个命令示例**：
```rust
// src/commands/get.rs
use crate::cli::Commands;
use crate::handlers::env_handler::get_env_value;
use super::CommandResult;

pub fn execute_get(key: &str, source: Option<&str>) -> CommandResult {
    match get_env_value(key, source) {
        Ok(Some(value)) => {
            println!("{}", value);
            Ok(())
        }
        Ok(None) => {
            eprintln!("未找到环境变量: {}", key);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("错误: {}", e);
            std::process::exit(1);
        }
    }
}
```

### Step 5: 创建业务逻辑处理层

**环境变量处理器**：
```rust
// src/handlers/env_handler.rs
use crate::core::store::EnvStore;
use crate::types::EnvSource;

/// 获取环境变量值
pub fn get_env_value(key: &str, source: Option<&str>) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let store = EnvStore::new()?;

    if let Some(src_str) = source {
        let src = EnvSource::from_str(src_str)?;
        return store.get_from_source(key, src);
    }

    store.get(key)
}

/// 设置环境变量
pub fn set_env_value(key: &str, value: &str, source: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let store = EnvStore::new()?;
    let src = match source {
        Some(s) => EnvSource::from_str(s)?,
        None => EnvSource::Project, // 默认项目级别
    };

    store.set(key, value, src)?;
    Ok(())
}

/// 列出环境变量
pub fn list_env(source: Option<&str>) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let store = EnvStore::new()?;

    if let Some(src_str) = source {
        let src = EnvSource::from_str(src_str)?;
        return store.list_from_source(src);
    }

    store.list_all()
}
```

**插件处理器**：
```rust
// src/handlers/plugin_handler.rs
use crate::plugin::manager::PluginManager;

/// 列出已安装插件
pub fn list_plugins() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let manager = PluginManager::new()?;
    let plugins = manager.list_plugins()?;

    Ok(plugins)
}

/// 加载并执行插件
pub fn execute_plugin(name: &str, args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let manager = PluginManager::new()?;
    manager.execute(name, &args)?;
    Ok(())
}
```

### Step 6: 简化 main.rs

**重构后的 main.rs**：
```rust
// src/main.rs
use clap::Parser;
use envcli::cli::{Cli, Commands};
use envcli::commands::{get, set, list, init, export, plugin, template, encrypt};

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Get { key, source } => {
            get::execute_get(&key, source.as_deref())
        }
        Commands::Set { key, value, source } => {
            set::execute_set(&key, &value, source.as_deref())
        }
        Commands::List { source } => {
            list::execute_list(source.as_deref())
        }
        Commands::Init { path, template } => {
            init::execute_init(path.as_deref(), template.as_deref())
        }
        Commands::Export { format, output } => {
            export::execute_export(format.as_deref(), output.as_deref())
        }
        Commands::Plugin { subcommand } => {
            plugin::execute_plugin(subcommand)
        }
        Commands::Template { subcommand } => {
            template::execute_template(subcommand)
        }
        Commands::Encrypt { file, backend } => {
            encrypt::execute_encrypt(file.as_deref(), backend.as_deref())
        }
    };

    if let Err(e) = result {
        eprintln!("错误: {}", e);
        std::process::exit(1);
    }
}
```

### Step 7: 更新 lib.rs

```rust
// src/lib.rs
pub mod cli;
pub mod types;
pub mod error;
pub mod config;
pub mod core;
pub mod plugin;
pub mod template;
pub mod utils;

// 导出命令和处理器模块
pub mod commands;
pub mod handlers;
```

---

## 🔍 重构检查清单

### ✅ 功能验证
- [ ] 所有原有命令功能正常
- [ ] CLI 参数解析正确
- [ ] 错误处理逻辑一致
- [ ] 输出格式保持不变

### ✅ 代码质量
- [ ] 所有测试通过
- [ ] Clippy 无警告
- [ ] 代码格式化 (cargo fmt)
- [ ] 文档注释完整

### ✅ 模块化标准
- [ ] main.rs < 100 行
- [ ] 每个命令文件 < 200 行
- [ ] 业务逻辑与 CLI 解耦
- [ ] 模块职责单一

### ✅ 性能验证
- [ ] 启动时间无明显变化
- [ ] 命令执行效率相当
- [ ] 内存使用正常

---

## 🛡️ 风险控制

### 测试保护
```bash
# 重构前运行所有测试
cargo test --all-features

# 每完成一个模块就测试
cargo test --lib
cargo test --test cli_integration

# 重构后完整测试
cargo test --all-features --verbose
```

### 渐进式重构
1. **先创建新结构**，不删除旧代码
2. **并行实现**，确保功能一致
3. **逐步迁移**，每步都验证
4. **最后删除**旧代码

### 版本控制
```bash
# 创建重构分支
git checkout -b refactor/modular-main

# 小步提交
git add src/commands/mod.rs
git commit -m "refactor: create commands module structure"

# 功能完成后再合并
git checkout master
git merge refactor/modular-main
```

---

## 📊 预期收益

### 代码维护性
- **可读性**：⭐⭐⭐⭐⭐ (从 42KB 单文件到模块化)
- **可测试性**：⭐⭐⭐⭐⭐ (模块独立测试)
- **可扩展性**：⭐⭐⭐⭐⭐ (新增命令只需添加模块)

### 开发效率
- **代码审查**：更容易聚焦变更
- **并行开发**：不同模块可独立开发
- **调试定位**：问题更容易定位到具体模块

### 代码质量
- **单一职责**：每个模块职责明确
- **耦合度降低**：CLI 与业务逻辑分离
- **复用性提升**：业务逻辑可被其他模块复用

---

## 🚀 实施建议

### 今日行动
1. ✅ 创建目录结构
2. ✅ 分析 main.rs，列出所有命令
3. ⏳ **开始迁移第一个命令 (Get)**

### 本周目标
- 完成所有命令模块化
- 完成业务逻辑处理层
- main.rs 简化为入口点
- 所有测试通过

### 完成标准
- main.rs < 100 行
- 代码结构清晰
- 功能完整不变
- 测试 100% 通过

---

**重构原则**：小步快跑，测试保护，功能不变，质量提升。