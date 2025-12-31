# EnvCLI 用户指南

> **5分钟上手环境变量管理** | **版本**: v0.1.0

---

## 📦 安装

### 1. 下载二进制文件
```bash
# 从 GitHub Releases 下载
# Windows: envcli.exe
# Linux/macOS: envcli
```

### 2. 添加到 PATH
```bash
# Windows (PowerShell)
$env:PATH += ";C:\path\to\envcli"

# Linux/macOS
sudo mv envcli /usr/local/bin/
chmod +x /usr/local/bin/envcli
```

### 3. 验证安装
```bash
envcli --version
# 输出: envcli v0.1.0

envcli --help
# 显示所有可用命令
```

---

## 🚀 5分钟快速上手

### 第 1 分钟：设置你的第一个变量
```bash
# 设置一个变量（默认写入 local 层级）
envcli set DB_HOST localhost

# 验证设置成功
envcli get DB_HOST
# 输出: localhost
```

### 第 2 分钟：查看所有变量
```bash
# 查看所有层级的变量（合并视图）
envcli list

# 查看特定层级
envcli list --source=local
envcli list --source=user
envcli list --source=project
envcli list --source=system

# 输出格式：JSON
envcli list --format=json
```

### 第 3 分钟：多层级管理
```bash
# 不同层级设置相同变量
envcli system-set API_KEY prod_key --scope=global
envcli set API_KEY dev_key

# 查看优先级（local 会覆盖 user）
envcli get API_KEY
# 输出: dev_key (来自 local)

# 按层级查看
envcli list --source=system  # prod_key
envcli list --source=local   # dev_key
```

### 第 4 分钟：导入导出
```bash
# 导出当前变量到 .env 文件
envcli export > .env

# 从 .env 文件导入
envcli import .env

# 导出特定层级
envcli export --source=project > project.env
```

### 第 5 分钟：诊断和验证
```bash
# 运行健康检查
envcli doctor

# 验证配置文件格式
envcli config validate

# 查看配置信息
envcli config info
```

---

## 🎯 核心概念

### 层级系统
EnvCLI 使用 4 个层级，**优先级从高到低**：

| 层级 | 说明 | 适用场景 | 文件位置 |
|------|------|----------|----------|
| **Local** | 项目本地 | 当前项目配置 | `./.envcli/local.env` |
| **Project** | 项目级 | 团队共享配置 | `./.envcli/project.env` |
| **User** | 用户级 | 个人全局配置 | `~/.envcli/user.env` |
| **System** | 系统级 | 机器全局配置 | 系统环境变量 |

**变量查找顺序**：
```
local > project > user > system
```

---

## 📋 常用命令速查

### 核心操作
```bash
envcli get <KEY>                    # 获取变量
envcli set <KEY> <VALUE>            # 设置变量 (Local 层)
envcli unset <KEY>                  # 删除变量 (Local 层)
envcli list                         # 列出所有变量 (合并视图)
envcli list --source=project        # 指定层级
envcli list --format=json           # JSON 输出
```

### 系统级操作
```bash
envcli system-set <KEY> <VALUE>     # 设置系统变量 (默认 user 层)
envcli system-set <KEY> <VALUE> --scope machine  # 机器级 (需管理员)
envcli system-unset <KEY>           # 删除系统变量
envcli system-unset <KEY> --scope machine
```

### 导入导出
```bash
envcli import <FILE>                # 导入 .env 到 Local 层
envcli import <FILE> --target=project  # 导入到指定层
envcli export                       # 导出所有变量
envcli export --source=project      # 导出指定层级
envcli export --format=json         # JSON 格式导出
```

### 配置管理
```bash
envcli config validate              # 验证配置格式
envcli config validate --verbose    # 详细验证
envcli config init                  # 初始化配置
envcli config init --force          # 强制重新初始化
envcli config info                  # 显示配置信息
```

### 诊断工具
```bash
envcli doctor                       # 健康检查
envcli doctor --verbose             # 详细诊断
envcli status                       # 显示状态信息
```

### 运行命令
```bash
envcli run KEY=value -- <COMMAND>   # 临时环境运行
envcli run --from-file .env -- <COMMAND>  # 从文件加载
envcli run DB_HOST=localhost DB_PORT=5432 -- python app.py
```

### 加密解密
```bash
envcli set <KEY> <VALUE> --encrypt        # 加密存储
envcli encrypt <KEY> <VALUE>              # 加密并存储
envcli encrypt <KEY> <VALUE> --target=project
envcli decrypt <KEY>                      # 解密查看
envcli decrypt <KEY> --source=project
envcli check-sops                         # 检查 SOPS 状态
```

### 模板系统
```bash
envcli template create <NAME> --vars VAR1 VAR2  # 创建模板
envcli template create <NAME> --inherits base --vars VAR1
envcli template list                      # 列出模板
envcli template list --verbose            # 详细信息
envcli template show <NAME>               # 查看模板
envcli template render <NAME> --var VAR1=value -o output.env
envcli template render <NAME> --interactive  # 交互式
envcli template delete <NAME>             # 删除模板
```

### 插件系统
```bash
envcli plugin list                       # 列出插件
envcli plugin list --verbose             # 详细信息
envcli plugin show <PLUGIN_ID>           # 查看插件详情
envcli plugin load <PATH>                # 加载插件
envcli plugin enable <PLUGIN_ID>         # 启用插件
envcli plugin disable <PLUGIN_ID>        # 禁用插件
envcli plugin unload <PLUGIN_ID>         # 卸载插件
envcli plugin reload <PLUGIN_ID>         # 热重载
envcli plugin status                     # 插件状态统计
envcli plugin status <PLUGIN_ID>         # 单个插件状态
envcli plugin test <PLUGIN_ID>           # 测试插件钩子
envcli plugin check-deps <PLUGIN_ID>     # 检查依赖
envcli plugin load-deps <PATH1> <PATH2>  # 加载依赖
envcli plugin generate-key-pair          # 生成签名密钥
envcli plugin sign <PLUGIN_ID> --key <KEY> --output sig.json
envcli plugin verify <PLUGIN_ID>         # 验证签名
envcli plugin verify-all                 # 验证所有签名
envcli plugin fingerprint <PUBLIC_KEY>   # 显示指纹
envcli plugin config set <PLUGIN_ID> <KEY> <VALUE>  # 配置插件
envcli plugin config get <PLUGIN_ID> <KEY>          # 获取配置
envcli plugin config reset <PLUGIN_ID>              # 重置配置
```

### 缓存管理
```bash
envcli cache stats                      # 查看缓存统计
envcli cache clear file                 # 清除文件缓存
envcli cache clear system               # 清除系统环境缓存
envcli cache clear all                  # 清除所有缓存
```

---

## 🔐 加密存储

### 前置要求
1. **安装 SOPS**
```bash
# macOS
brew install sops

# Linux
wget https://github.com/mozilla/sops/releases/download/v3.8.1/sops_3.8.1_amd64.deb
sudo dpkg -i sops_3.8.1_amd64.deb

# Windows
choco install sops
# 或
scoop install sops
```

2. **配置加密后端**

**选项 A: GPG（最简单）**
```bash
# 生成 GPG 密钥
gpg --generate-key

# 查看密钥 ID
gpg --list-secret-keys --keyid-format LONG
```

**选项 B: Age（推荐）**
```bash
# 安装 age
# macOS: brew install age
# Linux: 下载 release

# 生成密钥
age-keygen -o ~/.config/sops/age/keys.txt

# 获取公钥
age-keygen -y ~/.config/sops/age/keys.txt
```

### 使用加密

```bash
# 加密存储敏感变量
envcli set DB_PASS secret --encrypt

# 解密查看
envcli decrypt DB_PASS

# 检查 SOPS 状态
envcli check-sops

# 加密文件
env encrypt --backend age secrets.env
```

### 配置示例
```yaml
# .sops.yaml
creation_rules:
  - path_regex: secrets\.env$
    age: age1ql3z7hjy54pw3hyww5ayyfg7zqgvc7w3j2elw8zmrj2kg5sfn9aqmcac8p
```

---

## 🔌 插件系统

### 插件类型
| 类型 | 说明 | 适用场景 |
|------|------|----------|
| **Dynamic** | Rust 动态库 (.so/.dll) | 高性能、深度集成 |
| **Executable** | 可执行文件 | 任何语言、易于开发 |
| **Script** | Shell/Python 脚本 | 快速原型、简单逻辑 |

### 钩子类型
| 钩子 | 触发时机 | 典型用途 |
|------|----------|----------|
| **PreCommand** | 命令执行前 | 日志、验证、环境准备 |
| **PostCommand** | 命令执行后 | 清理、通知、结果处理 |
| **Error** | 发生错误时 | 错误报告、恢复 |
| **PreSet** | 设置变量前 | 数据验证、转换 |
| **PostGet** | 获取变量后 | 数据解密、转换 |

### 常用命令
```bash
# 列出插件
envcli plugin list

# 加载插件
envcli plugin load /path/to/plugin.so

# 启用插件
envcli plugin enable my-plugin

# 卸载插件
envcli plugin disable my-plugin
```

### 开发插件（Rust 动态库）

**步骤 1: 创建项目**
```bash
cargo new --lib hello-plugin
cd hello-plugin
```

**步骤 2: 配置 Cargo.toml**
```toml
[package]
name = "hello-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["dylib"]

[dependencies]
envcli = { path = "../envcli" }
```

**步骤 3: 编写插件**
```rust
use std::collections::HashMap;
use envcli::plugin::*;

#[derive(Clone)]
struct HelloPlugin {
    metadata: PluginMetadata,
    config: PluginConfig,
}

impl Plugin for HelloPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn on_pre_command(&self, context: &HookContext) -> Result<HookResult> {
        println!("Hello from plugin! Command: {}", context.command);
        Ok(HookResult::Continue)
    }
}

#[no_mangle]
pub fn create_plugin() -> Box<dyn Plugin> {
    Box::new(HelloPlugin {
        metadata: PluginMetadata {
            name: "hello-plugin".to_string(),
            version: "0.1.0".to_string(),
            author: "Your Name".to_string(),
            description: "A hello world plugin".to_string(),
        },
        config: PluginConfig::new(),
    })
}
```

**步骤 4: 编译和加载**
```bash
cargo build --release
envcli plugin load target/release/libhello_plugin.so
envcli plugin enable hello-plugin
```

---

## 💡 常见工作流

### 1. 项目配置
```bash
# 在项目目录初始化
envcli config init

# 设置项目级变量
envcli set DB_HOST localhost --source=project
envcli set DB_PORT 5432 --source=project

# 提交到版本控制
git add .envcli/project.env
git commit -m "Add project env vars"
```

### 2. 个人敏感信息
```bash
# 设置个人级变量（不提交到 git）
envcli set API_KEY secret_key --source=user

# 导出备份
envcli export --source=user > backup.user.env
```

### 3. 临时开发环境
```bash
# 设置临时变量
envcli set DEBUG true --source=local

# 运行程序
envcli run DEBUG=true LOG_LEVEL=debug -- cargo run

# 清理
envcli unset DEBUG --source=local
```

---

## 🐛 故障排除

### 变量未找到
```bash
# 解决方案 1: 查看所有变量
envcli list

# 解决方案 2: 按层级搜索
envcli list --source=local

# 解决方案 3: 运行诊断
envcli doctor
```

### 权限被拒绝
```bash
# 解决方案 1: 使用用户级变量
envcli set KEY value --source=user

# 解决方案 2: Windows 上以管理员运行
# 右键 PowerShell/CMD → 以管理员身份运行
```

### 配置文件格式错误
```bash
# 验证配置
envcli config validate --verbose

# 修复格式：每行 KEY=VALUE
# 示例：
# DB_HOST=localhost
# DB_PORT=5432
```

### 需要详细错误信息
```bash
# 使用 --verbose 标志
envcli get DB_HOST --verbose
envcli doctor --verbose
```

---

## 🛠️ 缓存管理

### 查看缓存状态
```bash
$ envcli cache stats
📋 缓存统计信息

系统环境缓存:
  状态: ✓ 已缓存
  存在时间: 5.2s
  TTL 剩余: 54.8s

文件内容缓存:
  使用 --verbose 查看详细统计信息

💡 缓存说明:
  - 系统环境缓存: 60秒 TTL
  - 文件缓存: 基于文件修改时间自动失效
  - 缓存可显著提升性能 (减少 80-90% I/O)
```

### 清除缓存
```bash
# 清除文件缓存
envcli cache clear file --verbose

# 清除系统环境缓存
envcli cache clear system --verbose

# 清除所有缓存
envcli cache clear all --verbose
```

### 性能对比
```bash
# 清除缓存
envcli cache clear all

# 第一次 (冷启动)
time envcli get DB_HOST      # ~50ms

# 第二次 (热缓存)
time envcli get DB_HOST      # ~45ms
```

---

## 📚 最佳实践

### 安全最佳实践
1. **敏感数据加密**
   - 使用 SOPS 加密敏感配置
   - 密钥文件权限设置为 `600`
   - 密钥不提交到版本控制

2. **密钥管理**
   - 不硬编码密钥
   - 不提交到 Git 仓库
   - 定期轮换密钥

3. **权限控制**
   - 默认使用用户级变量
   - 系统级变量需要显式指定
   - Unix 系统限制机器级操作

### 配置管理最佳实践
1. **层级使用**
   - Local: 本地开发配置
   - Project: 团队共享配置
   - User: 个人敏感信息
   - System: 机器全局配置

2. **版本控制**
   - 提交 `project.env`
   - 忽略 `local.env` 和 `user.env`
   - 使用 `.gitignore` 管理

### 插件开发最佳实践
1. **签名验证**
   - 使用 Ed25519 签名
   - 验证插件完整性
   - 定期更新签名

2. **钩子使用**
   - PreCommand: 验证和日志
   - PostCommand: 清理和通知
   - Error: 错误处理和恢复

3. **热重载**
   - 500ms 防抖
   - 自动检测文件变化
   - 失败时回滚

### 模板使用最佳实践
1. **语法规范**
   - 使用 `{{VAR_NAME}}` 语法
   - 避免循环依赖
   - 提供默认值

2. **继承策略**
   - Local > Project > User
   - 明确覆盖关系
   - 文档化模板结构

### 团队协作最佳实践
1. **配置分层**
   - 项目配置提交到 Git
   - 个人配置保留在本地
   - 敏感信息使用加密

2. **文档化**
   - README.md 说明环境要求
   - .env.example 提供模板
   - 加密指南说明密钥管理

### 性能优化最佳实践
1. **缓存使用**
   - 依赖自动缓存
   - 必要时手动清除
   - 监控缓存命中率

2. **命令优化**
   - 批量操作使用 `list`
   - 避免重复查询
   - 使用 `run` 执行环境命令

---

## 🎯 成功标准

### 基础使用
- [ ] 能设置和获取变量
- [ ] 理解层级优先级
- [ ] 会导入导出配置
- [ ] 能运行健康检查

### 进阶功能
- [ ] 使用加密存储敏感信息
- [ ] 配置和使用插件
- [ ] 创建和使用模板
- [ ] 管理系统环境变量

### 最佳实践
- [ ] 遵循安全指南
- [ ] 正确使用层级
- [ ] 团队协作配置
- [ ] 性能优化意识

---

## 💬 获取帮助

```bash
# 查看命令帮助
envcli --help
envcli <command> --help

# 运行健康检查
envcli doctor

# 查看详细文档
# - 项目概览: project-overview.md
# - 开发指南: development-guide.md
# - 变更日志: CHANGELOG.md
```

---

**准备就绪！** 🎉

你现在可以开始使用 EnvCLI 管理环境变量了。有任何问题，请运行 `envcli doctor` 或查看详细文档。

**文档版本**: v1.0.0
**最后更新**: 2025-12-30