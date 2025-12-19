# EnvCLI - 跨平台环境变量管理工具

<p align="center">
  <img src="https://img.shields.io/badge/Rust-1.75+-blue?logo=rust" alt="Rust Version" />
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey" alt="Platforms" />
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="License" />
</p>

> 🚀 **12-factor 应用风格**的跨平台环境变量管理工具
> ✨ **安静默认原则**：成功无输出，错误才显示
> 🔒 **四层架构**：系统 → 用户 → 项目 → 本地（优先级递增）

---

## ✨ 核心特性

### 1. **四层架构设计**
```
┌─────────────────────────────────────┐
│  CLI 临时变量  (最高优先级)          │
├─────────────────────────────────────┤
│  本地层  ./.envcli/local.env        │  ← .gitignore
├─────────────────────────────────────┤
│  项目层  ./.envcli/project.env      │
├─────────────────────────────────────┤
│  用户层  ~/.envcli/user.env         │
├─────────────────────────────────────┤
│  系统环境变量 (最低优先级)          │
└─────────────────────────────────────┘
```

### 2. **运行时环境注入**
```bash
# 临时变量直接运行程序
envcli run DB_HOST=localhost API_KEY=secret -- python app.py

# 从 .env 文件加载临时变量
envcli run --from-file .env.production -- npm start

# 组合使用
envcli run DB_HOST=localhost --from-file .env.dev -- cargo run
```

### 3. **跨平台支持**
- ✅ Windows（CMD/PowerShell 兼容）
- ✅ Linux (Ubuntu, CentOS, etc.)
- ✅ macOS

### 4. **格式转换与导出**
```bash
# 导出为 .env 格式
envcli export > backup.env

# 导出为 JSON
envcli export --format=json

# 仅导出特定层级
envcli export --source=project --format=json
```

---

## 🚀 快速开始

### 安装

先决条件：Rust 1.75+

```bash
# 1. 克隆项目
git clone https://github.com/your-repo/envcli.git
cd envcli

# 2. 编译并安装
cargo build --release

# 3. (可选) 添加到 PATH
# Windows: 复制 target/release/envcli.exe 到 PATH 目录
# Linux/macOS: sudo cp target/release/envcli /usr/local/bin/
```

### 首次运行

```bash
# 检查状态（会自动创建配置目录）
envcli status

# 诊断系统
envcli doctor
```

---

## 📖 使用指南

### 1. 基础读取操作

```bash
# 获取变量（不存在会报错）
envcli get DB_HOST
# 输出: localhost

# 详细错误信息（--verbose）
envcli get NONEXISTENT --verbose
# 输出: ❌ 变量 NONEXISTENT 不存在
```

### 2. 变量写入与管理

```bash
# 设置变量（写入本地层）
envcli set DB_HOST localhost
envcli set DB_PORT 5432

# 删除变量（从本地层）
envcli unset DB_HOST

# 详细删除
envcli unset DB_HOST --verbose
# 输出: ✓ 已删除
```

### 3. 列出所有变量

```bash
# 列出合并后的所有变量
envcli list

# 按 .env 格式输出
envcli list --format=env

# JSON 格式输出
envcli list --format=json

# 仅列出特定层级
envcli list --source=project
envcli list --source=local

# 查看系统变量（可能很多）
envcli list --source=system
```

### 4. 导入导出

```bash
# 导入 .env 文件到本地层
envcli import .env

# 导入到项目层
envcli import config.env --target=project

# 导出所有层级
envcli export > backup.env

# 导出项目层级为 JSON
envcli export --source=project --format=json > project.json
```

### 5. 状态检查

```bash
# 查看当前状态
envcli status
# 输出示例:
# 配置目录: C:\Users\用户名\.envcli
#   用户层/.../user.env: 存在 [2 个变量]
#   项目层/.../project.env: 不存在 [0 个变量]
#   本地层/.../local.env: 存在 [3 个变量]
#
# 合并后总计: 5 个变量

# 详细状态（显示所有变量）
envcli status --verbose
```

### 6. 问题诊断

```bash
# 诊断常见问题
envcli doctor

# 详细诊断
envcli doctor --verbose
```

---

## 🎯 12-factor 风格运行

### 场景 1：开发环境快速调试

```bash
# 临时设置调试变量
envcli run DEBUG=true LOG_LEVEL=trace -- python app.py

# 等价于：
# DEBUG=true LOG_LEVEL=trace python app.py
# 但在 Windows 上也能工作！
```

### 场景 2：使用环境文件

```bash
# .env.production 文件内容:
# API_URL=https://api.example.com
# DB_HOST=prod-db.example.com
# API_KEY=secret-production-key

# 运行生产环境应用
envcli run --from-file .env.production -- npm start
```

### 场景 3：混合模式（文件 + 临时覆盖）

```bash
# 基础配置来自文件，运行时临时覆盖
envcli run --from-file .env.base DB_HOST=localhost -- python app.py
# 结果：DB_HOST 将使用 localhost 而非文件中的值
```

### 场景 4：优先级演示

```bash
# 假设各层级都有 DB_PORT 定义：
# 系统: DB_PORT=5432
# 用户: DB_PORT=5433
# 项目: DB_PORT=5434
# 本地: DB_PORT=5435
# 临时: DB_PORT=5436

envcli run DB_PORT=5436 -- echo \$DB_PORT
# 输出: 5436  ← 临时变量优先级最高

# 不加临时变量
envcli run -- echo \$DB_PORT
# 输出: 5435  ← 本地层优先级最高
```

---

## 📂 文件结构

### 配置目录位置

- **Windows**: `C:\Users\<用户名>\.envcli\`
- **Linux**: `/home/<用户名>/.envcli/`
- **macOS**: `/Users/<用户名>/.envcli/`

### 层级文件

```
.envcli/
├── user.env       # 用户级（影响所有项目）
└── project.env    # 项目级（在项目根目录运行）
```

**本地层文件**: `<项目目录>/.envcli/local.env`

**注意**:
- `local.env` 默认在 `.gitignore` 中，不会被提交
- 本地层仅在当前工作目录存在时生效

---

## 🔧 命令参考

### 全局选项

| 选项 | 说明 |
|------|------|
| `--verbose` | 详细输出模式（显示成功信息） |
| `--config-dir <路径>` | 自定义配置目录 |

### 命令列表

| 命令 | 说明 | 示例 |
|------|------|-------|
| `get <KEY>` | 获取变量值 | `envcli get DB_HOST` |
| `set <KEY> <VALUE>` | 设置变量 | `envcli set DB_HOST localhost` |
| `unset <KEY>` | 删除变量 | `envcli unset DB_HOST` |
| `list` | 列出变量 | `envcli list --format=json` |
| `import <FILE>` | 导入文件 | `envcli import .env --target=project` |
| `export` | 导出变量 | `envcli export > backup.env` |
| `status` | 显示状态 | `envcli status --verbose` |
| `doctor` | 诊断问题 | `envcli doctor` |
| `run` | 运行程序 | `envcli run KEY=val -- npm start` |

### run 命令选项

| 选项 | 说明 | 示例 |
|------|------|-------|
| `-e, --env <KEY=VALUE>` | 临时环境变量（可多次） | `-e DB_HOST=localhost -e PORT=8080` |
| `-f, --from-file <FILE>` | 从 .env 文件加载 | `--from-file .env.production` |
| `<COMMAND>` | 要执行的命令 | `-- python app.py` |

---

## 🔍 实际案例

### 案例 1：多环境数据库配置

```bash
# 1. 设置用户级默认值
envcli set DB_HOST db.default.com
envcli set DB_PORT 5432

# 2. 项目特定配置
envcli set DB_HOST localhost --target=project

# 3. 本地开发覆盖
envcli set DB_PORT 5433

# 4. 查看最终配置
envcli list --verbose
# 输出:
# DB_HOST = localhost (来自 local)
# DB_PORT = 5433 (来自 local)

# 5. 运行应用（使用当前配置）
envcli run -- cargo run

# 6. 或临时覆盖
envcli run DB_HOST=127.0.0.1 -- cargo run
```

### 案例 2：CI/CD 集成

```bash
# !/bin/bash
# deploy.sh

# 导入生产配置
envcli import production.env --target=project

# 运行健康检查
envcli run --from-file production.env -- ./health-check.sh

# 如果检查通过，导出配置
envcli export --source=project --format=json > config.json
```

### 案例 3：团队协作

```
项目目录/
├── .envcli/
│   └── project.env      # 团队公共配置（提交到 git）
├── .envcli/
│   └── local.env        # 个人配置（.gitignore）
└── README.md
```

**team.env** (提交到 git):
```
DB_HOST=team-db.example.com
API_URL=https://api.example.com
```

**local.env** (.gitignore):
```
DB_HOST=localhost
API_KEY=secret-local
```

**使用**:
```bash
# 新成员克隆项目后
envcli import team.env --target=project

# 添加个人配置
envcli set API_KEY my-secret-key

# 运行
envcli run -- cargo run  # 使用合并后配置
```

---

## ⚙️ 高级配置

### 自定义配置目录

```bash
# 使用自定义目录（适用于便携式安装）
envcli --config-dir /path/to/custom/config status

# 或使用环境变量（如果程序支持）
export ENVCLI_CONFIG_DIR=/path/to/config
```

### 环境变量优先级调试

```bash
# 查看完整优先级链
envcli status --verbose

# 诊断重复定义
envcli doctor
```

---

## 🔒 安全最佳实践

### 1. 不提交敏感信息

```bash
# 确保 .envcli/local.env 在 .gitignore 中
echo ".envcli/local.env" >> .gitignore

# 检查是否已忽略
envcli status  # 本地层不应显示在 git 状态中
```

### 2. 敏感变量管理

```bash
# API 密钥、密码等放本地层
envcli set API_KEY sk-1234567890

# 公共配置放项目层
envcli set API_URL https://api.example.com --target=project

# 临时覆盖（不存储）
envcli run API_KEY=temp-key -- ./deploy.sh
```

### 3. 审计日志

```bash
# 查看当前所有变量（包括来源）
envcli status --verbose

# 定位敏感变量来源
envcli list --source=local  # 仅查看本机配置
```

---

## 🔧 故障排除

### 问题 1：命令找不到

**现象**: `envcli: command not found`

**解决**:
```bash
# Windows: 添加到 PATH
# Linux/macOS:
export PATH=$PATH:/path/to/envcli
# 永久生效: 添加到 ~/.bashrc 或 ~/.zshrc
```

### 问题 2：变量未按预期工作

**诊断**:
```bash
# 1. 查看所有层级
envcli status --verbose

# 2. 诊断重复定义
envcli doctor --verbose

# 3. 查看具体变量来源
envcli list --source=local
envcli list --source=project
```

### 问题 3：Windows 上 run 命令失败

**检查**:
```bash
# 1. 确认命令在 PATH 中
where python
where node

# 2. 使用完整路径
envcli run -- C:\Python39\python.exe app.py

# 3. 检查错误详细信息
envcli run DB_HOST=localhost -- echo %DB_HOST% --verbose
```

### 问题 4：特殊字符处理

**问题**: 值包含空格或特殊字符

**解决**:
```bash
# 引号会被保留到程序中
envcli set MESSAGE="Hello World"

# 但 shell 可能先解析，需要转义
envcli set MESSAGE="Hello \"World\""
envcli run MESSAGE="Hello World" -- ./program
```

---

## 📊 性能考量

- ✅ **启动时间**: < 10ms
- ✅ **内存占用**: < 5MB
- ✅ **文件 I/O**: 只在需要时读取
- ✅ **零依赖**: 仅使用 Rust 标准库 + clap/serde

---

## 🔬 测试

```bash
# 运行所有测试
cargo test

# 显示测试输出
cargo test -- --nocapture

# 测试特定模块
cargo test test_run_command
```

---

## 🚧 开发计划

- [ ] 支持 `.json` 配置格式
- [ ] 支持 环境变量模板
- [ ] 支持 变量加密存储（sops 集成）
- [ ] 支持 项目级 `.envcli` 配置
- [ ] 插件系统

---

## 📄 许可证

MIT License - 详见 [LICENSE](LICENSE)

---

## 💡 设计原则

本工具严格遵循以下原则：

1. **Unix 哲学**: 每个命令只做一件事
2. **安静默认**: 成功无输出，减少噪音
3. **错误明确**: 失败时给出可操作的错误信息
4. **配置分离**: 代码与配置完全分离（12-factor）
5. **跨平台**: 一次编写，到处运行
6. **文件优先**: 配置即文件，文本即接口
7. **组合优于配置**: 通过管道组合命令

---

## 🙏 致谢

Built with Rust + clap + serde.

---

## 📞 贡献

欢迎提 Issue 和 PR！

---
**版本**: v0.1.0
**最后更新**: 2025-12-18
