---
name: ajs-management-world
description: '管理世界 期刊论文专版——模板、格式、审稿标准、rebuttal、投稿。需配合 paper-workbench 做主路由调度。'
metadata:
  tags: [journal, ajs, 管理世界, conference]
  platforms: [supported]
  version: "1.0.0"
routing_layer: L3
routing_owner: owner
routing_priority: P2
trigger_hints:
  - 管理世界
  - management world
  - ajs-management-world
  - 管理世界 论文
  - 管理世界 投稿
  - 管理世界 审稿
---

# ajs-management-world — 管理世界论文专版

管理世界 期刊论文全流程支持。**不独立做路由，由 `$paper-workbench` 主路由调度后关联使用。**

## 能力范围

- 期刊/会议特定格式模板与检查
- 审稿标准理解与针对性修改
- Rebuttal 写作指导
- Camera-ready 准备
- 期刊/会议的投稿流程

## 使用方式

当通过 `$paper-workbench` 进入论文工作流时，本技能提供 venue 专版约束和模板。

## 相关技能

- [`paper-workbench`](../paper-workbench/SKILL.md) — 通用论文工作台（主路由入口）
