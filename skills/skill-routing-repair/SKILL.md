---
name: skill-routing-repair
description: |
  Patch routing misses after a task with the smallest safe skill fix.
  Use when the task is a post-task repair request such as 顺手修一下 skill, 这次为什么没触发,
  新加入 skill 没有走维护流程, 边界重叠后修改旧 skill, or gate 顺序修复.
  This is for concrete miss repair, not full framework redesign.
routing_layer: L0
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
short_description: Patch routing misses with the smallest safe skill fix
trigger_hints:
  - skill-routing-repair
  - 路由修复
  - 触发修复
  - 自提升
  - 顺手更新 skill
  - 顺手修一下 skill
  - 顺手把这个 skill 修一下
  - 这次为什么没触发
  - 为什么没有触发
  - 是不是路由问题
  - 路由问题吗
  - 路由没触发
  - 修复这次 miss
  - 以后别再选错
  - 做完后更新 skill
  - 复盘这次 miss
  - gate 应该先触发
allowed_tools:
  - shell
  - git
  - python
approval_required_tools:
  - git push
artifact_outputs:
  - SESSION_SUMMARY.md
  - TRACE_METADATA.json
metadata:
  version: "1.2.0"
  platforms: [codex]
  tags:
    - codex
    - routing-repair
    - trigger-repair
    - self-improvement
    - post-task-maintenance
    - overlay
risk: low
source: local
---
- **Dual-Dimension Audit (Pre: Miss-Root/Logic, Post: Patch-Stability/Sync Results)** → `$execution-audit` [Overlay]

# skill-routing-repair

This skill owns **post-task routing repair**: turn one observed miss into the
smallest safe library patch.

## When to use

- A finished or in-progress task exposed **Routing Drift**, **Struggle**, or **Missing Gate/Context**
- The user asks: "顺手把这个 skill 修一下", "这次为什么没触发", or "以后别再选错"
- `audit_evolution.py` or `.evolution_journal.jsonl` shows a recurring reroute pattern
- A gate should have fired first but did not
- A newly added skill did not enter the expected **维护规范 / 维护流程 / 自优化** path
- The right fix is to tighten an incumbent skill because of **边界重叠 / 修改旧 skill / 顺手修旧 skill / 旧 skill** confusion
- A trigger phrase is too weak for natural-language discovery

## Do not use

- The task is broad framework redesign or x-round framework optimization → use [`$skill-framework-developer`](/Users/joe/Documents/skill/skills/skill-framework-developer/SKILL.md)
- The task is direct creation or heavy rewriting of a skill package with no concrete miss anchor → use [`$skill-creator`](/Users/joe/Documents/skill/skills/.system/skill-creator/SKILL.md)
- The task is import/install/relink of a new skill → use [`$skill-installer`](/Users/joe/Documents/skill/skills/.system/skill-installer/SKILL.md)
- The task is Antigravity-only self-improvement → use `$skill-developer`

## Repair loop

1. Identify the miss: wrong owner, missing gate, weak trigger, or bad overlap.
2. Prefer the **smallest safe fix**:
   - patch description / headings first
   - patch opening-turn wording next
   - patch routing docs only if discovery really changed
   - split only if the incumbent skill cannot stay precise
- **Superior Quality Audit**: For critical routing repairs, trigger `$execution-audit` to verify against [Superior Quality Bar](../execution-audit/references/superior-quality-bar.md).
3. Re-run validation and sync.

## Validation

```bash
cd /Users/joe/Documents/skill
python3 scripts/check_skills.py --verify-codex-link
python3 scripts/check_skills.py --include-system --verify-codex-link
python3 scripts/sync_skills.py --apply
```

For local high-output verification runs, follow [`RTK.md`](/Users/joe/Documents/skill/RTK.md) and prefer the equivalent `rtk ...` wrapper when compact output is sufficient.

## Quality bar

A good repair should make the next similar task route correctly **without**
creating a noisy new sibling skill.
