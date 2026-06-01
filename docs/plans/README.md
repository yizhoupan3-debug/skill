---
last_verified: "2026-06-02"
depends_on:
  - ../framework_operator_primer.md
---

# Plans index

**planx 执行真源**：`/planx` 写入 `artifacts/current/<task_id>/ROADMAP.md` 与 `WAVE_STATE.json`（`artifacts/current/` 为本地手动画板，通常不入库）。历史 ROADMAP 在 git 历史或归档中，**不**假定本目录下仍有 stub 镜像。

Cursor Plan 模式（`.cursor/plans/*.plan.md`）仅在活跃任务需要时使用；已完成的计划文件不保留在仓库中。

## 相关 skill

- [`skills/planx/SKILL.md`](../../skills/planx/SKILL.md) — ROADMAP / WAVE_STATE

## 历史

2026-05 卫生清理已移除本目录下的 stub 镜像与过期 `.cursor/plans` 文件；细节见 git 历史。
