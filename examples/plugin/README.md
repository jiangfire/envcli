# EnvCLI 插件示例

本目录包含 EnvCLI 插件系统的示例实现。

## 目录结构

```
examples/plugin/
├── example_plugin.rs    # Rust 动态库插件示例
├── example_plugin.py    # Python 外部插件示例
└── README.md           # 本文件
```

## Rust 动态库插件

### 编译

```bash
# 编译为动态库
rustc --crate-type dylib example_plugin.rs -o example_plugin.dll

# 或者使用 cargo（如果作为项目的一部分）
cargo build --release
```

### 加载

```bash
# 加载插件
envcli plugin load ./example_plugin.dll

# 查看插件列表
envcli plugin list --verbose

# 测试插件
envcli plugin test example-plugin

# 测试特定钩子
envcli plugin test example-plugin --hook precommand
```

## Python 外部插件

### 前置要求

```bash
# 确保 Python 3.7+ 已安装
python --version

# 赋予执行权限（Linux/macOS）
chmod +x example_plugin.py
```

### 加载

```bash
# 加载插件
envcli plugin load ./example_plugin.py

# 查看插件列表
envcli plugin list --verbose

# 测试插件
envcli plugin test python-example

# 测试特定钩子
envcli plugin test python-example --hook prerun
```

## 插件功能说明

### 支持的钩子

两个示例插件都支持以下钩子：

1. **PreCommand** - 命令执行前
   - 注入环境变量
   - 记录命令信息

2. **PostCommand** - 命令执行后
   - 记录执行信息
   - 更新插件数据

3. **PreRun** - run 命令执行前
   - 设置运行模式
   - 注入运行时变量

4. **PostRun** - run 命令执行后
   - 清理工作

### 注入的环境变量

- `EXAMPLE_PLUGIN_PRE` - PreCommand 钩子注入
- `COMMAND_NAME` - 当前命令名称
- `RUN_MODE` - 运行模式
- `EXAMPLE_VERSION` - 插件版本
- `PYTHON_PLUGIN` - Python 插件标识
- `PYTHON_RUN_MODE` - Python 运行模式
- `PYTHON_VERSION` - Python 版本

## 配置管理

```bash
# 设置配置
envcli plugin config set example-plugin timeout 30

# 获取配置
envcli plugin config get example-plugin

# 重置配置
envcli plugin config reset example-plugin
```

## 状态查看

```bash
# 查看所有插件状态
envcli plugin status --verbose

# 查看单个插件状态
envcli plugin status --plugin example-plugin
```

## 卸载插件

```bash
# 卸载插件
envcli plugin unload example-plugin
envcli plugin unload python-example
```

## 开发自己的插件

### Rust 插件开发

1. 创建新项目：
```bash
cargo new --lib my-plugin
```

2. 添加依赖：
```toml
# Cargo.toml
[dependencies]
envcli = { path = "../envcli" }
```

3. 实现 Plugin trait（参考 example_plugin.rs）

4. 编译为动态库：
```bash
cargo build --release
```

### Python 插件开发

1. 创建 Python 脚本
2. 实现以下函数：
   - `get_metadata()` - 返回元数据
   - `execute_hook(hook_type, context)` - 执行钩子
3. 通过 JSON 与 EnvCLI 通信

## 调试技巧

### 查看详细日志

```bash
# 使用 verbose 模式
envcli plugin list --verbose
envcli plugin test <id> --verbose
```

### 测试钩子执行

```bash
# 测试所有钩子
envcli plugin test <id>

# 测试单个钩子
envcli plugin test <id> --hook precommand
```

### 检查配置

```bash
envcli plugin config get <id>
```

## 常见问题

### Q: 插件加载失败？

A: 检查：
1. 文件路径是否正确
2. 文件是否有执行权限（Python 插件）
3. 动态库是否与当前平台兼容

### Q: 钩子没有执行？

A: 检查：
1. 插件是否已启用
2. 钩子类型是否在 metadata 中注册
3. 使用 `plugin status` 查看插件状态

### Q: 如何调试插件？

A:
1. 在插件代码中添加打印语句
2. 使用 `--verbose` 参数
3. 查看 `plugin test` 的输出

## 更多资源

- 主文档：`PLUGIN_SYSTEM.md`
- 类型定义：`src/plugin/types.rs`
- 插件管理器：`src/plugin/mod.rs`
