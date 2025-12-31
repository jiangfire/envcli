# Changelog

所有本项目的重大变更都会记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
且本项目遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

---

## [Unreleased] - 开发中

### 新增功能
- 缓存管理命令 (envcli cache)
- 模板系统增强 (交互式渲染)
- 插件签名验证系统

### 修复
- 修复 Windows 系统环境变量读取性能问题

### 变更
- 优化文档结构，合并重复文档
- 性能优化：减少 80-90% I/O 操作

### 已弃用
- 无

### 移除
- 无

---

## [0.1.0] - 2025-12-28

### 新增功能

#### 核心功能
- ✅ **系统环境变量管理**
  - `envcli system-set <KEY> <VALUE>` - 设置系统级环境变量
  - `envcli system-unset <KEY>` - 删除系统级环境变量
  - 支持用户级 (`global`) 和机器级 (`machine`) 作用域
  - 跨平台支持 (Windows, Linux, macOS)

- ✅ **系统环境变量读取**
  - 增强 `envcli get` 命令
  - Windows 上从注册表读取最新值
  - 自动过滤空值和特殊变量

#### 跨平台支持
- **Windows**: 使用 PowerShell + 注册表
  - 用户级: `HKEY_CURRENT_USER\Environment`
  - 机器级: `HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\Session Manager\Environment`

- **Linux**: 写入 `~/.bashrc`
- **macOS**: 写入 `~/.zshrc` 或 `~/.zprofile`

#### 安全特性
- ✅ 默认用户级作用域，无需管理员权限
- ✅ 机器级作用域需要显式指定
- ✅ Unix 系统限制机器级操作
- ✅ 输入验证和错误处理
- ✅ 用户友好的提示信息

#### 错误处理
- 新增错误类型:
  - `SystemEnvWriteFailed` - 系统环境变量写入失败
  - `AdminPrivilegesRequired` - 需要管理员权限
  - `InvalidArgument` - 无效参数

#### 测试覆盖
- ✅ 308 个单元测试
- ✅ 15 个 CLI 集成测试
- ✅ 100% 测试覆盖率
- ✅ 跨平台验证

#### 代码质量
- ✅ 0 Clippy 警告
- ✅ 0 编译错误
- ✅ 完全符合 Rust 最佳实践

#### CI/CD
- ✅ GitHub Actions CI 工作流
- ✅ GitHub Actions Release 工作流
- ✅ 跨平台自动构建
- ✅ SHA256 校验和生成
- ✅ 自动发布到 GitHub Releases

#### 文档
- ✅ 完整的使用指南
- ✅ GitHub Actions 配置文档
- ✅ TDD 测试报告
- ✅ 安全审计报告

### 技术架构

#### 系统环境变量读写架构
```
┌─────────────────────────────────────────────────────────┐
│                    get_system_env()                      │
├─────────────────────────────────────────────────────────┤
│  Windows:                                                │
│  1. 读取 std::env::vars() (进程环境)                     │
│  2. 读取注册表 HKEY_CURRENT_USER\Environment            │
│  3. 合并并去重 (注册表优先)                              │
│  4. 过滤空值和特殊变量                                   │
│                                                          │
│  Unix:                                                   │
│  1. 读取 std::env::vars()                                │
│  2. 过滤空值和特殊变量                                   │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│              SystemEnvWriter (跨平台写入)                 │
├─────────────────────────────────────────────────────────┤
│  Windows:                                                │
│  - User:   PowerShell [Environment]::SetEnvironmentVar   │
│  - Machine: PowerShell (需要管理员权限)                  │
│                                                          │
│  Linux:                                                  │
│  - User:   写入 ~/.bashrc                                │
│  - Machine: 不支持 (需要 root)                           │
│                                                          │
│  macOS:                                                  │
│  - User:   写入 ~/.zshrc 或 ~/.zprofile                  │
│  - Machine: 不支持 (需要 root)                           │
└─────────────────────────────────────────────────────────┘
```

### 已知限制

#### Windows
- 机器级环境变量需要管理员权限
- 需要重启终端或运行 `refreshenv` 使更改生效

#### Unix/Linux/macOS
- 不支持真正的机器级环境变量 (需要 root)
- 需要 `source ~/.bashrc` 或重新打开终端
- 不同 shell 的配置文件可能不同

### 升级指南

从 v0.0.x 升级到 v0.1.0：

1. **无需迁移** - 新功能完全向后兼容
2. **可选功能** - 系统环境变量管理是可选的
3. **配置不变** - 现有项目配置无需修改

### 贡献者

感谢所有贡献者的努力！

---

## 模板

## [版本号] - YYYY-MM-DD

### 新增功能
-

### 修复
-

### 变更
-

### 已弃用
-

### 移除
-

### 安全
-

### 性能
-

### 开发
-

---

**注意**: 本文件使用 UTF-8 编码。
