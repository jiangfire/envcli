# EnvCLI 项目概览

> 版本: v0.3.0

## 项目定位

EnvCLI 是一个跨平台环境变量管理工具，面向本地开发、团队协作和 CI 场景，提供统一的分层配置与命令行工作流。

## 当前能力

- 四层级存储模型: `local > project > user > system`
- 变量读写与来源选择
- `.env/json` 导入导出
- 运行命令时注入环境
- 配置初始化、诊断与缓存管理

## 代码结构

```text
src/
├── main.rs                 # CLI 入口与命令分发
├── cli.rs                  # clap 参数定义
├── app.rs                  # 应用容器与依赖组装
├── lib.rs                  # 模块导出
├── commands/               # 各命令处理器
├── application/            # 应用服务层
├── domain/                 # 领域模型、错误、仓储接口
└── infrastructure/         # 存储/缓存/路径实现
```

## 架构分层

- `domain`: 纯业务类型与仓储抽象，不依赖具体实现。
- `application`: 编排业务流程，调用领域接口。
- `infrastructure`: 提供文件存储、缓存、路径等具体实现。
- `commands`: 对接 CLI 子命令，将参数映射到应用服务。

## 关键模块职责

- `src/cli.rs`: 命令和参数定义。
- `src/main.rs`: 解析参数并分发到 `commands/*`。
- `src/infrastructure/storage.rs`: `.env` 文件读取、合并、写入。
- `src/commands/config.rs`: 初始化配置目录与基础文件。

## 技术栈

- Rust 2024
- tokio
- clap
- serde / serde_json
- anyhow / thiserror / miette
- regex / chrono

## 质量与验证

常用验证命令:

```bash
cargo check
cargo test
cargo test --test cli_integration
```

## 版本变更提示

当前版本已移除插件、模板与加密模块，并完成历史代码清理。
