# Rust-Only / Codex-Only Execution Plan

日期：2026-04-25

## Review Conclusion

当前仓库已进入 Rust-owned、Codex-only 的可验证收口阶段。本轮执行目标是把“默认入口、路由基线、文档契约、Browser MCP live surface”从并行语义收敛为单一路径。

已确认的方向：

- 默认 router help 只展示 canonical subcommands。
- 旧顶层 live flags 不再作为默认入口；使用时返回迁移提示。
- Codex profile 只保留 `codex_profile` 作为默认 profile artifact。
- Browser MCP live path 为 Rust stdio；TypeScript surface 仅保留为 dev/parity harness。

## Target Definition

### Rust-only

默认 runtime、routing、profile、host integration、memory/artifact continuity、trace/storage/browser MCP stdio 入口都由 Rust 拥有。

允许存在的非 Rust 内容：

- 文档、测试 fixture、历史记录。
- 技术领域 skill，例如 `python-pro`、`typescript-pro`，因为它们是任务路由 owner，不是框架 runtime。
- 明确标记为 dev-only 或 legacy-only 的工具 surface。

不允许存在的默认 live 内容：

- Python route shim。
- Python artifact emitter。
- Python live fallback。
- Node/TypeScript 作为默认 Browser MCP live server。
- Claude/Gemini/Cursor/Anthropic 等非 Codex host 作为默认 host projection、hook、prompt、runtime state 或 install target。

### Codex-only

Codex 是唯一默认 host。默认 profile artifact 只允许 `codex_profile`。

不允许默认生成：

- 旧的 generic / CLI / Desktop adapter artifacts。
- non-Codex host projection。

## Completed Blocking Repairs

### P0: Routing Baseline

已修复：

- `tests/routing_route_fixtures.json` 的 `overlay-selection` case 已与实际 route contract 对齐。
- `tests/routing_eval_cases.json` 中 paper reviewer 图表排版 slice 的 expected/forbidden owner 矛盾已解除。
- Router-rs 核心测试已恢复绿色。

验收：

```bash
cargo test --manifest-path scripts/router-rs/Cargo.toml --quiet
```

## Command Surface Cutover

### Target State

默认 help 只展示 subcommands：

```text
route
search
framework
codex
trace
storage
browser
profile
migrate
```

旧顶层 live flags 的策略：

- 不再作为 in-repo caller 使用。
- 不在 top-level help 展示。
- 如果仍被用户或外部脚本调用，返回清晰的 canonical subcommand 迁移提示。

### Canonical Commands

| Surface | Canonical command |
|---|---|
| route decision | `route <q>` |
| skill search | `search <q> --json` |
| framework snapshot | `framework snapshot` |
| framework contract summary | `framework contract-summary` |
| memory recall | `framework memory-recall <q>` |
| memory policy | `framework memory-policy --input-json <json>` |
| prompt compression | `framework prompt-compression --input-json <json>` |
| session artifact write | `framework session-artifact-write --input-json <json>` |
| refresh | `framework refresh` |
| alias | `framework alias <alias>` |
| hook projection | `codex hook-projection` |
| host entrypoint sync | `codex sync` |
| host entrypoint check | `codex check` |
| explicit audit hook | `codex hook <cmd>` |
| native host integration | `codex host-integration <args...>` |
| runtime storage | `storage runtime --input-json <json>` |
| backend catalog | `storage backend-catalog` |
| backend parity | `storage backend-parity` |
| Browser MCP stdio | `browser mcp-stdio` |
| Browser attach resolver | `browser resolve-attach-artifact` |
| profile emit | `profile emit --framework-profile <path>` |
| profile artifacts | `profile artifacts --framework-profile <path>` |

### Implementation Status

- Added a top-level help contract test for canonical subcommands only.
- Migrated in-repo test helpers and Browser MCP resolver tests to canonical subcommands.
- Migrated host-integration self-calls and memory runbooks to canonical subcommands.
- Converted legacy top-level live flags to fail-fast migration guidance before dispatch.

验收：

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml -- --help
cargo test --manifest-path scripts/router-rs/Cargo.toml --quiet
```

## Codex-Only Profile Cleanup

Current state：

- `framework_profile.rs` emits `codex_profile` as the only default profile artifact.
- Host-private fields stay under `codex_profile.codex_host_payload`.
- Docs now describe `profile emit` / `profile artifacts` canonical subcommands.
- Historical adapter artifact names are not documented as active surfaces.

验收：

```bash
rg -n "generic adapter|multi-host adapter" docs configs scripts tests skills/SKILL_ROUTING_RUNTIME.json
```

## Browser MCP Decision

Chosen path：Option A。

Meaning：Browser MCP TypeScript remains dev/parity harness only for this transition slice. Rust stdio is the only supported live Codex path.

已执行：

1. `tools/browser-mcp/README.md` states Rust stdio is the only live Codex path.
2. Policy coverage checks live config/docs/start script do not point to the Node build.
3. Existing launcher tests assert there is no Node fallback.
4. TypeScript tests remain dev regression coverage until the surface is deleted.

验收：

```bash
cargo test --manifest-path scripts/router-rs/Cargo.toml --quiet
```

## Host Integration And Migration Cleanup

Current state：

- Skill source language uses resource/source naming rather than bridge naming.
- Legacy artifact migration remains explicit under `router-rs migrate ...` or explicit host-integration options.
- Normal memory automation does not silently run legacy migration unless requested.
- `.codex/config.toml` and `.codex/hooks.json` are consistent: hooks are disabled and hook JSON is not generated.

验收：

```bash
cargo test --test host_integration --quiet
```

## Final Acceptance Gate

The cutover is complete when all commands pass:

```bash
cargo test --manifest-path scripts/router-rs/Cargo.toml --quiet
cargo test --quiet
cargo run --manifest-path scripts/router-rs/Cargo.toml -- --help
```

Expected result:

- Tests pass.
- Top-level router help exposes only canonical subcommands.
- Legacy command callers are migrated or fail with clear migration messages.
- Browser MCP live path is Rust-owned, with Node/TS dev-only.
- Docs describe canonical subcommands, not old flag truth.
- `codex_profile` is the only default profile artifact.
- No default host projection targets Claude, Gemini, Cursor, Anthropic, or generic multi-host adapters.
