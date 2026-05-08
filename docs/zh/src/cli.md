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

## 直接工具命令

```powershell
cargo run -p unio -- tool read --args path=README.md
cargo run -p unio -- tool write --args path=notes.txt,content=hello
```

写入和进程类工具会根据所选策略要求审批。
