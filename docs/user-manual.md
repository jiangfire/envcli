# EnvCLI 完整用户手册

**环境变量管理的终极指南**

---

## 📖 目录

1. [概述](#概述)
2. [安装配置](#安装配置)
3. [核心命令](#核心命令)
4. [层级系统](#层级系统)
5. [高级功能](#高级功能)
6. [配置管理](#配置管理)
7. [插件系统](#插件系统)
8. [故障排除](#故障排除)
9. [最佳实践](#最佳实践)

---

## 概述

EnvCLI 是一个跨平台的环境变量管理工具，支持多层级配置、格式转换、加密存储和插件扩展。

### 核心特性

- ✅ **多层级管理**: System/User/Project/Local 四级优先级
- ✅ **格式转换**: 支持 .env, JSON, YAML 等格式
- ✅ **加密支持**: 集成 SOPS 进行加密存储
- ✅ **插件系统**: 可扩展的插件架构
- ✅ **模板系统**: 变量模板和继承
- ✅ **健康诊断**: 内置诊断工具
- ✅ **跨平台**: Windows, Linux, macOS 全支持

---

## 安装配置

### 系统要求

- **Windows**: 10/11 (x64)
- **Linux**: glibc 2.28+ (Ubuntu 18.04+, CentOS 8+)
- **macOS**: 10.15+ (Catalina)

### 安装方法

#### 方法 1: 二进制下载（推荐）

```bash
# Windows
# 下载 envcli.exe 并添加到 PATH

# Linux/macOS
curl -L https://github.com/your-repo/envcli/releases/latest/download/envcli -o envcli
chmod +x envcli
sudo mv envcli /usr/local/bin/
```

#### 方法 2: 包管理器

```bash
# Windows (Scoop)
scoop install envcli

# macOS (Homebrew)
brew install envcli

# Linux (curl)
bash -c "$(curl -fsSL https://envcli.dev/install.sh)"
```

#### 方法 3: 从源码编译

```bash
git clone https://github.com/your-repo/envcli.git
cd envcli
cargo build --release
# 二进制文件在 target/release/envcli
```

### 首次运行配置

```bash
# 1. 验证安装
envcli --version

# 2. 初始化配置目录
envcli config init

# 3. 运行健康检查
envcli doctor

# 4. 设置你的第一个变量
envcli set MY_VAR "Hello World"
```

---

## 核心命令

### 1. 获取变量 `envcli get`

**语法**：
```bash
envcli get <KEY> [选项]
```

**选项**：
- `--verbose, -v`: 详细错误信息

**示例**：
```bash
# 基本使用
envcli get DB_HOST

# 详细错误
envcli get DB_HOST --verbose
```

**行为**：
- 按优先级查找变量（local → project → user → system）
- 如果未找到，返回错误
- 使用 `--verbose` 获取详细错误和建议

---

### 2. 设置变量 `envcli set`

**语法**：
```bash
envcli set <KEY> <VALUE> [选项]
```

**选项**：
- `--source, -s`: 指定层级（默认: local）

**示例**：
```bash
# 设置本地变量
envcli set DB_HOST localhost

# 设置项目级变量
envcli set DB_HOST localhost --source=project

# 设置用户级变量
envcli set API_KEY my_secret --source=user
```

**注意**：
- `system` 层级需要管理员权限
- 建议使用 `envcli system-set` 设置系统变量

---

### 3. 删除变量 `envcli unset`

**语法**：
```bash
envcli unset <KEY> [选项]
```

**选项**：
- `--source, -s`: 指定层级（默认: local）

**示例**：
```bash
# 删除本地变量
envcli unset DB_HOST

# 删除项目级变量
envcli unset DB_HOST --source=project
```

---

### 4. 列出变量 `envcli list`

**语法**：
```bash
envcli list [选项]
```

**选项**：
- `--source, -s`: 指定层级（不指定则合并所有）
- `--format, -f`: 输出格式（env/json，默认: env）

**示例**：
```bash
# 列出所有变量（合并视图）
envcli list

# 列出本地变量
envcli list --source=local

# JSON 格式输出
envcli list --format=json

# 项目级变量 JSON
envcli list --source=project --format=json
```

**输出示例**：
```
DB_HOST=localhost
DB_PORT=5432
API_KEY=secret (来自 user)
```

---

### 5. 系统级操作

#### 设置系统变量
```bash
envcli system-set <KEY> <VALUE> [选项]
```

**选项**：
- `--scope`: 作用域（global/machine，默认: global）

**示例**：
```bash
# 用户级系统变量
envcli system-set JAVA_HOME "C:\Program Files\Java"

# 机器级系统变量（需要管理员）
envcli system-set PATH "C:\MyTools" --scope=machine
```

#### 删除系统变量
```bash
envcli system-unset <KEY> [选项]
```

**选项**：
- `--scope`: 作用域（global/machine，默认: global）

---

### 6. 导入导出

#### 导入 `envcli import`
```bash
envcli import <FILE> [选项]
```

**选项**：
- `--target, -t`: 目标层级（user/project/local，默认: local）

**示例**：
```bash
# 导入到本地
envcli import .env

# 导入到项目级
envcli import config.env --target=project
```

#### 导出 `envcli export`
```bash
envcli export [选项]
```

**选项**：
- `--source, -s`: 来源层级（不指定则合并所有）
- `--format, -f`: 输出格式（env/json，默认: env）

**示例**：
```bash
# 导出所有变量
envcli export > backup.env

# 导出项目级变量（JSON）
envcli export --source=project --format=json > project.json
```

---

### 7. 运行命令 `envcli run`

**语法**：
```bash
envcli run [选项] -- <COMMAND> [ARGS...]
```

**选项**：
- `--env, -e`: 临时环境变量（KEY=VALUE）
- `--from-file`: 从 .env 文件加载

**示例**：
```bash
# 临时变量运行
envcli run DB_HOST=localhost DB_PORT=5432 -- python app.py

# 从文件加载
envcli run --from-file .env.local -- npm start

# 混合使用
envcli run API_KEY=secret --from-file .env -- cargo run
```

---

## 层级系统

### 四级优先级

```
┌─────────────────────────────────────┐
│  1. Local (最高优先级)              │
│     ./ .envcli/local.env            │
├─────────────────────────────────────┤
│  2. Project                         │
│     ./ .envcli/project.env          │
├─────────────────────────────────────┤
│  3. User                            │
│     ~/.envcli/user.env              │
├─────────────────────────────────────┤
│  4. System (最低优先级)             │
│     操作系统环境变量                │
└─────────────────────────────────────┘
```

### 优先级规则

**变量查找顺序**：
1. 首先检查 local 层级
2. 如果不存在，检查 project 层级
3. 如果不存在，检查 user 层级
4. 如果不存在，检查 system 层级
5. 如果都不存在，返回错误

**变量覆盖**：
```
local > project > user > system
```

### 使用场景

| 层级 | 适用场景 | 示例 | Git 提交 |
|------|----------|------|----------|
| **Local** | 个人开发配置 | `DEBUG=true` | ❌ 不提交 |
| **Project** | 团队共享配置 | `DB_HOST=localhost` | ✅ 提交 |
| **User** | 个人全局配置 | `GITHUB_TOKEN` | ❌ 不提交 |
| **System** | 机器全局配置 | `JAVA_HOME` | ❌ 不提交 |

---

## 高级功能

### 1. 加密存储

EnvCLI 集成 SOPS 进行加密存储。

#### 检查 SOPS 状态
```bash
envcli check-sops
```

#### 加密设置
```bash
# 设置并加密变量
envcli set DB_PASS secret_password --encrypt

# 使用专用命令
envcli encrypt DB_PASS secret_password
envcli encrypt API_KEY key --target=project
```

#### 解密查看
```bash
# 解密查看
envcli decrypt DB_PASS

# 指定来源
envcli decrypt API_KEY --source=project
```

**加密文件格式**：
```
DB_PASS=encrypted:<encrypted_value>
API_KEY=encrypted:<encrypted_value>
```

---

### 2. 模板系统

#### 创建模板
```bash
envcli template create <NAME> --vars <VAR1>,<VAR2> [选项]
```

**选项**：
- `--inherits, -i`: 继承的父模板
- `--vars, -s`: 变量列表

**示例**：
```bash
# 创建基础数据库模板
envcli template create db --vars DB_HOST,DB_PORT,DB_USER,DB_PASS

# 创建继承模板
envcli template create web --inherits db --vars APP_ENV,API_URL
```

#### 列出模板
```bash
envcli template list
envcli template list --verbose
```

#### 渲染模板
```bash
envcli template render <NAME> --values <KEY>=<VALUE>,...
```

**示例**：
```bash
# 渲染数据库模板
envcli template render db --values host=localhost,port=5432,user=admin,pass=secret

# 输出：
# DB_HOST=localhost
# DB_PORT=5432
# DB_USER=admin
# DB_PASS=secret
```

---

### 3. 插件系统

#### 插件管理
```bash
# 列出插件
envcli plugin list
envcli plugin list --verbose

# 加载插件
envcli plugin load /path/to/plugin.so

# 启用/禁用
envcli plugin enable <plugin-id>
envcli plugin disable <plugin-id>

# 卸载
envcli plugin uninstall <plugin-id>
```

#### 插件配置
```bash
# 设置配置
envcli plugin config set my-plugin timeout 60

# 获取配置
envcli plugin config get my-plugin
envcli plugin config get my-plugin timeout

# 重置配置
envcli plugin config reset my-plugin
```

#### 插件审计
```bash
# 安全检查
envcli plugin audit
envcli plugin audit --verbose
```

---

## 配置管理

### 1. 配置验证

```bash
envcli config validate [选项]
```

**选项**：
- `--verbose, -v`: 显示详细信息

**功能**：
- ✅ 检查所有层级文件
- ✅ 验证格式（KEY=VALUE）
- ✅ 检测空文件
- ✅ 识别格式错误

**示例**：
```bash
# 基础验证
envcli config validate

# 详细验证（显示所有变量）
envcli config validate --verbose
```

---

### 2. 配置初始化

```bash
envcli config init [选项]
```

**选项**：
- `--force, -f`: 强制重新初始化

**功能**：
- ✅ 创建配置目录
- ✅ 初始化各层级文件
- ✅ 添加格式说明注释

**示例**：
```bash
# 首次初始化
envcli config init

# 重新初始化（覆盖现有）
envcli config init --force
```

---

### 3. 配置信息

```bash
envcli config info
```

**显示**：
- 配置目录路径
- 各层级文件状态（大小、行数）
- 系统平台信息
- 当前工作目录

---

## 健康诊断

### Doctor 命令

```bash
envcli doctor [选项]
```

**选项**：
- `--verbose, -v`: 详细诊断

**诊断项目**：

1. **📁 配置目录检查**
   - 目录存在性
   - 权限检查

2. **📄 配置文件状态**
   - 文件存在性
   - 格式验证
   - 空文件检测

3. **🔄 变量冲突检查**
   - 多层定义检测
   - 优先级分析

4. **🖥️ 系统环境变量**
   - 变量数量统计
   - 关键变量检查

5. **🔌 插件系统状态**
   - 插件加载状态
   - 执行统计

6. **🔧 运行环境**
   - 工作目录
   - PATH 统计

**示例**：
```bash
# 基础诊断
envcli doctor

# 详细诊断
envcli doctor --verbose
```

---

## 故障排除

### 常见问题

#### 1. 变量未找到
```bash
# 错误
❌ 错误: 变量未找到: DB_HOST

# 解决方案
envcli list                    # 查看所有变量
envcli list --source=local     # 查看特定层级
envcli doctor                  # 运行诊断
```

#### 2. 权限被拒绝
```bash
# 错误
❌ 错误: 权限被拒绝: 系统环境变量层只读

# 解决方案
envcli set KEY value --source=user     # 使用用户级变量
# 或以管理员运行（Windows）
```

#### 3. 配置文件格式错误
```bash
# 验证配置
envcli config validate --verbose

# 正确格式：
# KEY=VALUE
# # 注释
# (空行)
```

#### 4. 需要详细错误信息
```bash
# 使用 --verbose
envcli get DB_HOST --verbose
envcli doctor --verbose
envcli config validate --verbose
```

---

## 最佳实践

### 1. 项目配置管理

```bash
# 1. 初始化项目配置
envcli config init

# 2. 设置项目共享变量
envcli set DB_HOST localhost --source=project
envcli set DB_PORT 5432 --source=project

# 3. 提交到版本控制
git add .envcli/project.env
git commit -m "Add project environment variables"

# 4. 添加 .gitignore
echo ".envcli/local.env" >> .gitignore
echo ".envcli/user.env" >> .gitignore
```

### 2. 敏感信息管理

```bash
# 1. 使用加密存储
envcli set DB_PASS secret --encrypt

# 2. 或使用用户级变量
envcli set API_KEY my_secret --source=user

# 3. 导出备份
envcli export --source=user > backup.user.env
```

### 3. 开发工作流

```bash
# 1. 设置开发环境
envcli set DEBUG true --source=local
envcli set LOG_LEVEL debug --source=local

# 2. 运行应用
envcli run DEBUG=true -- cargo run

# 3. 测试生产配置
envcli run --from-file .env.production -- cargo run

# 4. 清理
envcli unset DEBUG --source=local
```

### 4. 团队协作

```bash
# 1. 创建团队模板
envcli template create team-db --vars DB_HOST,DB_PORT,DB_USER

# 2. 文档化变量
# 在 README.md 中说明：
# ```bash
# envcli set DB_HOST localhost --source=project
# envcli set DB_PORT 5432 --source=project
# ```

# 3. 使用 CI/CD
# 在 CI 中设置变量
envcli system-set CI_TOKEN "$TOKEN" --scope=global
```

### 5. 安全建议

- ❌ **不要**提交包含敏感信息的文件到 Git
- ✅ **应该**使用加密或用户级变量
- ❌ **不要**在命令行中直接显示密码
- ✅ **应该**使用交互式输入或文件
- ❌ **不要**共享 user.env 文件
- ✅ **应该**使用 project.env + 加密

---

## 命令参考速查

### 核心操作
| 命令 | 说明 | 示例 |
|------|------|------|
| `envcli get <KEY>` | 获取变量 | `envcli get DB_HOST` |
| `envcli set <KEY> <VAL>` | 设置变量 | `envcli set DB_HOST localhost` |
| `envcli unset <KEY>` | 删除变量 | `envcli unset DB_HOST` |
| `envcli list` | 列出变量 | `envcli list --source=project` |

### 系统级
| 命令 | 说明 | 示例 |
|------|------|------|
| `envcli system-set` | 设置系统变量 | `envcli system-set JAVA_HOME "C:\Java"` |
| `envcli system-unset` | 删除系统变量 | `envcli system-unset JAVA_HOME` |

### 导入导出
| 命令 | 说明 | 示例 |
|------|------|------|
| `envcli import` | 导入文件 | `envcli import .env` |
| `envcli export` | 导出变量 | `envcli export > backup.env` |

### 加密
| 命令 | 说明 | 示例 |
|------|------|------|
| `envcli encrypt` | 加密变量 | `envcli encrypt DB_PASS secret` |
| `envcli decrypt` | 解密变量 | `envcli decrypt DB_PASS` |
| `envcli check-sops` | 检查状态 | `envcli check-sops` |

### 配置管理
| 命令 | 说明 | 示例 |
|------|------|------|
| `envcli config validate` | 验证配置 | `envcli config validate --verbose` |
| `envcli config init` | 初始化配置 | `envcli config init` |
| `envcli config info` | 显示信息 | `envcli config info` |

### 模板
| 命令 | 说明 | 示例 |
|------|------|------|
| `envcli template create` | 创建模板 | `envcli template create db --vars DB_HOST,DB_PORT` |
| `envcli template list` | 列出模板 | `envcli template list` |
| `envcli template render` | 渲染模板 | `envcli template render db --values host=localhost` |

### 插件
| 命令 | 说明 | 示例 |
|------|------|------|
| `envcli plugin list` | 列出插件 | `envcli plugin list --verbose` |
| `envcli plugin load` | 加载插件 | `envcli plugin load /path/to/plugin.so` |
| `envcli plugin enable` | 启用插件 | `envcli plugin enable my-plugin` |

### 诊断
| 命令 | 说明 | 示例 |
|------|------|------|
| `envcli doctor` | 健康检查 | `envcli doctor --verbose` |
| `envcli status` | 显示状态 | `envcli status` |

### 运行
| 命令 | 说明 | 示例 |
|------|------|------|
| `envcli run` | 临时环境运行 | `envcli run KEY=val -- command` |

---

## 配置文件格式

### .env 格式（默认）

```
# 注释以 # 开头
DB_HOST=localhost
DB_PORT=5432
DB_USER=admin
DB_PASS=secret

# 空行会被忽略

API_URL=https://api.example.com
```

### JSON 格式

```json
{
  "DB_HOST": "localhost",
  "DB_PORT": "5432",
  "DB_USER": "admin",
  "DB_PASS": "secret",
  "API_URL": "https://api.example.com"
}
```

### 加密格式

```
DB_HOST=localhost
DB_PASS=encrypted:<base64_encoded_encrypted_value>
API_KEY=encrypted:<base64_encoded_encrypted_value>
```

---

## 环境变量参考

EnvCLI 自身使用的环境变量：

| 变量名 | 说明 | 默认值 |
|--------|------|--------|
| `ENVCLI_CONFIG_DIR` | 配置目录路径 | `~/.envcli` |
| `ENVCLI_VERBOSE` | 默认详细模式 | `false` |
| `ENVCLI_SOPS_PATH` | SOPS 可执行文件路径 | `sops` |

---

## 获取帮助

```bash
# 查看所有命令
envcli --help

# 查看特定命令帮助
envcli get --help
envcli set --help
envcli doctor --help

# 查看版本
envcli --version
```

---

## 相关资源

- **快速开始**: [quick-start.md](./quick-start.md) - 5分钟上手
- **插件开发**: [plugin-development.md](./plugin-development.md) - 自定义插件
- **API 文档**: [api.md](./api.md) - Rust API 参考
- **GitHub**: https://github.com/your-repo/envcli

---

**文档版本**: v0.1.0
**最后更新**: 2025-12-30
**维护者**: EnvCLI Team
