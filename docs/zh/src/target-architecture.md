# 目标架构

Unio 的目标是以 daemon 为后端的智能 agent 系统，用户侧只有一个入口：`unio`。

## 混合 CLI

- 裸 `unio` 打开混合 CLI/TUI 界面。
- 脚本化命令仍然可用：`exec`、`status`、`resume`、`trace`、`approvals`、`skills`、`models`、`tool` 和 `daemon`。
- 混合界面负责提示编辑、斜杠建议、文件引用、审批热键、状态显示、追踪时间线和上下文指示器。
- CLI/混合界面不直接调用模型或执行工具，而是向 daemon 提交请求。

## 守护进程

- daemon 是 session、会话、run、审批、事件广播、存储、追踪持久化和工具执行的运行时所有者。
- 它协调 agent 执行，使 CLI 行为保持协议化。

## Agent

- Root agent 决定是直接回答、调用工具、调用 planner 还是分发 sub-agent。
- Planner 是只读的，返回 `PlanSpec`。
- Sub-agent 和 skill-agent 使用独立的 `agent_id` 和隔离的上下文，返回结构化结果而非原始上下文转储。
- 上下文预算记录 token 使用量和上下文比例，设有警告和压缩阈值。

## 工具与安全

- 工具通过注册表暴露，并通过统一契约执行。
- 执行前运行安全预检。
- `default`：允许只读和计划工具；写、进程、网络和 skill-tool 操作需要审批。
- `auto`：低风险工作区操作可自动运行；高风险操作需要审批。
- `full-trust`：直接允许工具执行。

## 技能

- 从 `{workspace}/.unio/skills/` 和 `~/.unio/skills/` 发现技能。
- 每个技能目录必须包含 `SKILL.md`。
- 技能执行通过 `skill-tool` 和 skill-agent 完成，root agent 不接收完整的技能正文。

## 存储与可观测性

- SQLite 存储 session、run、审批授权、模型配置、更新缓存和聚合索引。
- JSONL 存储 message 级 transcript 记录和追踪事件。
- Trace 记录通过 `trace_id` 关联模型调用、工具、审批、token 使用量、费用和上下文比例。
