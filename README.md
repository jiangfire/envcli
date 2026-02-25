# EnvCLI - 跨平台环境变量管理工具

<p align="center">
  <img src="https://img.shields.io/badge/Rust-1.75+-blue?logo=rust" alt="Rust Version" />
  <img src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey" alt="Platforms" />
  <img src="https://img.shields.io/badge/license-MIT-blue" alt="License" />
</p>

一个简洁的环境变量管理工具，提供四层级配置、导入导出、命令注入运行、缓存与配置诊断能力。

## 核心能力

- 四层级环境管理: `local > project > user > system`
- 统一读写命令: `get / set / unset / list`
- 导入导出: `.env` 与 `json` 格式
- 运行时注入: `run --env ... -- <command>`
- 缓存管理与配置自检

## 当前命令

```text
envcli get <KEY>
envcli set <KEY> <VALUE> [--target <local|project|user>]
envcli unset <KEY> [--target <local|project|user>]
envcli list [--source <system|user|project|local>] [--format <env|json>]
envcli export [--source <...>] [--format <env|json>]
envcli import <FILE> [--target <local|project|user>]
envcli run [--env KEY=VALUE ...] [--from-file FILE] -- <COMMAND...>
envcli status
envcli doctor
envcli system-set <KEY> <VALUE> [--scope <global|machine>]
envcli system-unset <KEY> [--scope <global|machine>]
envcli cache <stats|clear>
envcli config <validate|init|info>
```

## 快速开始

### 1. 构建

```bash
git clone https://github.com/your-repo/envcli.git
cd envcli
cargo build --release
```

### 2. 初始化

```bash
envcli config init
envcli doctor
```

### 3. 基础操作

```bash
envcli set DB_HOST localhost
envcli set DB_PORT 5432 --target project
envcli get DB_HOST
envcli list --format json
```

### 4. 导入导出

```bash
envcli import .env --target local
envcli export --source project > project.env
envcli export --format json > env.json
```

### 5. 运行时注入

```bash
envcli run --env APP_ENV=development --env LOG_LEVEL=debug -- npm run dev
envcli run --from-file .env.production -- ./start.sh
```

## 层级与文件位置

- 用户层: `~/.envcli/user.env`
- 项目层: `<project>/.envcli/project.env`
- 本地层: `<project>/.envcli/local.env`
- 系统层: 操作系统环境变量

优先级: `local > project > user > system`。

## 兼容性说明

从 `v0.3.0` 开始，已移除插件、模板与加密模块；当前文档仅覆盖现有命令与能力。

## 开发与测试

```bash
cargo check
cargo test
cargo test --test cli_integration
```

更多开发细节见 `docs/development-guide.md`。

## 许可证

MIT
