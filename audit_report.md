# Documentation Audit Report

## Verdict

状态：PASS with cleanup applied。

当前文档主线已经改成：Rust-owned runtime/control-plane + thin host projection。旧的 Python fallback、Python/Rust parity lane、OMC live dependency、aionrs/AionUI 主线叙事都只能作为历史或显式 compatibility inventory 出现。

## Current Authority

- Runtime / routing / memory / artifact / host-integration truth：`scripts/router-rs/`
- Shared policy truth：`AGENT.md`
- Host entry proxies：`AGENTS.md` / `CLAUDE.md` / `GEMINI.md`
- Skill routing truth：`skills/SKILL_ROUTING_RUNTIME.json` + `skills/SKILL_ROUTING_INDEX.md`
- Continuity truth：root continuity artifacts + `artifacts/current/*` mirror

## Retired Narratives

- `rust_execute_fallback_to_python` is no longer a kept runtime request surface.
- `framework_runtime/` Python package is retired.
- Python artifact emitter, Python host materializer, Python route shim, Python hook bridge, and Python session artifact writer are retired.
- `aionrs_companion_adapter`, `aionui_host_adapter`, `generic_host_adapter`, and `codex_desktop_host_adapter` are not default peer adapters.
- OMC is replaced, not embedded as compatibility runtime.

## Guardrail

Future doc updates must not describe removed Python paths as current work items. If a document needs to mention them, it must label them as historical, retired, or compatibility-inventory only.
