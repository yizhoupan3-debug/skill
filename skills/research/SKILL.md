---
description: '统一科研前门：内部自动路由到 paper-workbench（手稿/审稿/润色）和 research-workspace 两个 lane。单一入口，内部 lane 分发。'
metadata:
  platforms:
  - supported
  tags:
  - research
  - paper
  - manuscript
  - theory
  - experiment
  version: '1.0.0'
name: research
scene: research
risk: low
routing_gate: none
routing_layer: L2
routing_owner: owner
routing_priority: P2
session_start: preferred
short_description: 统一科研前门——手稿审改、科学探索（研究工作区由独立技能 research-workspace 提供）
trigger_hints:
- 科研
- 研究
- research
- 学术调研
- 文献综述
- 实验设计
- 论文审稿
- 数学建模
- 研究方向
- 理论背景
- ablation
- benchmark
- 手稿
- paper
- 实验方案
- 科学计算
- 实验分析
- 研究问题
- 文献调研
---
# Research — 统一科研前门

本 skill 是 **科研工作的统一 L2 前门**。它根据用户意图分发到对应 lane：

- **paper-workbench**：手稿审阅、返修、润色、投稿策略、rebuttal

> 研究平台工作区（研究方向初始化、claim/假设管理、实验记录、barrier 升级）由独立一等技能 `$research-workspace` 提供，见 [`../research-workspace/SKILL.md`](../research-workspace/SKILL.md)。

## When to use（入口条件）

- 用户想做任何涉及 **学术/工程科研** 的事情
- 用户的问题涉及 paper-workbench / research-workspace 中多个阶段
- 用户不确定自己是"查文献"还是"做实验"还是"写论文"，从此开始

## 内部路由逻辑

检测用户输入中的意图信号，分发到对应 lane：

| 信号关键词 | Lane | 说明 |
|------------|------|------|
| 论文、审稿、改稿、润色、能不能投、R&R、rebuttal、cover letter、顶刊、顶会、投稿 | **paper-workbench** | 手稿全流程 |

> 研究方向初始化、claim 管理、假设跟踪、实验记录、smoke test、瓶颈研究等请直接使用独立技能 `$research-workspace`。

**fallback（意图不明朗）**：输出一个简短的两行路由菜单让用户选择，不假设、不猜测。

## 关联 skill

本前门下辖以下内部 lane（非独立路由入口，仅为用途分组）：

- [paper-workbench/SKILL.md](paper-workbench/SKILL.md) — 手稿前门（保留独立 framework contracts + flags）

> **研究平台工作区**（研究方向初始化、claim/假设管理、实验记录、barrier 升级）现已独立为 L2 一等技能 `$research-workspace`。

外部调用的 L3/L4 工具 skill：

- `$citation-management` — 引用格式核查（L3）
- `$experiment-reproducibility` — 可复现性管理（L3）
- `$deep-search` (gate=approve) — 通用 Web 深度调研（L3）
- `$statistical-analysis` — 统计方法选型与解读（L4）
- `$math-verify` — 对抗式数学推导验证（L4）
- `$math-explore` — 数学性质探索发现（L4）

上游 skill（选题/澄清）：

- `$good-question` — 选题尖锐化（用户有模糊兴趣但无具体问题时）
- `$deepinterview` — 需求收敛（用户需求不清时）

## Verification and failure contract

本前门的验证合约由各 lane 自行定义。所有 lane 输出统一使用结构化卡片格式：

```text
mode: <当前 lane 名称>
blocker: <当前阻塞项或空>
next_step: <下一步最小可执行动作>
handoff_evidence: <向下游传递的结构化上下文>
```

## Lane handoffs（跨 lane 传递协议）

- → paper-workbench：传递 `experiment_design` + `code_verification` + `reproducibility` + `math_results` 和 `language_register`。
- → research-workspace：研究工作区管理（方向初始化/claim/实验记录/barrier 升级）请直接调用独立一等技能 `$research-workspace`，不通过本前门路由。

## Cross-references

- **内部 lane 路由协议**：[references/research-lane-routing.md](references/research-lane-routing.md) — 内部分发规则、math 分工线、scenario routing table
- **学术来源检索**：[references/academic-sources.md](references/academic-sources.md) — arXiv / OpenAlex / CrossRef / PubMed / DOAJ API
- **手稿 prose chain**：[paper-workbench/references/prose-chain-contract.md](paper-workbench/references/prose-chain-contract.md)
- **旧路径映射**：[../LEGACY_MAP.md](../LEGACY_MAP.md)

## Hard constraints

- 不要跳过 lane 分发直接做子 lane 的工作
- 如果意图不明朗，给出路由菜单让用户选择
- 手稿工作必须路由到 paper-workbench lane（保留独立 contracts）
- Keep citation formatting out of this front door; hand it to `$citation-management`
- For reproducibility-only tasks, hand off to `$experiment-reproducibility`
