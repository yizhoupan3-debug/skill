---
name: skill-framework-developer
description: |
  Skill 框架治理：路由、边界、owner/gate/overlay、维护规范、sync health、
  registry drift cleanup 与减少 token 消耗。
  Use when the request is about framework policy, routing-system review, skill-boundary audit,
  overlap repair strategy, skill-library maintenance/self-optimization, validation,
  sync checks, registry drift cleanup, or feedback that a domain skill is "不好用".
routing_layer: L0
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
user-invocable: false
disable-model-invocation: true
short_description: Design and tune Codex skill routing/framework behavior
trigger_hints:
  - skill-framework-developer
  - skill框架
  - 路由系统
  - skill 边界
  - framework review
  - 路由 review
  - routing framework
  - 边界重叠
  - 减少 token 消耗
  - framework 自优化
  - skill系统
  - 行为驱动
  - 多余入口
  - 不必要抽象
  - 减法视角
  - 第一性原理
  - runtime 轻量化
  - 兼容层
  - 胶水层
  - 减少入口
  - 减入口
  - 不损害功能
  - skill 维护
  - sync health checks
  - registry drift cleanup
  - skill library maintenance
  - 写一个 skill
  - 批量规范 skill
  - 路由没触发
  - validate skills
  - owner gate overlay
  - skill 不好用
  - skill不好用
  - 持续优化 skill
  - 外部调研优化 skill
  - 科研 skill 不好用
  - 写作 skill 不好用
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
  version: "3.3.0"
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
  - EVIDENCE_INDEX.json
bridge_behavior: mobile_complete_once

---
- **Dual-Dimension Audit (Pre: Framework-Policy/Logic, Post: Sync-Health/Registry Results)** → runtime verification gate

# skill-framework-developer

This skill owns **shared skill-framework design and maintenance**:
owner/gate/overlay policy, boundary cleanup, trigger tuning, session-start
routing, validation, sync health, registry drift cleanup, and library
self-optimization.

Check this skill early at **conversation start / first turn / 每轮对话开始** when
the request is about framework structure rather than one isolated file edit.

## When to use

- The user wants to redesign the Codex skill framework or route selection policy
- The user wants routine skill-library validation, sync checks, or drift cleanup
- The task is **框架自优化 / 路由诊断 / 触发精准度优化 / 减少 token 消耗**
- The user asks how to handle **新增 skill / 新加入 skill / 维护规范 / 维护流程**
- The task is about **边界重叠 / 修改旧 skill / 顺手修旧 skill / 旧 skill 该不该拆**
- The user wants to decide owner vs gate vs overlay, or when to split vs extend an incumbent skill
- A skill is over-broad, misfiring, or weak at first-turn routing
- The user says a domain skill is not useful, too generic, or needs continuous optimization
- The task is one of the framework maintenance modes: single-skill wording, batch normalization, route-miss repair, or external skill ecosystem scouting
- Best for requests like:
  - "优化整个 skill 框架"
  - "科研相关 skill 太不好用了，持续优化，允许外部调研"
  - "写作 skill 还是不好用，帮我继续收紧"
  - "这个 Codex skill 到底该谁当 owner？"
  - "边界重叠怎么处理，先改旧 skill 还是新建？"
  - "把这个框架改得更快、更准、更省 token"

## Do not use

- The task is **concrete single skill package creation/update** after the boundary is known → use [`$skill-creator`](/Users/joe/Documents/skill/skills/.system/skill-creator/SKILL.md)
- The task is **new skill intake / install / relink / re-index** → use [`$skill-installer`](/Users/joe/Documents/skill/skills/.system/skill-installer/SKILL.md)

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

- route it through [`runtime delegation gate`](runtime policy)

If runtime policy does **not** permit spawning:

- keep the same inspection plan as a local-supervisor queue
- avoid narrating every wording change in the main thread

## Mode map

Use this owner split:

- **`skill-framework-developer`** → framework policy, routing rules, overlap decisions, split strategy
- **`single-skill wording pass`** → description quality, trigger hints, token budget, and boundary language for one skill
- **`batch wording normalization`** → consistent shape across many skill files
- **`miss repair`** → smallest safe route repair after a concrete miss, plus regression case
- **`external scout`** → external skill ecosystem benchmarking when the output is local framework guidance
- **`skill-creator`** → create/update/split a specific Codex skill package
- **`skill-installer`** → import, normalize, link, and re-index a new skill

Default to **incumbent-first** repair:

1. extend the old skill if ownership did not really change
2. split only when owner / gate / overlay role changes, runtime assumptions differ, or discovery would become noisy
3. move optional detail into `references/` before creating a sibling skill

## Framework workflow

1. Extract **object / action / constraints / deliverable**.
2. Decide whether the problem is **policy**, **authoring**, **installation**, `single-skill wording pass`, `batch wording normalization`, `miss repair`, or `external scout`.
3. Decide owner vs gate vs overlay.
4. Tighten the discovery surface first:
   - `description`
   - `## When to use`
   - `## Do not use`
   - opening-turn note
5. Remove duplicated framework prose; keep one canonical source when possible.
6. Validate, sync, and remove registry drift.

## Validation

```bash
cd /Users/joe/Documents/skill
cargo run --manifest-path scripts/skill-compiler-rs/Cargo.toml -- \
  --skills-root skills \
  --source-manifest skills/SKILL_SOURCE_MANIFEST.json \
  --apply
```

For local high-output runs, follow [`RTK.md`](/Users/joe/Documents/skill/RTK.md) and prefer the corresponding `rtk ...` wrapper when raw output is not required.

## References

- [references/skill-maintenance-modes.md](references/skill-maintenance-modes.md)

## Quality bar

Before finishing, verify:

- owner boundary is obvious in under 30 seconds
- first-turn wording is explicit when `session_start` is `preferred` or `required`
- description carries real trigger phrasing users will say
- optional examples live in `references/` instead of bloating `SKILL.md`
- the framework is more precise, faster to scan, and cheaper to load than before
- **Superior Quality Audit**: For framework-level redesigns, apply the runtime verification gate to verify against [Superior Quality Bar](runtime verification criteria).

## Trigger examples
- "强制进行 Codex 框架深度审计 / 检查路由策略与同步状态。"
- "Use the runtime verification gate to audit this framework-policy for sync-health idealism."
