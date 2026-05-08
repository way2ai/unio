# 快速开始

所有命令都在仓库根目录运行。

## 验证工作区

```powershell
cargo fmt --all
cargo test --workspace
cargo build --workspace
```

## 查看状态

```powershell
cargo run -p unio -- status
```

## 运行提示词

```powershell
cargo run -p unio -- exec "hello unio"
```

如果没有配置模型凭据，Unio 会使用 mock provider，让本地开发保持可预测。

## 配置真实模型

Unio 会从 `~/.unio/config.toml` 读取持久化模型设置。环境变量优先级更高，适合临时覆盖。

使用 slash command 创建或更新这个文件：

```powershell
cargo run -p unio -- "/model"
```

```toml
[model]
provider = "openai-compatible"
model = "gpt-4o-mini"
base_url = "https://api.openai.com/v1"
api_key = "sk-..."
```

Anthropic 示例：

```toml
[model]
provider = "anthropic"
model = "claude-3-5-sonnet-latest"
api_key = "sk-ant-..."
```

支持的 provider 是 `openai`、`openai-compatible`、`anthropic` 和 `mock`。如果选择真实 provider
但没有配置 API key，Unio 会报告请求的 provider，并回退到 mock provider。

## 通过工具层读取文件

```powershell
cargo run -p unio -- tool read --args path=README.md
```

## 检查最近工作

```powershell
cargo run -p unio -- resume --limit 10
cargo run -p unio -- trace <trace_id> --run <run_id>
```

## 下一步

- 阅读 [CLI](cli.md) 了解命令参考。
- 在启用写入或进程执行前阅读 [工具与审批](tools-and-approvals.md)。
- 使用 [技能](skills.md) 添加仓库专属工作流。
