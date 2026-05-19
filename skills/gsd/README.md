# GSD Skills - Hot Manifest

## S / M / L 档位（叙事深度）

| 档位 | 何时用 | 工件 |
|------|--------|------|
| **S** | 单文件/单 bug，用户已给清验收 | 口头 goal + 1 条验证命令即可；可跳过完整 ROADMAP |
| **M** | 单模块特性（默认） | `REQUIREMENTS.md` + `ROADMAP.md` + `WAVE_STATE.json` |
| **L** | 跨模块/多宿主 harness 变更 | 上列 + ADR + `RISK_REGISTER.md` + 全量 `cargo test` |

**Plan 正交**：Cursor `.cursor/plans/*.plan.md` 与 `artifacts/current/<task>/ROADMAP.md` 互指；ROADMAP 为 execute 机读真源（见 `skills/gsd/plan-phase/SKILL.md`）。

## Routing

Skills are registered in `SKILL_ROUTING_RUNTIME.json` (L0-L2 hot skills) and `SKILL_MANIFEST.json` (full cold manifest).

## Skills

### gsd (L0 Framework Command)

**Path**: `skills/gsd/SKILL.md`
**Owner**: owner
**Gate**: none
**Priority**: P1
**Invocable**: Yes (`/gsd`, `/gsd-new-project`, `/gsd-plan-phase`, `/gsd-execute-phase`, `/gsd-verify-work`, `/gsd-discuss-phase`, `/gsd-ship`)
**Platforms**: [desktop-mcp, cli, codex, cursor]

### gsd-new-project (L1)

**Path**: `skills/gsd/new-project/SKILL.md`
**Owner**: owner
**Gate**: none
**Priority**: P1
**Invocable**: Yes (`/gsd-new-project`)
**Platforms**: [desktop-mcp, cli, codex, cursor]

### gsd-plan-phase (L1)

**Path**: `skills/gsd/plan-phase/SKILL.md`
**Owner**: owner
**Gate**: evidence
**Priority**: P1
**Invocable**: Yes (`/gsd-plan-phase`)
**Platforms**: [desktop-mcp, cli, codex, cursor]

### gsd-execute-phase (L1)

**Path**: `skills/gsd/execute-phase/SKILL.md`
**Owner**: owner
**Gate**: evidence
**Priority**: P1
**Invocable**: Yes (`/gsd-execute-phase`)
**Platforms**: [desktop-mcp, cli, codex, cursor]

### gsd-verify-work (L1)

**Path**: `skills/gsd/verify-work/SKILL.md`
**Owner**: owner
**Gate**: evidence
**Priority**: P1
**Invocable**: Yes (`/gsd-verify-work`)
**Platforms**: [desktop-mcp, cli, codex, cursor]

### gsd-discuss-phase (L1)

**Path**: `skills/gsd/discuss-phase/SKILL.md`
**Owner**: owner
**Gate**: none
**Priority**: P2
**Invocable**: Yes (`/gsd-discuss-phase`)
**Platforms**: [desktop-mcp, cli, codex, cursor]

### gsd-ship (L1)

**Path**: `skills/gsd/ship/SKILL.md`
**Owner**: owner
**Gate**: evidence
**Priority**: P1
**Invocable**: Yes (`/gsd-ship`)
**Platforms**: [desktop-mcp, cli, codex, cursor]
