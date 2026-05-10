# 02 技术架构

## 总体架构

Unio 采用本地 daemon 中心化架构。CLI 保持相对薄，只负责解析命令、展示交互界面、读取 daemon instance 文件并调用 daemon HTTP API。daemon 拥有 session、run、审批队列、工具执行、存储和 trace。

```text
user
  -> unio CLI / TUI
  -> local daemon HTTP API
  -> root agent
  -> model provider or tool registry
  -> security approval policy
  -> SQLite / JSONL storage
  -> trace and user-visible output
```

这种分工让模型调用、工具执行和持久化远离 CLI，后续可以扩展到编辑器、后台进程或其他自动化入口。

## Workspace 分层

```text
apps/
  cli       command parsing, TUI, daemon client, output rendering
  daemon    local HTTP runtime, orchestration, persistence

crates/
  core           IDs, paths, daemon instance metadata
  protocol       shared request/response and transcript schemas
  agent          root agent, planner, sub-agent and skill-agent contracts
  model          provider configuration and model API adapters
  tools          built-in tool registry and execution implementations
  security       permission mode and approval decision matrix
  skills         SKILL.md discovery and skill-tool execution
  storage        SQLite session/run/audit data and JSONL transcripts
  observability  trace event store and context budget events
```

核心依赖方向是：`apps` 调用多个 domain crate；`protocol` 提供共享 wire/data 类型；`core` 提供基础 ID 和路径；`tools` 依赖 `security` 和 `skills`；`daemon` 将 agent、tools、storage、observability 组合成运行时。

## 运行链路

1. CLI 解析用户命令，例如 `exec`、`tool`、`approvals`、`trace` 或交互式 prompt。
2. CLI 检查或启动 daemon，并读取 `~/.unio/daemon/instance.json` 中的 HTTP 地址。
3. CLI 调用 daemon 的 HTTP 端点，例如 `/exec`、`/tools/execute`、`/approvals`、`/traces/query`。
4. daemon 解析或创建 workspace session，并为每次执行创建 run、conversation 和 trace ID。
5. root agent 构造模型消息，将历史 transcript 和用户输入发送给 provider。
6. provider 返回文本或工具调用；daemon 对工具调用逐个执行。
7. 工具调用先进入 security precheck，根据权限模式决定允许、拒绝或等待审批。
8. daemon 持久化 run、transcript、approval grant 和 trace event。
9. CLI 将结果渲染为用户可读的状态、摘要、审批提示或 trace 时间线。

## Daemon API

daemon 使用 `axum` 暴露本地 HTTP API。当前主要路由包括：

- `GET /status`: 返回 daemon、session、审批、trace 和模型状态。
- `GET /models`: 返回当前模型 provider 摘要。
- `GET /sessions`: 列出 session。
- `POST /sessions/resolve`: 按 workspace 解析或创建 session。
- `POST /sessions/transcript`: 读取 session transcript。
- `POST /traces/query`: 按 trace ID 查询事件，可按 run ID 过滤。
- `POST /exec`: 执行一个用户 turn。
- `POST /tools/execute`: 直接执行工具。
- `GET /approvals`: 列出待审批工具调用。
- `GET /approvals/history`: 列出审批历史。
- `POST /approvals/resolve`: 处理审批通过或拒绝。

## Agent 与模型

`crates/agent` 定义 `RootAgent`、`SubAgent`、`SkillAgent` trait 和相关任务/结果类型。当前 root agent 在普通输入下调用 `ResolvedProvider`，在计划触发语句下生成 `PlanSpec`。模型调用包含系统消息、近期历史消息和当前用户输入。

`crates/model` 通过 `ProviderConfig` 解析配置，优先级为环境变量高于 `~/.unio/config.toml`。支持的 provider 形态包括：

- `mock`: 本地确定性 provider，用于开发测试。
- `openai-compatible`: 调用 `/chat/completions` 风格 API。
- `anthropic`: 调用 `/messages` 风格 API。

如果请求真实 provider 但缺少 API key，系统会 fallback 到 mock，并在状态中标记 `fallback_to_mock`。

## 工具与审批

`crates/tools` 提供内置工具注册表。工具定义包含名称、描述、能力类型和默认风险。执行前会构造 `ToolPrecheck`，交给 `crates/security` 决策。

权限模式包括：

- `Default`: 允许 workspace 内读取和 plan；写入、进程、网络和技能工具需要审批。
- `Auto`: 允许低风险 workspace 写入和可信低风险网络访问；更高风险操作需要审批。
- `FullTrust`: 直接允许。

默认模式下，写文件、执行进程、网络访问和 skill-tool 都会进入审批队列，用户可以通过 CLI/TUI 审批或拒绝。

## 存储与观测

`crates/storage` 使用 SQLite 保存：

- sessions
- runs
- approval grants

Transcript 使用每个 session 一个 JSONL 文件保存，消息粒度包括 user、assistant 和 tool。

`crates/observability` 使用 JSONL 保存 trace event。每个事件包含 trace ID、run ID、kind、message、可选 token usage 和记录时间。上下文预算根据 `context_ratio` 生成 `normal`、`summarize`、`compress`、`critical` 等等级事件。

## 架构原则

- CLI 不直接拥有模型调用、工具执行和持久化。
- 所有有风险工具操作都通过统一安全策略。
- 使用显式 ID 连接 session、run、conversation、agent、approval 和 trace。
- 使用 mock provider 保证没有外部凭据时仍可开发。
- 使用共享 protocol crate 保持 CLI 与 daemon 的数据契约一致。
