# Rust 下一阶段执行清单

> 状态：已收口，保留为后续 Rust-owned 能力深化的边界清单。上一阶段“压缩 Python 主面”已经结束；后续只做 Rust-owned 能力深化。

## 当前起点

- 默认 runtime/control-plane/host-integration 已是 Rust-owned。
- 仓库级 Python runtime、Python route shim、Python host materializer、Python artifact writer、pytest 入口已删除。
- 当前风险不再是 Python 兜底，而是 Rust surface 继续变多后出现文档、contract、consumer 各写一套。

## 本轮目标

把现有 Rust surface 从“可用”推进到“更稳、更少歧义、更好恢复”。当前主线已经完成 Rust authority 收口，后续按能力面继续增量推进：

| ID | 任务 | 目标 | 主要写入范围 |
| --- | --- | --- | --- |
| 1 | Attach / transport hardening | 让 attach descriptor、handoff、binding artifact、cleanup/replay 语义保持一条 contract | `scripts/router-rs/src/trace_runtime.rs`, `scripts/router-rs/src/browser_mcp.rs`, `tools/browser-mcp/src/runtime.ts`, attach/browser tests |
| 2 | Persistence / compaction | 强化 backend-family parity、SQLite/filesystem 读写一致性、snapshot-delta 边界 | `scripts/router-rs/src/runtime_storage.rs`, `scripts/router-rs/src/trace_runtime.rs`, compaction/storage tests |
| 3 | Sandbox control plane | 把 `docs/runtime_sandbox_contract.md` 的 state machine 接入可验证 Rust payload | `scripts/router-rs/src/main.rs`, sandbox/control tests |
| 4 | Host integration polish | 继续收紧 install/bootstrap/sync-skills/entrypoint sync 的 Rust-only 体验 | `scripts/router-rs/src/host_integration.rs`, `scripts/router-rs/src/claude_hooks.rs`, host integration tests |
| 5 | Docs / generated consistency | 只刷新仍描述旧 Python/OMC/aionrs 主线的文档和生成面 | docs, root checklists, `skills/SKILL_*`, targeted doc tests |
| 6 | Memory policy journal | 让 Rust memory policy 同时拥有 SQLite row 与 stable `decisions.md` journal contract | `scripts/router-rs/src/framework_runtime.rs`, memory policy tests, Rust contract docs |

## 验收标准

- 不出现新的 `framework_runtime/` Python 包、`scripts/*.py` runtime wrapper、pytest 配置或 Python fallback 文案。
- memory policy 产物只能由 `router-rs` 写入；不得新增 Python journal writer 或第二套 memory artifact emitter。
- `cargo test --test policy_contracts` 和 `cargo test --test documentation_contracts` 继续锁住退场边界。
- 涉及 host entrypoint 的改动必须通过：
  `cargo run --manifest-path ./scripts/router-rs/Cargo.toml --release -- --sync-host-entrypoints-json --repo-root "$PWD"`。
- 涉及 skill 路由的改动必须通过：
  `cargo run --manifest-path scripts/skill-compiler-rs/Cargo.toml -- --skills-root skills --source-manifest skills/SKILL_SOURCE_MANIFEST.json --health-manifest skills/SKILL_HEALTH_MANIFEST.json --apply`。

## 明确不做

- 不恢复 Python-first route/runtime/control-plane。
- 不把 `codexcli` 抬升为 framework truth。
- 不把 `aionrs` / `AionUI` / OMC 重新写成未来主线。
- 不把 compatibility inventory 当主回归基线；主基线仍是 shared contract 与 parity snapshots。
