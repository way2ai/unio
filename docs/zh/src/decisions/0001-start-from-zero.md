# ADR 0001：从零开始重构

## 状态

已采纳

## 背景

Unio 旧实现已经具备 CLI、daemon、agent、skills、security、trace、session store 等能力，但模块命名、边界和数据模型来自早期演进。继续在旧结构上小修会让长期架构被历史路径牵引。

## 决策

新系统按从零开始设计处理：

- 新架构以领域 crate 为边界。
- 旧 `engine/*` 不再是目标结构。
- 不保证旧 session、trace、`.skills` 兼容。
- 文档只描述新系统目标和新骨架。

## 影响

- 可以重新定义协议、数据 schema、权限策略和 crate 边界。
- 后续实现可以更直接地围绕 daemon、agent、tools、security、storage 分工。
- 旧代码会短期并存，但不再决定目标架构。
