# 03 功能模块化介绍

## `apps/cli`

CLI 是用户入口，负责命令解析、交互式 TUI、slash command、文件引用补全、daemon 客户端调用和输出渲染。

主要能力：

- `exec`: 提交 prompt，创建或复用 session。
- `resume`: 查看最近 transcript。
- `sessions`: 列出 session。
- `skills`: 列出发现的技能。
- `models`: 查看模型状态。
- `status`: 查看 daemon 和运行状态。
- `trace`: 查询 trace。
- `tool`: 直接调用工具。
- `approvals`: 查看、通过、拒绝审批。
- `daemon start/status`: 管理本地 daemon。
- 无子命令时进入 TUI。

TUI 支持 `/` 命令提示、`@` workspace 文件引用、模型配置 wizard、审批快捷键和 trace/resume/status 等常用操作。

## `apps/daemon`

daemon 是运行时核心。它启动本地 HTTP 服务，创建 `DaemonState`，打开 SQLite store，初始化 JSONL trace store，并维护内存中的 pending approval queue。

主要职责：

- 提供本地 HTTP API。
- 解析和创建 session。
- 执行用户 turn。
- 调用 root agent。
- 执行模型返回的工具调用。
- 管理审批队列和审批历史。
- 持久化 transcript、run、approval grant。
- 写入 trace 和上下文预算事件。

daemon 也是 agent、tools、storage、observability 的组合层。

## `crates/core`

核心基础类型模块，负责：

- `SessionId`、`RunId`、`TraceId`、`ApprovalId`、`AgentId`。
- `AgentKind` 和 ID 前缀。
- `WorkspacePaths`，统一 user skills 与 workspace skills 路径。
- `UserPaths`，统一 `~/.unio` 下 daemon、sessions、transcripts、traces 路径。
- daemon instance 文件读写。

该模块尽量不承载业务逻辑，只提供跨 crate 共享的基础结构。

## `crates/protocol`

协议 crate 定义 CLI、daemon 和 agent 之间共享的数据结构，包括：

- `PermissionMode`
- `RunStage`
- session 解析和列表类型
- exec turn 请求/响应
- transcript message schema
- model status
- daemon status
- trace 查询和 trace event record
- tool execute 请求/响应
- approval list、resolve 和 history 类型

它是跨进程和跨模块契约的主要来源。

## `crates/agent`

agent crate 定义智能体抽象和当前 root agent runtime。

主要类型：

- `AgentRuntime`: 一次 agent 执行所需的 session、run、trace、输入、历史和权限模式。
- `AgentOutcome`: agent 输出、计划、工具调用、事件、provider 摘要和 token 信息。
- `RootAgent`、`SubAgent`、`SkillAgent` trait。
- `PlanSpec`、`PlanStep`、`StepExecutor`。

当前 root agent 会在需要规划时生成固定结构的计划，否则调用模型 provider。模型返回工具调用时，结果交给 daemon 继续处理。

## `crates/model`

模型模块负责 provider 抽象、配置解析和 API 适配。

主要能力：

- `ModelProvider` trait。
- `ModelRequest`、`ModelResponse`。
- `ProviderConfig` 从配置文件和环境变量解析 provider。
- `ResolvedProvider` 根据配置选择 mock、OpenAI-compatible 或 Anthropic。
- `MockModelProvider` 支持本地 deterministic 行为和测试用 mock tool request。
- `OpenAiCompatibleProvider` 调用 chat completions API。
- `AnthropicProvider` 调用 messages API。

配置文件路径是 `~/.unio/config.toml`，环境变量优先。

## `crates/tools`

工具模块定义工具注册表和实际执行逻辑。内置工具包括：

- `glob`: 按通配符查找文件。
- `grep`: 搜索文本。
- `read`: 读取 workspace 文件。
- `edit`: 替换文件中的精确文本片段。
- `write`: 创建或覆盖文件。
- `bash`: 执行受限命令，不允许 shell 组合语法。
- `fetch`: 只允许可信 URL。
- `plan`: 返回只读计划 JSON。
- `skill-tool`: 调用 skill agent。

工具执行时使用 `ToolExecutionContext` 提供 workspace root、user home 和权限模式。

## `crates/security`

安全模块提供统一审批策略。它不执行工具，只基于 `ToolPrecheck` 和 `PermissionMode` 返回：

- `Allow`
- `RequireApproval`
- `Deny`

能力类型包括 workspace 读取、workspace 写入、进程执行、网络访问、只读计划和 skill tool。该模块是工具风险决策的中心。

## `crates/skills`

技能模块负责从两个位置发现技能：

- `{workspace}/.unio/skills/`
- `~/.unio/skills/`

每个技能目录包含 `SKILL.md`。模块会读取首个非空行作为描述，并可将技能注入为可调用工具定义。`skill-tool` 执行时返回结构化结果，不回传完整私有 skill body。

## `crates/storage`

存储模块包含两类持久化：

- SQLite：session、run、approval grant。
- JSONL transcript：按 session 记录 user、assistant、tool message。

SQLite store 提供 session 解析、列表、run 插入、最新 context ratio 查询和审批历史查询。

## `crates/observability`

观测模块负责 trace event JSONL store 和上下文预算事件。

主要能力：

- 追加 trace event。
- 汇总最新 trace ID 和事件数量。
- 按 trace ID 查询事件。
- 根据 context ratio 生成 summary、compression、critical 相关事件。

这让一次执行结束后仍能追溯工具、审批、模型和上下文状态。
