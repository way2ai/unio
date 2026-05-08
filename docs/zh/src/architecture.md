# 架构

Unio 是一个以本地 daemon 为核心的 agent 系统。用户通过 `unio` 混合 CLI 或脚本化子命令与系统交互。daemon 拥有运行时状态、session/run 生命周期、审批、工具、存储和追踪持久化。

## 工作区结构

```text
apps/
  cli/       用户侧 unio 命令和混合终端界面
  daemon/    session、run、审批、工具、追踪的本地运行时所有者

crates/
  protocol/       CLI/daemon/agent 协议类型
  core/           ID、路径、元数据、共享工具
  agent/          root agent、planner、sub-agent、skill-agent 契约
  model/          OpenAI 兼容、Anthropic 和 mock provider
  tools/          工具注册表和执行契约
  security/       审批策略和风险预检
  skills/         skill 发现和 skill-tool 执行
  observability/  追踪和上下文事件
  storage/        SQLite 元数据和 JSONL transcript/trace 存储
```

## 主执行流

```text
用户
  -> 混合 CLI
  -> daemon
  -> root agent
  -> planner? -> sub-agent/tool?
  -> daemon
  -> 混合 CLI
  -> 用户
```

## ID 模型

- `session_id`：长期存在的工作区会话。
- `conversation_id`：一次用户请求链。
- `run_id`：一次 agent 执行。
- `agent_id`：root、planner、sub-agent 或 skill-agent 实例。
- `trace_id`：可观测性关联 ID。

## 当前契约

- `apps/cli` 仅负责用户交互。
- `apps/daemon` 负责运行时编排和持久化。
- `crates/security` 决定允许、拒绝或需要审批的结果。
- `crates/tools` 仅在安全预检后执行工具。
- `crates/storage` 持久化 SQLite 记录和 message 级 JSONL transcript。
- `crates/observability` 记录追踪和上下文事件。

另请参阅：

- [混合文件引用](hybrid-file-references.md)
- [混合斜杠命令](hybrid-slash-commands.md)
- [混合输入编辑](hybrid-input-editing.md)
