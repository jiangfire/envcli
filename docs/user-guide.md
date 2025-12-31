# EnvCLI 用户指南

> **5分钟上手环境变量管理** | **版本**: v0.1.0

---

## 📦 安装

### 下载二进制文件
从 GitHub Releases 下载对应平台的二进制文件：
- Windows: `envcli.exe`
- Linux/macOS: `envcli`

### 添加到 PATH

**Windows (PowerShell):**
```powershell
$env:PATH += ";C:\path\to\envcli"
```

**Linux/macOS:**
```bash
sudo mv envcli /usr/local/bin/
chmod +x /usr/local/bin/envcli
```

### 验证安装
```bash
envcli --version  # 输出: envcli v0.1.0
envcli --help     # 显示所有可用命令
```

---

## 🚀 5分钟快速上手

### 第 1 分钟：设置变量
```bash
envcli set DB_HOST localhost
envcli get DB_HOST  # 输出: localhost
```

### 第 2 分钟：查看变量
```bash
envcli list                    # 所有层级（合并）
envcli list --source=local     # 指定层级
envcli list --format=json      # JSON 输出
```

### 第 3 分钟：多层级管理
```bash
# 不同层级设置相同变量
envcli system-set API_KEY prod_key --scope=global
envcli set API_KEY dev_key

# 查看优先级（local 覆盖 user）
envcli get API_KEY  # 输出: dev_key (来自 local)
```

### 第 4 分钟：导入导出
```bash
envcli export > .env           # 导出所有变量
envcli import .env             # 导入到 Local
envcli export --source=project > project.env  # 导出指定层级
```

### 第 5 分钟：诊断
```bash
envcli doctor                  # 健康检查
envcli config validate         # 验证配置
envcli config info             # 查看配置信息
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

## 📋 常用命令速查

### 核心操作
```bash
envcli get <KEY>                    # 获取变量
envcli set <KEY> <VALUE>            # 设置变量 (Local)
envcli unset <KEY>                  # 删除变量
envcli list                         # 列出变量
envcli list --source=<layer>        # 指定层级
envcli list --format=json           # JSON 输出
```

### 系统级操作
```bash
envcli system-set <KEY> <VALUE>           # 设置系统变量
envcli system-set <KEY> <VALUE> --scope machine  # 机器级 (需管理员)
envcli system-unset <KEY>                 # 删除系统变量
```

### 导入导出
```bash
envcli import <FILE>                # 导入到 Local
envcli import <FILE> --target=project
envcli export                       # 导出所有变量
envcli export --source=project      # 导出指定层级
```

### 加密解密
```bash
envcli set <KEY> <VALUE> --encrypt        # 加密存储
envcli decrypt <KEY>                      # 解密查看
envcli check-sops                         # 检查 SOPS 状态
```

### 模板系统
```bash
envcli template create <NAME> --vars VAR1 VAR2
envcli template list
envcli template render <NAME> --var VAR1=value -o output.env
```

### 插件系统
```bash
envcli plugin list
envcli plugin load <PATH>
envcli plugin enable <PLUGIN_ID>
envcli plugin reload <PLUGIN_ID>
envcli plugin generate-key-pair
envcli plugin verify <PLUGIN_ID>
```

### 缓存管理
```bash
envcli cache stats
envcli cache clear all
```

---

## 🔐 加密存储

### 安装 SOPS
```bash
# macOS
brew install sops

# Linux
wget https://github.com/mozilla/sops/releases/download/v3.8.1/sops_3.8.1_amd64.deb
sudo dpkg -i sops_3.8.1_amd64.deb

# Windows
choco install sops
```

### 配置 Age 密钥
```bash
age-keygen -o ~/.config/sops/age/keys.txt
age-keygen -y ~/.config/sops/age/keys.txt  # 获取公钥
```

### 使用加密
```bash
envcli set DB_PASS secret --encrypt
envcli decrypt DB_PASS
envcli check-sops
```

---

## 💡 常见工作流

### 项目配置
```bash
envcli config init
envcli set DB_HOST localhost --source=project
git add .envcli/project.env
git commit -m "Add project env vars"
```

### 个人敏感信息
```bash
envcli set API_KEY secret_key --source=user
envcli export --source=user > backup.user.env
```

### 临时开发环境
```bash
envcli set DEBUG true --source=local
envcli run DEBUG=true -- cargo run
envcli unset DEBUG --source=local
```

---

## 🐛 故障排除

```bash
# 变量未找到
envcli list
envcli doctor

# 权限被拒绝
envcli set KEY value --source=user  # 使用用户级变量

# 需要详细错误
envcli get DB_HOST --verbose
envcli doctor --verbose
```

---

## 📚 最佳实践

### 版本控制
```bash
# .gitignore
.envcli/local.env
.envcli/user.env

# 但保留
!.envcli/project.env
```

### 配置分层
- **Local**: 本地开发（不提交）
- **Project**: 团队共享（提交）
- **User**: 个人敏感（不提交）
- **System**: 机器全局（不适用）

### 安全
- 敏感数据使用 SOPS 加密
- 密钥权限设为 `600`
- 密钥不提交到版本控制

---

## 💬 获取帮助

```bash
envcli --help
envcli <command> --help
envcli doctor
```

---

**准备就绪！** 🎉

运行 `envcli doctor` 进行健康检查，或查看 [development-guide.md](development-guide.md) 获取插件开发和高级用法。

**文档版本**: v1.0.0
**最后更新**: 2025-12-31
