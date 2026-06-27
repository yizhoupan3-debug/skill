---

allowed_tools:
- shell
- git
- python
approval_required_tools:
- git push
description: Design and tune cross-host skill routing and framework behavior
metadata:
  platforms:
  - supported
  tags:
  - multi-host
  - host-agnostic
  - skill-authoring
  - routing
  - trigger-debugging
  - skill-splitting
  - first-turn-routing
  version: '3.3.1'
name: skill-framework-developer
scene: general
network_access: conditional
risk: low
routing_gate: none
routing_layer: L0
routing_owner: owner
routing_priority: P1
session_start: preferred
short_description: Design and tune cross-host skill routing and framework behavior
source: project
trigger_hints:
- owner / gate / overlay
- skill 库维护
- skill 框架治理
- skill 路由
- skill 边界
- 同步健康
- 同步状态
- 框架策略
- 框架自优化
- 注册表
- 维护规范
- 路由系统
- 路由表
- 路由诊断
- 边界重叠
---
**Dual-Dimension Audit (Pre: Framework-Policy/Logic, Post: Sync-Health/Registry Results) → runtime verification gate**

# skill-framework-developer

This skill owns **shared skill-framework design and maintenance**:
owner/gate/overlay policy, boundary cleanup, trigger tuning, session-start
routing, validation, sync health, registry drift cleanup, and library
self-optimization.

Check this skill early at **conversation start / first turn / 每轮对话开始** when
the request is about framework structure rather than one isolated file edit.

## When to use

- The user wants to redesign the skill framework or route selection policy
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
  - "这个 skill 到底该谁当 owner？"
  - "边界重叠怎么处理，先改旧 skill 还是新建？"
  - "把这个框架改得更快、更准、更省 token"

## Do not use

- The task is **concrete single skill package creation/update** after the boundary is known → do it directly in this skill's workflow
- The task is **new skill intake / install / relink / re-index** → do it directly in this skill's workflow

## Primary operating principle

This owner should behave like a **framework-control layer**:

1. tighten routing and boundary clarity before adding prose
2. keep the main thread to policy decisions, affected skills, and validation status
3. sink file-by-file wording churn into patches and sync outputs
4. if runtime policy permits, sidecar bounded read-only framework inspection
5. if spawning is blocked, preserve the same inspection slices in local-supervisor mode

**与仓库 `AGENTS.md` 对齐（减法 / 第一性原理）**：先回答「只改变什么、明确不做什么、现有 owner 是谁」，再动路由或新增 skill；默认 **incumbent-first**、**detail 下沉 `references/`**，禁止用更多入口/抽象掩盖不确定。用户口头说「减法视角」「第一性原理」时，仍优先命中本 owner（长尾同义词见 [`references/trigger-hints-long.md`](references/trigger-hints-long.md)），但结论必须落在可验证的 registry/skill 改动上。

## Main-thread compression contract

The main thread should contain only:

- framework problem statement
- owner/gate/overlay decision
- impacted skills
- validation result
- next repair step

## Runtime-policy adaptation

If bounded framework inspection benefits from parallelism and runtime policy permits:

- route it through [`agent-swarm-orchestration` (delegation gate)](../agent-swarm-orchestration/SKILL.md)

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
- **`skill-creator`** [archived, merged here] → create/update/split a specific skill package
- **`skill-installer`** [archived, merged here] → import, normalize, link, and re-index a new skill

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
cd "<repo-root>"
cargo run --manifest-path core/router-rs/Cargo.toml -- framework skills refresh --framework-root "$PWD" --write
```

For local high-output runs, prefer the raw command (no RTK wrapper).

If validation tools fail, report the failing command, the affected generated or
runtime surface, and the smallest next repair step. Do not claim sync health
from prose-only inspection when a compiler, registry, or contract check was
available but failed or was not run.

## References

- [references/skill-maintenance-modes.md](references/skill-maintenance-modes.md)
- [references/trigger-hints-long.md](references/trigger-hints-long.md)

## Quality bar

Before finishing, verify:

- owner boundary is obvious in under 30 seconds
- first-turn wording is explicit when `session_start` is `preferred` or `required`
- description carries real trigger phrasing users will say
- optional examples live in `references/` instead of bloating `SKILL.md`
- the framework is more precise, faster to scan, and cheaper to load than before
- **Superior Quality Audit**: For framework-level redesigns, apply the runtime verification gate (see [`skills/SKILL_FRAMEWORK_PROTOCOLS.md`](../SKILL_FRAMEWORK_PROTOCOLS.md)) and verify against its “Superior Quality Bar / verification gate” criteria.

## Trigger examples
- "强制进行框架深度审计 / 检查路由策略与同步状态。"
- "Use the runtime verification gate to audit this framework-policy for sync-health idealism."

## Exit Criteria

- MANIFEST/RUNTIME 已同步（framework skills validate 通过）
- 路由测试通过（cargo test --test routing_tests）
- 新增/修改的 skill 可被路由命中
