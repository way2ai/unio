# ADR 0003：存储合同

## 状态

已采纳

## 决策

第一版存储合同：

- SQLite 保存 session、run、配置、update cache、聚合索引。
- JSONL 保存 message 级 transcript、tool call、trace event。
- transcript 不再使用 turn 级 `{prompt, final_text}` 记录。
- trace 使用 `trace_id` 和 `run_id` 关联。

## 影响

长会话恢复、上下文压缩和统计聚合可以从第一版围绕 message 级数据设计，不需要再从旧 turn 级 schema 反推。
