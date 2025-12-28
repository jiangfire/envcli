#!/usr/bin/env python3
"""
EnvCLI 插件系统示例 - Python 外部插件

这个脚本展示了如何创建一个 Python 外部插件，通过 JSON 与 EnvCLI 通信。

使用方法：
1. 确保 Python 3.7+ 已安装
2. 赋予执行权限: chmod +x example_plugin.py
3. 加载插件: envcli plugin load ./example_plugin.py
4. 测试插件: envcli plugin test python-example
"""

import json
import sys
import time


def get_metadata():
    """返回插件元数据"""
    return {
        "id": "python-example",
        "name": "Python Example Plugin",
        "version": "1.0.0",
        "description": "Python 外部插件示例",
        "author": "EnvCLI Team",
        "plugin_type": "ExternalExecutable",
        "hooks": ["PreCommand", "PostCommand", "PreRun", "PostRun"],
        "extensions": [],
        "config_schema": None,
        "enabled": True,
        "dependencies": [],
        "platforms": ["Windows", "Linux", "MacOS"],
        "envcli_version": None
    }


def handle_pre_command(context):
    """处理命令执行前钩子"""
    command = context.get("command", "unknown")
    args = context.get("args", [])

    print(f"[PythonPlugin] PreCommand: {command} {args}", file=sys.stderr)

    return {
        "modified_env": {
            "PYTHON_PLUGIN": "active",
            "LAST_COMMAND": command,
            "COMMAND_TIME": str(time.time())
        },
        "plugin_data": {},
        "continue_execution": True,
        "message": f"Python plugin processed pre-command: {command}"
    }


def handle_post_command(context):
    """处理命令执行后钩子"""
    command = context.get("command", "unknown")

    print(f"[PythonPlugin] PostCommand: {command}", file=sys.stderr)

    return {
        "modified_env": {},
        "plugin_data": {
            "python_executed": "true",
            "timestamp": str(time.time())
        },
        "continue_execution": True,
        "message": f"Python plugin processed post-command: {command}"
    }


def handle_pre_run(context):
    """处理运行前钩子"""
    command = context.get("command", "unknown")

    print(f"[PythonPlugin] PreRun: {command}", file=sys.stderr)

    return {
        "modified_env": {
            "PYTHON_RUN_MODE": "example",
            "PYTHON_VERSION": sys.version.split()[0]
        },
        "plugin_data": {},
        "continue_execution": True,
        "message": "Python plugin pre-run hook"
    }


def handle_post_run(context):
    """处理运行后钩子"""
    command = context.get("command", "unknown")

    print(f"[PythonPlugin] PostRun: {command}", file=sys.stderr)

    return {
        "modified_env": {},
        "plugin_data": {},
        "continue_execution": True,
        "message": "Python plugin post-run hook"
    }


def execute_hook(hook_type, context):
    """执行指定的钩子"""
    handlers = {
        "PreCommand": handle_pre_command,
        "PostCommand": handle_post_command,
        "PreRun": handle_pre_run,
        "PostRun": handle_post_run,
    }

    handler = handlers.get(hook_type)
    if handler:
        return handler(context)
    else:
        return {
            "modified_env": {},
            "plugin_data": {},
            "continue_execution": True,
            "message": None
        }


def main():
    """主入口 - 处理 EnvCLI 请求"""
    try:
        # 读取请求
        request = json.load(sys.stdin)
        action = request.get("action")

        response = {"success": False}

        if action == "metadata":
            # 返回元数据
            response = {
                "success": True,
                "metadata": get_metadata()
            }

        elif action == "execute_hook":
            # 执行钩子
            hook_type = request.get("hook_type")
            context = request.get("context", {})

            if not hook_type:
                response["error"] = "Missing hook_type"
            else:
                result = execute_hook(hook_type, context)
                response = {
                    "success": True,
                    "result": result
                }

        elif action == "initialize":
            # 初始化插件
            config = request.get("config", {})
            print(f"[PythonPlugin] Initialized with config: {config}", file=sys.stderr)
            response = {"success": True}

        elif action == "shutdown":
            # 关闭插件
            print("[PythonPlugin] Shutdown", file=sys.stderr)
            response = {"success": True}

        else:
            response["error"] = f"Unknown action: {action}"

        # 输出响应
        json.dump(response, sys.stdout)
        sys.stdout.flush()

    except Exception as e:
        error_response = {
            "success": False,
            "error": str(e)
        }
        json.dump(error_response, sys.stdout)
        sys.stdout.flush()
        sys.exit(1)


if __name__ == "__main__":
    main()
