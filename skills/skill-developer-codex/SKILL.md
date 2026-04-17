---
name: skill-developer-codex
description: |
  Codex skill 框架治理：路由、边界、owner/gate/overlay、维护规范与减少 token 消耗。
  Use when the request is about framework policy, overlap repair strategy, or skill-library self-optimization.
routing_layer: L0
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: preferred
short_description: Design and tune Codex skill routing/framework behavior
trigger_phrases:
  - skill框架
  - 边界重叠
  - 减少 token 消耗
  - framework 自优化
  - owner gate overlay
framework_roles:
  - planner
  - gate
framework_phase: 0
framework_contracts:
  emits_findings: true
  consumes_findings: false
  emits_execution_items: true
  consumes_execution_items: false
  emits_verification_results: false
metadata:
  version: "3.2.0"
  platforms: [codex]
  tags:
    - codex
    - skill-authoring
    - routing
    - trigger-debugging
    - skill-splitting
    - first-turn-routing
risk: low
source: local
allowed_tools:
  - shell
  - git
  - python
approval_required_tools:
  - git push
filesystem_scope:
  - repo
  - artifacts
network_access: conditional
artifact_outputs:
  - SESSION_SUMMARY.md
  - TRACE_METADATA.json
bridge_behavior: mobile_complete_once
---
- **Dual-Dimension Audit (Pre: Framework-Policy/Logic, Post: Sync-Health/Registry Results)** → `$execution-audit-codex` [Overlay]

# skill-developer-codex

This skill owns **Codex skill-framework design**: owner/gate/overlay policy,
boundary cleanup, trigger-surface tuning, session-start routing, and high-level
library self-optimization.

Check this skill early at **conversation start / first turn / 每轮对话开始** when
the request is about framework structure rather than one isolated file edit.

## When to use

- The user wants to redesign the Codex skill framework or route selection policy
- The task is **框架自优化 / 路由诊断 / 触发精准度优化 / 减少 token 消耗**
- The user asks how to handle **新增 skill / 新加入 skill / 维护规范 / 维护流程**
- The task is about **边界重叠 / 修改旧 skill / 顺手修旧 skill / 旧 skill 该不该拆**
- The user wants to decide owner vs gate vs overlay, or when to split vs extend an incumbent skill
- A skill is over-broad, misfiring, or weak at first-turn routing
- The problem is framework-level overlap between `skill-writer`, `skill-creator`, `skill-installer`, and `skill-routing-repair-codex`
- Best for requests like:
  - "优化整个 skill 框架"
  - "这个 Codex skill 到底该谁当 owner？"
  - "边界重叠怎么处理，先改旧 skill 还是新建？"
  - "把这个框架改得更快、更准、更省 token"

## Do not use

- The main job is a **post-task smallest-patch repair** after a miss → use [`$skill-routing-repair-codex`](/Users/joe/Documents/skill/skills/skill-routing-repair-codex/SKILL.md)
- The task is **concrete skill file authoring** rather than framework policy → use [`$skill-creator`](/Users/joe/Documents/skill/skills/.system/skill-creator/SKILL.md)
- The task is **new skill intake / install / relink / re-index** → use [`$skill-installer`](/Users/joe/Documents/skill/skills/.system/skill-installer/SKILL.md)
- The task is mainly **single-skill writing guidance** for description quality / token budget / packaging → use [`$skill-writer`](/Users/joe/Documents/skill/skills/skill-writer/SKILL.md)
- The task is Antigravity-only behavior or indexing → use `$skill-developer`

## Primary operating principle

This owner should behave like a **framework-control layer**:

1. tighten routing and boundary clarity before adding prose
2. keep the main thread to policy decisions, affected skills, and validation status
3. sink file-by-file wording churn into patches and sync outputs
4. if runtime policy permits, sidecar bounded read-only framework inspection
5. if spawning is blocked, preserve the same inspection slices in local-supervisor mode

## Main-thread compression contract

The main thread should contain only:

- framework problem statement
- owner/gate/overlay decision
- impacted skills
- validation result
- next repair step

## Runtime-policy adaptation

If bounded framework inspection benefits from parallelism and runtime policy permits:

- route it through [`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md)

If runtime policy does **not** permit spawning:

- keep the same inspection plan as a local-supervisor queue
- avoid narrating every wording change in the main thread

## Boundary map

Use this owner split:

- **`skill-developer-codex`** → framework policy, routing rules, overlap decisions, split strategy
- **`skill-routing-repair-codex`** → post-task smallest safe repair after a concrete miss
- **`skill-creator`** → create/update/split a specific Codex skill package
- **`skill-installer`** → import, normalize, link, and re-index a new skill
- **`skill-writer`** → wording, description budget, progressive disclosure, packaging advice

Default to **incumbent-first** repair:

1. extend the old skill if ownership did not really change
2. split only when owner / gate / overlay role changes, runtime assumptions differ, or discovery would become noisy
3. move optional detail into `references/` before creating a sibling skill

## Framework workflow

1. Extract **object / action / constraints / deliverable**.
2. Decide whether the problem is **policy**, **authoring**, **installation**, or **post-task repair**.
3. Decide owner vs gate vs overlay.
4. Tighten the discovery surface first:
   - `description`
   - `## When to use`
   - `## Do not use`
   - opening-turn note
5. Remove duplicated framework prose; keep one canonical source when possible.
6. Validate and sync.

## Validation

```bash
cd /Users/joe/Documents/skill
python3 scripts/check_skills.py --verify-codex-link
python3 scripts/check_skills.py --include-system --verify-codex-link
python3 scripts/sync_skills.py --apply
```

## Quality bar

Before finishing, verify:

- owner boundary is obvious in under 30 seconds
- first-turn wording is explicit when `session_start` is `preferred` or `required`
- description carries real trigger phrasing users will say
- optional examples live in `references/` instead of bloating `SKILL.md`
- the framework is more precise, faster to scan, and cheaper to load than before
- **Superior Quality Audit**: For framework-level redesigns, trigger `$execution-audit-codex` to verify against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).

## Trigger examples
- "强制进行 Codex 框架深度审计 / 检查路由策略与同步状态。"
- "Use $execution-audit-codex to audit this framework-policy for sync-health idealism."
