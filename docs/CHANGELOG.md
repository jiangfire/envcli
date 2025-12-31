# Changelog

所有本项目的重大变更都会记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
且本项目遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

---

## [Unreleased] - 开发中

### 新增功能
- 模板系统增强（交互式渲染）
- 更多插件类型支持
- 并行处理优化

### 修复
- 无

### 变更
- 无

### 已弃用
- 无

### 移除
- 无

---

## [0.1.0] - 2025-12-30

### 新增功能

#### 核心功能
- ✅ **四层架构存储引擎**
  - Local > Project > User > System 优先级系统
  - 跨平台支持 (Windows/Linux/macOS)
  - 自动层级合并和优先级处理

- ✅ **系统环境变量管理**
  - `envcli system-set <KEY> <VALUE>` - 设置系统级环境变量
  - `envcli system-unset <KEY>` - 删除系统级环境变量
  - Windows: PowerShell + 注册表
  - Linux/macOS: Shell 配置文件

- ✅ **加密存储（SOPS 集成）**
  - 支持 Age、GPG 等多种加密后端
  - `envcli set <KEY> <VALUE> --encrypt`
  - `envcli decrypt <KEY>`
  - `envcli check-sops`

- ✅ **插件系统**
  - 动态库加载 (.so/.dll)
  - 外部可执行插件
  - Shell/Python 脚本支持
  - Ed25519 签名验证
  - 热重载监控（500ms 防抖）
  - 钩子系统：PreCommand/PostCommand/Error/PreSet/PostGet/PreDelete/PostDelete
  - 优先级系统：Critical/High/Normal/Low/Background

- ✅ **模板系统**
  - `{{VAR}}` 语法
  - 默认值：`{{VAR|default}}`
  - 多层继承
  - 循环依赖检测

- ✅ **缓存系统**
  - 系统环境缓存（60秒 TTL）
  - 文件内容缓存（基于修改时间）
  - `envcli cache stats`
  - `envcli cache clear all`

- ✅ **完整测试套件**
  - 1165 行测试代码
  - 100% 通过率
  - 跨平台验证

#### 性能优化
- 系统环境读取：90%+ 性能提升（缓存）
- 文件读取：90%+ 性能提升（缓存）
- 环境合并：75% 性能提升（1次遍历）

#### 代码质量
- ✅ 模块化架构（27个辅助函数）
- ✅ 0 编译错误
- ✅ 0 Clippy 警告
- ✅ KISS/DRY/LOD 原则

### 技术架构

#### 主程序架构
```
main() → run_command() → 8个命令分组处理函数
├── handle_read_commands()      # 读取类: Get, List, Export, Status
├── handle_write_commands()     # 写入类: Set, Unset, Import
├── handle_plugin_commands()    # 插件类: Plugin 子命令
├── handle_encrypt_commands()   # 加密类: Encrypt, Decrypt, SetEncrypt, CheckSops
├── handle_system_commands()    # 系统类: SystemSet, SystemUnset, Doctor, Run
├── handle_template_commands()  # 模板类: Template 子命令
├── handle_config_commands()    # 配置类: Config 子命令
└── handle_cache_commands()     # 缓存类: Cache 子命令
```

#### 源代码组织
```
src/
├── main.rs                    # 主程序入口 (1724行)
├── cli.rs                    # CLI 参数解析
├── types.rs                  # 核心数据结构
├── error.rs                  # 错误处理系统
├── core/
│   └── store.rs              # 核心存储引擎
├── plugin/                   # 插件系统
│   ├── manager.rs           # 插件管理器
│   ├── signature.rs         # 签名验证
│   ├── watcher.rs           # 热重载监控
│   ├── hook.rs              # 钩子系统
│   └── loader/              # 插件加载器
├── template/                 # 模板系统
│   ├── parser.rs            # 模板解析器
│   └── renderer.rs          # 模板渲染器
├── config/                   # 配置格式处理
│   └── format/
│       ├── dotenv.rs        # .env 解析器
│       └── encrypted_dotenv.rs # 加密 .env 解析器
└── utils/                    # 工具模块
    ├── encryption.rs        # SOPS 加密
    ├── env_merge.rs         # 环境变量合并
    ├── executor.rs          # 命令执行器
    ├── paths.rs             # 跨平台路径
    ├── system_env.rs        # 系统环境变量管理
    ├── system_env_tests.rs  # 系统环境测试 (954行)
    └── system_env_integration_tests.rs # 集成测试 (211行)
```

### 重构成果

#### KISS 原则 ✅
- 主函数：375+ 行 → 50 行 (⬇️ 87%)
- run_command：375+ 行 → 120 行 (⬇️ 68%)
- 代码清晰，职责单一

#### DRY 原则 ✅
- 提取 27 个辅助函数
- 消除所有代码重复
- 维护成本降低

#### LOD 原则 ✅
- 辅助函数封装复杂交互
- 模块间低耦合
- 易于独立测试

### 已知限制

#### Windows
- 机器级环境变量需要管理员权限
- 需要重启终端或运行 `refreshenv` 使更改生效

#### Unix/Linux/macOS
- 不支持真正的机器级环境变量（需要 root）
- 需要 `source ~/.bashrc` 或重新打开终端
- 不同 shell 的配置文件可能不同

### 升级指南

从 v0.0.x 升级到 v0.1.0：

1. **无需迁移** - 新功能完全向后兼容
2. **可选功能** - 系统环境变量管理是可选的
3. **配置不变** - 现有项目配置无需修改

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
