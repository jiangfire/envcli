# EnvCLI 开发指南

> 面向贡献者的最小必要开发文档（当前版本）

## 开发环境

- Rust 1.75+
- Cargo
- Windows / Linux / macOS 其一

## 本地运行

```bash
cargo run -- --help
cargo run -- status
```

## 代码组织

```text
src/
├── main.rs
├── cli.rs
├── app.rs
├── commands/
├── application/
├── domain/
└── infrastructure/
```

### 分层约束

- `domain` 不依赖 `infrastructure`。
- `application` 只依赖 `domain` 抽象。
- `commands` 负责参数校验与调用服务，不放业务细节。
- `main` 只做装配和分发。

## 新增命令的标准流程

1. 在 `src/cli.rs` 增加命令定义。
2. 在 `src/commands/` 增加处理器并实现 `CommandHandler`。
3. 在 `src/main.rs` 增加分发分支。
4. 如需业务能力，优先加到 `application/services`。
5. 如需持久化，扩展 `domain/repositories` 与 `infrastructure` 实现。

## 错误处理约定

- 领域错误统一使用 `DomainError`。
- 命令层输出面向用户、可操作。
- 对外错误信息避免暴露底层实现细节。

## 测试约定

### 必跑项

```bash
cargo check
cargo test
cargo test --test cli_integration
```

### 推荐项

```bash
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
```

## 文档维护约定

- 修改命令参数时同步更新 `README.md` 与 `docs/user-guide.md`。
- 重大行为变更需更新 `docs/CHANGELOG.md` 的 `Unreleased`。
- 文档仅描述当前可用功能，不保留下线功能的操作说明。

## 提交流程建议

1. 小步提交，提交信息说明“做了什么 + 为什么”。
2. 每次提交前至少通过 `cargo check`。
3. 涉及 CLI 行为时补充或更新集成测试。

## 版本说明

当前版本已移除插件、模板与加密模块，并清理历史遗留代码。
