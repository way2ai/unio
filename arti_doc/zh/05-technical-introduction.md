# 05 技术介绍

## 技术栈

Unio 使用 Rust workspace 组织。主要依赖包括：

- `tokio`: async runtime。
- `axum`: daemon HTTP API。
- `clap`: CLI 参数解析。
- `ratatui` 和 `crossterm`: 终端 TUI。
- `reqwest`: 模型 API 和可信 URL fetch。
- `serde`、`serde_json`、`toml`: 配置和协议序列化。
- `rusqlite`: SQLite 持久化。
- `uuid`: session、run、trace、approval、agent ID。
- `chrono`: 时间戳。
- `anyhow`、`thiserror`: 错误处理。

## 配置模型

模型配置由 `crates/model` 解析。文件配置位于：

```text
~/.unio/config.toml
```

环境变量优先于配置文件。provider 选择逻辑：

- `UNIO_MODEL_PROVIDER=openai-compatible` 或 `openai`: 使用 OpenAI-compatible provider。
- `UNIO_MODEL_PROVIDER=anthropic`: 使用 Anthropic provider。
- 未配置或未知 provider：使用 mock。
- 请求真实 provider 但缺少 API key：fallback 到 mock，并记录 `fallback_to_mock=true`。

## 数据模型

Unio 的核心 ID 都带稳定前缀：

- `session_...`
- `run_...`
- `trace_...`
- `approval_...`
- `agent_root_...`
- `agent_planner_...`
- `agent_sub_...`
- `agent_skill_...`

这些 ID 让 CLI 输出、daemon API、SQLite 记录、JSONL transcript 和 trace event 可以互相连接。

## 持久化布局

用户级状态位于：

```text
~/.unio/
  daemon/
    instance.json
    logs/
  sessions/
    state.db
    transcripts/
  traces/
    events.jsonl
  config.toml
```

SQLite 表包括：

- `sessions`: session 元数据、workspace root、权限模式和最新 run。
- `runs`: prompt、final text、trace、provider、model、token 和 context ratio。
- `approval_grants`: 审批授权或拒绝审计记录。

Transcript 和 trace 使用 JSONL，便于追加写入和人工检查。

## 工具安全模型

工具执行拆成两个步骤：

1. `crates/tools` 根据工具定义和参数生成 `ToolPrecheck`。
2. `crates/security` 根据权限模式返回安全决策。

这种设计让工具实现保持简单，也让风险策略集中维护。

能力类型包括：

- workspace 读取
- workspace 写入
- 进程执行
- 网络访问
- 只读计划
- skill tool

默认策略保守：读 workspace 和 plan 可直接执行，有副作用的操作进入审批。

## 模型工具调用

模型 provider 返回 `ModelResponse`，其中可以包含文本和 `ToolCall`。daemon 负责执行工具调用，而不是 provider 或 agent 直接执行。这保持了模型层和工具安全层的分离。

OpenAI-compatible provider 会把 Unio 工具定义映射为 function tools。Anthropic provider 会解析 `tool_use` content block。mock provider 支持特殊输入，例如 `mock-tool` 和 `mock-usage`，用于测试工具调用和上下文预算。

## 上下文预算

当前实现使用 token usage 计算 `context_ratio`，上限按 128,000 token 估算。观测模块根据比例生成事件：

- `< 0.70`: normal。
- `>= 0.70`: summarize。
- `>= 0.85`: compress。
- `>= 0.90`: critical。

daemon 在高上下文比例且任务较大时会要求先进行 compaction，避免继续增加长任务负载。

## 测试组织

测试位于各 crate 源码旁的 `#[cfg(test)]` 模块中，覆盖：

- ID 前缀和路径解析。
- 协议 schema。
- session、run、approval 和 transcript 持久化。
- trace event 和上下文预算事件。
- 权限策略矩阵。
- 工具注册和工具执行。
- 技能发现和 skill-tool 输出。
- mock provider、配置覆盖和 provider fallback。
- CLI 输出格式、slash command、文件引用补全和 TUI 辅助逻辑。
- daemon session、exec、approval、trace 和 transcript API 行为。

## 当前限制

- daemon 是本地 HTTP runtime，尚未体现远程多租户或分布式部署模型。
- `bash` 工具是简单空白分割命令，不支持 shell 组合语法。
- `fetch` 只允许少数可信 URL 前缀。
- skill execution 当前返回结构化摘要，不是真正执行复杂脚本。
- sub-agent 当前有 mock 实现，规划和并行执行能力仍是基础形态。
- 安装和发布说明依赖 release artifact 完善程度。

## 维护建议

- 新增 CLI 命令时，同步更新 `docs/src/cli.md`、中文文档和本报告使用指南。
- 新增工具或审批规则时，同步更新工具、安全、架构章节。
- 修改模型 provider 或配置优先级时，同步更新 README、mdBook 和本报告技术介绍。
- 修改存储 schema 或 trace event 时，同步更新架构和技术介绍。
- 有意义的代码、行为、架构、工具、工作流或文档变更都记录到 `CHANGE.md`。
