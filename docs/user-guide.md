# EnvCLI 用户指南

> **5分钟上手环境变量管理** | **版本**: v0.1.0

---

## 📦 安装

### 1. 下载二进制文件
从 GitHub Releases 下载对应平台的二进制文件：
- Windows: `envcli.exe`
- Linux/macOS: `envcli`

### 2. 添加到 PATH

**Windows (PowerShell):**
```powershell
$env:PATH += ";C:\path\to\envcli"
```

**Linux/macOS:**
```bash
sudo mv envcli /usr/local/bin/
chmod +x /usr/local/bin/envcli
```

### 3. 验证安装
```bash
envcli --version  # 输出: envcli v0.1.0
envcli --help     # 显示所有可用命令
```

---

## 🚀 5分钟快速上手

### 第 1 分钟：设置变量
```bash
# 设置变量（默认 local 层级）
envcli set DB_HOST localhost

# 验证
envcli get DB_HOST  # 输出: localhost
```

### 第 2 分钟：查看变量
```bash
# 查看所有层级（合并视图）
envcli list

# 查看特定层级
envcli list --source=local
envcli list --source=project
envcli list --format=json
```

### 第 3 分钟：多层级管理
```bash
# 不同层级设置相同变量
envcli system-set API_KEY prod_key --scope=global
envcli set API_KEY dev_key

# 查看优先级（local 覆盖 user）
envcli get API_KEY  # 输出: dev_key (来自 local)

# 按层级查看
envcli list --source=system  # prod_key
envcli list --source=local   # dev_key
```

### 第 4 分钟：导入导出
```bash
# 导出到 .env
envcli export > .env

# 从 .env 导入
envcli import .env

# 导出特定层级
envcli export --source=project > project.env
```

### 第 5 分钟：诊断
```bash
# 健康检查
envcli doctor

# 验证配置
envcli config validate

# 查看配置信息
envcli config info
```

---

## 🎯 核心概念

### 层级系统（优先级从高到低）
| 层级 | 说明 | 文件位置 |
|------|------|----------|
| **Local** | 项目本地配置 | `./.envcli/local.env` |
| **Project** | 团队共享配置 | `./.envcli/project.env` |
| **User** | 个人全局配置 | `~/.envcli/user.env` |
| **System** | 机器全局配置 | 系统环境变量 |

**查找顺序**: `local > project > user > system`

---

## 📋 常用命令

### 核心操作
```bash
envcli get <KEY>                    # 获取变量
envcli set <KEY> <VALUE>            # 设置变量 (Local)
envcli unset <KEY>                  # 删除变量 (Local)
envcli list                         # 列出所有变量
envcli list --source=project        # 指定层级
envcli list --format=json           # JSON 输出
```

### 系统级操作
```bash
envcli system-set <KEY> <VALUE>           # 设置系统变量 (user 层)
envcli system-set <KEY> <VALUE> --scope machine  # 机器级 (需管理员)
envcli system-unset <KEY>                 # 删除系统变量
```

### 导入导出
```bash
envcli import <FILE>                # 导入到 Local
envcli import <FILE> --target=project
envcli export                       # 导出所有变量
envcli export --source=project      # 导出指定层级
envcli export --format=json         # JSON 格式
```

### 配置管理
```bash
envcli config validate              # 验证配置
envcli config init                  # 初始化配置
envcli config info                  # 显示配置信息
```

### 诊断工具
```bash
envcli doctor                       # 健康检查
envcli doctor --verbose             # 详细诊断
envcli status                       # 显示状态
```

### 运行命令
```bash
envcli run KEY=value -- <COMMAND>           # 临时环境运行
envcli run --from-file .env -- <COMMAND>    # 从文件加载
envcli run DB_HOST=localhost DB_PORT=5432 -- python app.py
```

### 加密解密
```bash
envcli set <KEY> <VALUE> --encrypt        # 加密存储
envcli encrypt <KEY> <VALUE>              # 加密并存储
envcli decrypt <KEY>                      # 解密查看
envcli check-sops                         # 检查 SOPS 状态
```

### 模板系统
```bash
envcli template create <NAME> --vars VAR1 VAR2  # 创建模板
envcli template list                      # 列出模板
envcli template show <NAME>               # 查看模板
envcli template render <NAME> --var VAR1=value -o output.env
envcli template delete <NAME>             # 删除模板
```

### 插件系统
```bash
envcli plugin list                       # 列出插件
envcli plugin load <PATH>                # 加载插件
envcli plugin enable <PLUGIN_ID>         # 启用插件
envcli plugin disable <PLUGIN_ID>        # 禁用插件
envcli plugin reload <PLUGIN_ID>         # 热重载
envcli plugin status                     # 插件状态
envcli plugin generate-key-pair          # 生成签名密钥
envcli plugin sign <PLUGIN_ID> --key <KEY> --output sig.json
envcli plugin verify <PLUGIN_ID>         # 验证签名
envcli plugin config set <PLUGIN_ID> <KEY> <VALUE>  # 配置插件
```

### 缓存管理
```bash
envcli cache stats                      # 查看缓存统计
envcli cache clear file                 # 清除文件缓存
envcli cache clear system               # 清除系统缓存
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
choco install sops  # 或 scoop install sops
```

2. **配置加密后端（推荐 Age）**
```bash
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
```

### 配置示例
```yaml
# .sops.yaml
creation_rules:
  - path_regex: secrets\\.env$
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
| 钩子 | 触发时机 | 用途 |
|------|----------|------|
| **PreCommand** | 命令执行前 | 日志、验证、环境准备 |
| **PostCommand** | 命令执行后 | 清理、通知、结果处理 |
| **Error** | 发生错误时 | 错误报告、恢复 |
| **PreSet** | 设置变量前 | 数据验证、转换 |
| **PostGet** | 获取变量后 | 数据解密、转换 |

### 开发插件（Rust 动态库）

**Cargo.toml:**
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

**插件代码:**
```rust
use envcli::plugin::*;

#[derive(Clone)]
struct HelloPlugin {
    metadata: PluginMetadata,
    config: PluginConfig,
}

impl Plugin for HelloPlugin {
    fn metadata(&self) -> &PluginMetadata { &self.metadata }

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

**编译和加载:**
```bash
cargo build --release
envcli plugin load target/release/libhello_plugin.so
envcli plugin enable hello-plugin
```

---

## 💡 常见工作流

### 项目配置
```bash
# 初始化项目
envcli config init

# 设置项目级变量
envcli set DB_HOST localhost --source=project
envcli set DB_PORT 5432 --source=project

# 提交到版本控制
git add .envcli/project.env
git commit -m "Add project env vars"
```

### 个人敏感信息
```bash
# 设置个人级变量（不提交到 git）
envcli set API_KEY secret_key --source=user

# 导出备份
envcli export --source=user > backup.user.env
```

### 临时开发环境
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
envcli list                    # 查看所有变量
envcli list --source=local     # 按层级搜索
envcli doctor                  # 运行诊断
```

### 权限被拒绝
```bash
# 使用用户级变量（无需管理员权限）
envcli set KEY value --source=user

# Windows 需要管理员权限时：
# 右键 PowerShell/CMD → 以管理员身份运行
```

### 配置文件格式错误
```bash
envcli config validate --verbose  # 验证配置

# 修复格式：每行 KEY=VALUE
# DB_HOST=localhost
# DB_PORT=5432
```

### 需要详细错误信息
```bash
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

💡 缓存说明:
  - 系统环境缓存: 60秒 TTL
  - 文件缓存: 基于文件修改时间自动失效
  - 缓存可显著提升性能 (减少 80-90% I/O)
```

### 清除缓存
```bash
envcli cache clear file     # 清除文件缓存
envcli cache clear system   # 清除系统缓存
envcli cache clear all      # 清除所有缓存
```

---

## 📚 最佳实践

### 安全
- 敏感数据使用 SOPS 加密
- 密钥文件权限设为 `600`
- 密钥不提交到版本控制
- 默认使用用户级变量（无需管理员权限）

### 配置管理
- **Local**: 本地开发配置（不提交）
- **Project**: 团队共享配置（提交）
- **User**: 个人敏感信息（不提交）
- **System**: 机器全局配置（不适用）

### 版本控制
```bash
# .gitignore
.envcli/local.env
.envcli/user.env

# 但保留
!.envcli/project.env
```

### 插件开发
- 使用 Ed25519 签名验证插件完整性
- PreCommand: 验证和日志
- PostCommand: 清理和通知
- Error: 错误处理和恢复
- 热重载：500ms 防抖

### 模板系统
- 语法：`{{VAR_NAME}}`
- 默认值：`{{API_URL|http://localhost:3000}}`
- 避免循环依赖
- 明确继承关系：Local > Project > User

### 团队协作
1. 提交 `project.env` 到 Git
2. 忽略 `local.env` 和 `user.env`
3. 使用 `.env.example` 作为模板
4. 敏感信息使用加密
5. README.md 说明环境要求

### 性能优化
- 依赖自动缓存（60秒 TTL）
- 必要时手动清除缓存
- 批量操作使用 `list`
- 使用 `run` 执行环境命令

---

## 💬 获取帮助

```bash
envcli --help                    # 查看所有命令
envcli <command> --help          # 查看特定命令帮助
envcli doctor                    # 运行健康检查
```

---

**准备就绪！** 🎉

你现在可以开始使用 EnvCLI 管理环境变量了。有任何问题，请运行 `envcli doctor` 或查看详细文档。

**文档版本**: v1.0.0
**最后更新**: 2025-12-31
