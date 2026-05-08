# ADR 0003: 存储合同

## Status

Accepted

## Decision

第一版存储合同：

- SQLite 保存 session、run、配置、update cache、聚合索引。
- JSONL 保存 message-level transcript、tool call、trace event。
- transcript 不再使用 turn-level `{prompt, final_text}` 记录。
- trace 使用 `trace_id` 和 `run_id` 关联。

## Consequences

长会话恢复、上下文压缩和统计聚合可以从第一版围绕 message-level 数据设计，不需要再从旧 turn-level schema 反推。
