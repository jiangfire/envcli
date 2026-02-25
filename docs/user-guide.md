# EnvCLI 用户指南

> 版本: v0.3.0

## 安装

### 从源码构建

```bash
git clone https://github.com/your-repo/envcli.git
cd envcli
cargo build --release
```

## 5 分钟快速上手

### 1. 初始化配置

```bash
envcli config init
envcli doctor
```

### 2. 设置和读取变量

```bash
envcli set DB_HOST localhost
envcli set DB_PORT 5432 --target project
envcli get DB_HOST
```

### 3. 列出变量

```bash
envcli list
envcli list --source project
envcli list --format json
```

### 4. 导入导出

```bash
envcli import .env --target local
envcli export --source project > project.env
envcli export --format json > env.json
```

### 5. 运行命令并注入环境

```bash
envcli run --env APP_ENV=development -- cargo run
envcli run --from-file .env.production -- ./start.sh
```

## 层级说明

| 层级 | 作用 | 路径 |
|---|---|---|
| Local | 当前项目本地覆盖 | `./.envcli/local.env` |
| Project | 项目共享配置 | `./.envcli/project.env` |
| User | 用户级默认配置 | `~/.envcli/user.env` |
| System | 系统环境变量 | OS 环境变量 |

优先级: `local > project > user > system`。

## 常用命令速查

### 核心操作

```bash
envcli get <KEY>
envcli set <KEY> <VALUE> --target <local|project|user>
envcli unset <KEY> --target <local|project|user>
envcli list --source <system|user|project|local> --format <env|json>
```

### 导入导出

```bash
envcli import <FILE> --target <local|project|user>
envcli export --source <system|user|project|local> --format <env|json>
```

### 运行命令

```bash
envcli run --env KEY=VALUE -- <command>
envcli run --from-file .env -- <command>
```

### 系统级操作

```bash
envcli system-set <KEY> <VALUE> --scope <global|machine>
envcli system-unset <KEY> --scope <global|machine>
```

### 配置与缓存

```bash
envcli config validate
envcli config init
envcli config info
envcli cache stats
envcli cache clear all
```

## 故障排查

```bash
envcli doctor
envcli get SOME_KEY --verbose
envcli list --format json
```

常见建议:

- 写入失败先检查目标层级是否可写。
- 系统级修改失败通常需要更高权限。

## 版本说明

当前版本已不包含插件、模板和加密命令。
