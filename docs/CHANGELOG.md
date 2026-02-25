# Changelog

所有本项目的重大变更都会记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
且本项目遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

---

## [Unreleased]

### 变更

- 无

---

## [0.3.0] - 2026-02-25

### 变更

- 文档全面清理并与当前命令行为对齐（`README.md` + `docs/`）。
- CI/CD 工作流修正：
  - CI 支持 tag 触发发布校验。
  - Release 构建目标二进制名统一为 `envcli`。
  - 发布说明移除过期功能描述并更新 changelog 路径。
- 历史代码清理：删除不可达遗留模块（`src/config`、`src/core`、`src/utils`、`src/error.rs`、`src/types.rs`、`src/test_utils.rs`）。

### 移除

- 移除插件模块（Plugin）。
- 移除模板模块（Template）。
- 移除加密模块（Encryption）。
- CLI 不再提供 `plugin`、`template`、`encrypt`、`decrypt` 子命令。
- `set` 命令不再支持 `--encrypt` 选项。

---

## [0.1.0] - 2025-12-30

### 新增

- 四层级环境变量存储模型（Local/Project/User/System）。
- 核心命令集（get/set/unset/list/import/export/run/status/doctor）。
- SOPS 加密存储与解密读取能力。
- 缓存管理与配置管理命令。

### 说明

- v0.1.0 包含的插件、模板与加密能力已在后续版本移除。

---

**注意**: 本文件使用 UTF-8 编码。
