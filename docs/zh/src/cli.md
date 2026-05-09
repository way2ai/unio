# CLI

`unio` 二进制是用户入口。它与 daemon 通信，并让终端、编辑器和自动化脚本都能复用同一套命令行为。

## 常用命令

```powershell
cargo run -p unio -- status
cargo run -p unio -- exec "summarize this repository"
cargo run -p unio -- resume --limit 10
cargo run -p unio -- trace <trace_id> --run <run_id>
```

## 交互入口

不传子命令时会打开终端界面：

```powershell
cargo run -p unio
```

常用输入能力：

- `/` 打开命令建议。
- `@` 搜索工作区文件。
- `Shift+Enter` 或 `Ctrl+J` 插入换行。
- 连续两次 `Ctrl+C` 退出。

提交普通提示后，终端界面会立即清空编辑区，并先把提示移动到消息流中；
daemon 完成运行后再追加 agent 响应。

## 模型配置

`/model` slash command 会显示并更新 `~/.unio/config.toml` 中的持久化模型设置。

```powershell
cargo run -p unio -- "/model"
```

`/model` 会先显示当前 provider，然后在终端界面或直接 slash 调用中提示输入 provider、model、
base URL 和 API key。这个流程会更新 `~/.unio/config.toml`；环境变量仍然优先于配置文件。

## 运行渲染

交互界面和直接命令会把模型与工具活动渲染成更易读的运行摘要，而不是直接暴露原始事件名：

- `Running`：模型执行和阶段变化。
- `Tool`：已完成的工具调用，包含工具名和简短目标。
- `Approval`：等待审批的工具调用或审批结果。
- `Skill`：skill-tool 活动，尽量显示技能名。
- `Done`：最终阶段、模型、token 用量和上下文比例。

调试时仍可通过 `/trace <trace_id>` 查看原始 trace event。

## 直接工具命令

```powershell
cargo run -p unio -- tool read --args path=README.md
cargo run -p unio -- tool write --args path=notes.txt,content=hello
```

写入和进程类工具会根据所选策略要求审批。
