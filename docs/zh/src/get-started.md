# 快速开始

本指南说明如何构建和使用当前的 Unio 重构版本。

## 环境要求

- Rust stable 工具链
- Cargo
- Windows、macOS 或 Linux

```powershell
rustc --version
cargo --version
```

## 构建与测试

在仓库根目录运行：

```powershell
cargo fmt --all
cargo test --workspace
cargo build --workspace
```

## 启动 Unio

启动混合 CLI/TUI：

```powershell
cargo run -p unio
```

裸 `unio` 命令打开交互界面，脚本化子命令同样可用：

```powershell
cargo run -p unio -- status
cargo run -p unio -- exec "hello unio"
cargo run -p unio -- resume --limit 10
cargo run -p unio -- trace <trace_id> --run <run_id>
```

若未配置模型凭证，Unio 将回退到 mock provider。

## 混合输入

混合界面常用按键：

- `/`：显示斜杠命令建议。
- `@`：搜索工作区文件。
- `Up` / `Down`：选择建议或滚动历史。
- `Left` / `Right`：在输入行中移动光标。
- `Home` / `End`、`Ctrl+A` / `Ctrl+E`：跳转到行首或行尾。
- `Ctrl+W`：删除上一个单词。
- `Ctrl+U`：清除提示。
- `Shift+Enter` 或 `Ctrl+J`：插入换行。
- `Ctrl+C` 两次：退出。

示例：

```text
/re
inspect @README.md
plan a refactor
```

## 工具与审批

直接运行工具：

```powershell
cargo run -p unio -- tool read --args path=README.md
cargo run -p unio -- tool write --args path=notes.txt,content=hello
```

在 `default` 模式下，写操作和进程执行需要审批：

```powershell
cargo run -p unio -- approvals
cargo run -p unio -- approvals approve approval_xxx
cargo run -p unio -- approvals deny approval_xxx
```

仅在明确允许执行时使用 `full-trust`：

```powershell
cargo run -p unio -- tool write --approval full-trust --args path=notes.txt,content=hello
```

## 技能

Unio 从以下路径发现技能：

```text
{workspace}/.unio/skills/
~/.unio/skills/
```

创建并列出工作区技能：

```powershell
New-Item -ItemType Directory -Force .unio\skills\repo-helper
Set-Content .unio\skills\repo-helper\SKILL.md "# Repo helper`nUse for repository analysis."
cargo run -p unio -- skills
```

通过工具契约调用技能：

```powershell
cargo run -p unio -- tool skill-tool --approval full-trust --args name=repo-helper,request=inspect-modules
```

## Mock 开发路径

在没有模型凭证的情况下开发时，使用 mock 指令：

```powershell
cargo run -p unio -- exec "mock-tool read path=README.md"
cargo run -p unio -- exec "mock-tool skill-tool name=repo-helper,request=inspect-modules"
cargo run -p unio -- exec "mock-usage input=90000,output=1000"
```

更多设计细节请阅读 [架构](architecture.md)、[混合输入编辑](hybrid-input-editing.md) 和 [目标架构](target-architecture.md)。
