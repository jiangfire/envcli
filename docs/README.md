# EnvCLI 文档中心

欢迎来到 EnvCLI 文档中心！这里包含了项目的所有技术文档、使用指南和最佳实践。

---

## 📚 文档导航

### 📊 项目概览
**[project-overview.md](project-overview.md)** - 项目架构、功能状态、性能优化

包含：
- 项目概述和核心特性
- 源代码组织架构
- 代码质量指标
- 性能优化详情
- 重构成果总结

### 📖 用户指南
**[user-guide.md](user-guide.md)** - 完整的使用说明和快速上手

包含：
- 安装配置
- 5分钟快速上手
- 核心概念（层级系统）
- 常用命令速查
- 加密存储
- 插件系统
- 模板系统
- 故障排除
- 缓存管理
- 最佳实践

### 🔧 开发指南
**[development-guide.md](development-guide.md)** - 插件开发和最佳实践

包含：
- 代码架构原则（KISS/DRY/LOD）
- 插件开发完整指南
- 安全最佳实践
- 配置管理策略
- 模板系统规范
- 团队协作流程
- 性能优化技巧
- 测试与质量

### 📈 项目管理
**[CHANGELOG.md](CHANGELOG.md)** - 版本更新记录

---

## 🎯 快速开始

### 作为用户
```bash
# 1. 阅读用户指南
cat docs/user-guide.md

# 2. 5分钟上手
# 按照 user-guide.md 的快速上手章节操作

# 3. 遇到问题
envcli doctor
```

### 作为开发者
```bash
# 1. 了解项目架构
cat docs/project-overview.md

# 2. 阅读开发指南
cat docs/development-guide.md

# 3. 开发插件
# 参考 development-guide.md 的插件开发章节
```

---

## 📊 项目状态

| 指标 | 状态 | 详情 |
|------|------|------|
| 功能完整性 | ✅ 优秀 | 核心功能全部实现 |
| 测试覆盖 | ✅ 优秀 | 1000+ 行测试，100% 通过 |
| 跨平台支持 | ✅ 良好 | Windows/Linux/macOS |
| 代码质量 | ✅ 优秀 | 模块化架构，遵循最佳实践 |
| 文档完整度 | ✅ 优秀 | 用户指南 + 开发指南 |

---

## 🔗 相关资源

- **源代码**: `src/` 目录
- **测试**: `src/**/tests.rs` 和 `src/**/integration_tests.rs`
- **配置**: `Cargo.toml`
- **CI/CD**: `.github/workflows/`

---

## 📝 提交信息规范

```bash
# 重构提交
git commit -m "refactor: Simplify main.rs with KISS/DRY/LOD principles"

# UX 增强提交
git commit -m "feat: UX增强 + 文档完善"

# 性能优化提交
git commit -m "perf: 实现性能缓存优化"

# 文档更新
git commit -m "docs: 重构文档系统，优化项目结构"
```

---

## 🚀 核心特性

### 四层架构存储引擎
```
Local > Project > User > System
```

### 插件系统
- 动态库加载 (.so/.dll)
- 外部可执行插件
- Shell/Python 脚本
- Ed25519 签名验证
- 热重载监控
- 钩子系统 (PreCommand/PostCommand/Error/PreSet/PostGet)

### 模板系统
- `{{VAR}}` 语法
- 多层继承
- 循环依赖检测
- 默认值支持

### 安全特性
- SOPS 集成 (Age/GPG)
- 加密存储
- 签名验证
- 权限控制

### 性能优化
- 系统环境缓存 (60秒 TTL)
- 文件内容缓存 (基于修改时间)
- 环境变量合并优化 (4次→1次)
- 80-90% I/O 性能提升

---

## 📅 时间线

**开始**: 2025-12-27
**完成**: 2025-12-30
**文档优化**: 2025-12-31

---

**文档版本**: v1.0.0
**最后更新**: 2025-12-31
**维护者**: EnvCLI 团队
