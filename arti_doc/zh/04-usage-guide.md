# 04 使用指南

## 环境要求

从源码运行需要 Rust 工具链。文档站需要可选的 `mdbook`。

```powershell
cargo fmt --all
cargo test --workspace
cargo build --workspace
```

## 查看状态

```powershell
cargo run -p unio -- status
```

该命令会显示 daemon、workspace、session 数量、待审批数量、最新 trace、上下文比例和当前模型 provider。

## 执行 Prompt

```powershell
cargo run -p unio -- exec "hello unio"
cargo run -p unio -- exec "summarize this repository"
```

如果没有真实模型配置，Unio 使用 mock provider。mock 输出可用于验证本地流程是否连通。

## 进入交互式界面

```powershell
cargo run -p unio
```

常用交互能力：

- 输入 `/` 查看 slash command 建议。
- 输入 `@` 搜索 workspace 文件引用。
- `Shift+Enter` 或 `Ctrl+J` 插入换行。
- `Ctrl+C` 两次退出。
- `/model` 进入模型配置。
- `/approval` 或 `/approvals` 查看待审批。
- `/trace <trace_id> [run_id]` 查看 trace。
- `/resume [limit]` 查看最近 transcript。

## 配置模型

交互式配置：

```powershell
cargo run -p unio -- "/model"
```

配置会写入：

```text
~/.unio/config.toml
```

示例：

```toml
[model]
provider = "openai-compatible"
model = "gpt-4o-mini"
base_url = "https://api.openai.com/v1"
api_key = "sk-..."
```

环境变量优先级更高。常见变量：

```powershell
$env:UNIO_MODEL_PROVIDER="openai-compatible"
$env:OPENAI_MODEL="gpt-4o-mini"
$env:OPENAI_API_KEY="sk-..."
```

Anthropic 风格 provider 可使用：

```powershell
$env:UNIO_MODEL_PROVIDER="anthropic"
$env:ANTHROPIC_MODEL="claude-3-5-sonnet-latest"
$env:ANTHROPIC_API_KEY="..."
```

## 直接执行工具

读取文件：

```powershell
cargo run -p unio -- tool read --args path=README.md
```

写入文件：

```powershell
cargo run -p unio -- tool write --args path=notes.txt,content=hello
```

默认模式下写入会等待审批。需要明确允许时可使用：

```powershell
cargo run -p unio -- tool write --approval full-trust --args path=notes.txt,content=hello
```

## 审批工具调用

列出待审批：

```powershell
cargo run -p unio -- approvals
```

通过审批：

```powershell
cargo run -p unio -- approvals approve approval_xxx
```

拒绝审批：

```powershell
cargo run -p unio -- approvals deny approval_xxx
```

查看审批历史：

```powershell
cargo run -p unio -- approvals history
```

## 使用技能

创建 workspace 技能：

```powershell
New-Item -ItemType Directory -Force .unio\skills\repo-helper
Set-Content .unio\skills\repo-helper\SKILL.md "# Repo helper`nUse for repository analysis."
```

列出技能：

```powershell
cargo run -p unio -- skills
```

调用技能工具：

```powershell
cargo run -p unio -- tool skill-tool --approval full-trust --args name=repo-helper,request=inspect-modules
```

## 查看 Session 和 Trace

列出 session：

```powershell
cargo run -p unio -- sessions
```

恢复最近记录：

```powershell
cargo run -p unio -- resume --limit 10
```

查询 trace：

```powershell
cargo run -p unio -- trace trace_xxx
cargo run -p unio -- trace trace_xxx --run run_xxx
```

## 运行文档站

```powershell
cargo install mdbook
mdbook serve docs
mdbook serve docs/zh
```

## 开发验证

完成代码或文档变更前建议运行：

```powershell
cargo fmt --all
cargo test --workspace
cargo build --workspace
```

文档变更时，如果安装了 mdBook：

```powershell
mdbook build docs
mdbook build docs/zh
```
