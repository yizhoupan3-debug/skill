---
name: security-threat-model
description: |
  Repository-grounded threat modeling for applications, services, MCP servers,
  APIs, and agent systems. Use when the user wants trust boundaries, assets,
  entry points, attacker goals, abuse paths, misuse cases, data flows, and
  prioritized mitigations for a codebase or system slice, rather than code-
  level bug hunting. 适用于“威胁建模”“攻击面分析”“trust boundary”“abuse path”
  “资产梳理”“AppSec 风险盘点”“给仓库做安全模型”这类请求；不要用于普通代码
  review、泛泛架构总结或非安全设计讨论。
risk: medium
source: community-adapted
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - security
    - threat
    - model
---
# security-threat-model

This skill owns evidence-based threat modeling for code repositories, not
generic security checklists.

Distinction from `security-audit`:
- `security-threat-model` → system-level attack paths, trust boundaries, assets
- `security-audit` → code implementation and defense gaps

## When to use

- The user wants trust boundaries, attack surfaces, or abuse path analysis for a system
- The task involves threat modeling for applications, APIs, MCP servers, or agent systems
- The user says "威胁建模", "attack surface", "trust boundary", "abuse path", "资产梳理"
- The user wants attacker-goal-driven risk enumeration rather than code-level bug hunting
- The task requires cataloging assets, entry points, and attacker capabilities

## Do not use

- 普通代码 review
- 只看某个 bug 或某段代码是否写对
- 泛泛的架构总结
- 不涉及安全目标的设计讨论
- 只是做 dependency audit、secret scan 或 lint
