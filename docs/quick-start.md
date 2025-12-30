# EnvCLI 快速开始指南

**5 分钟上手环境变量管理**

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

## 🚀 5 分钟快速上手

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

### 常见工作流

#### 1. 项目配置
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

#### 2. 个人敏感信息
```bash
# 设置个人级变量（不提交到 git）
envcli set API_KEY secret_key --source=user

# 导出备份
envcli export --source=user > backup.user.env
```

#### 3. 临时开发环境
```bash
# 设置临时变量
envcli set DEBUG true --source=local

# 运行程序
envcli run DEBUG=true LOG_LEVEL=debug -- cargo run

# 清理
envcli unset DEBUG --source=local
```

---

## 💡 常用命令速查

### 核心操作
```bash
envcli get <KEY>                    # 获取变量
envcli set <KEY> <VALUE>            # 设置变量
envcli unset <KEY>                  # 删除变量
envcli list                         # 列出所有变量
```

### 系统级操作
```bash
envcli system-set <KEY> <VALUE>     # 设置系统变量
envcli system-unset <KEY>           # 删除系统变量
```

### 导入导出
```bash
envcli import <FILE>                # 导入 .env 文件
envcli export                       # 导出到 .env 格式
```

### 配置管理
```bash
envcli config validate              # 验证配置格式
envcli config init                  # 初始化配置
envcli config info                  # 显示配置信息
```

### 诊断工具
```bash
envcli doctor                       # 健康检查
envcli doctor --verbose             # 详细诊断
```

### 运行命令
```bash
envcli run KEY=value -- <COMMAND>   # 临时环境运行
envcli run --from-file .env -- <COMMAND>  # 从文件加载
```

---

## 🔧 高级功能

### 加密支持
```bash
# 加密存储敏感变量
envcli set DB_PASS secret --encrypt

# 解密查看
envcli decrypt DB_PASS

# 检查 SOPS 状态
envcli check-sops
```

### 模板系统
```bash
# 创建模板
envcli template create db --vars DB_HOST,DB_PORT,DB_USER

# 使用模板
envcli template render db --values host=localhost,port=5432
```

### 插件系统
```bash
# 列出插件
envcli plugin list

# 加载插件
envcli plugin load /path/to/plugin.so

# 启用插件
envcli plugin enable my-plugin
```

---

## 🐛 故障排除

### 问题：变量未找到
```bash
# 解决方案 1: 查看所有变量
envcli list

# 解决方案 2: 按层级搜索
envcli list --source=local

# 解决方案 3: 运行诊断
envcli doctor
```

### 问题：权限被拒绝
```bash
# 解决方案 1: 使用用户级变量
envcli set KEY value --source=user

# 解决方案 2: Windows 上以管理员运行
# 右键 PowerShell/CMD → 以管理员身份运行
```

### 问题：配置文件格式错误
```bash
# 验证配置
envcli config validate --verbose

# 修复格式：每行 KEY=VALUE
# 示例：
# DB_HOST=localhost
# DB_PORT=5432
```

### 问题：需要详细错误信息
```bash
# 使用 --verbose 标志
envcli get DB_HOST --verbose
envcli doctor --verbose
```

---

## 📚 下一步学习

- **完整用户手册**: [user-manual.md](./user-manual.md) - 所有命令详解
- **插件开发**: [plugin-development.md](./plugin-development.md) - 自定义插件
- **最佳实践**: [best-practices.md](./best-practices.md) - 使用建议

---

## 💬 获取帮助

```bash
# 查看命令帮助
envcli --help
envcli <command> --help

# 运行健康检查
envcli doctor
```

---

**准备就绪！** 🎉

你现在可以开始使用 EnvCLI 管理环境变量了。有任何问题，请运行 `envcli doctor` 或查看详细文档。
