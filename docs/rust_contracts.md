# Runtime Rust Contracts

## Purpose

This document freezes the Rust-owned runtime contracts for this repository.
Historical migration notes live under `docs/history/`; this file describes only
the current runtime truth in `router-rs` and related Rust tools.

Upper-layer control-plane narrative (L1–L5, evidence/resume injection boundaries):
[`harness_architecture.md`](harness_architecture.md). Host adapter portability (portable core, event→CLI pointers, new-host checklist):
[`host_adapter_contract.md`](host_adapter_contract.md). Steady-state doc index:
[`README.md`](README.md) in this directory.

**Host-neutral entrypoint sync**：`router-rs framework sync-entrypoints --repo-root <repo>` 与 `router-rs codex sync --repo-root <repo>` 调用同一 `sync_host_entrypoints`（Codex provider）；优先在文档/操作序列中使用前者以减少「只有 Codex 才做 sync」的误导。

**Skill `platforms` 缺省**：`skill-compiler-rs` 从 `RUNTIME_REGISTRY.json` 读取 `host_targets.supported`；`SKILL.md` 未写 `platforms` 或写 `supported` / `all-hosts` 时，生成产物中的 `host_support.platforms` 展开为闭集全集（与 harness 默认路由对等策略一致）。

It is the contract source of truth for:

- routing and route diagnostics
- profile / explicit host projection compilation
- execution response shape
- runtime control-plane descriptors
- framework runtime snapshot / artifact continuity
- trace transport, checkpointing, compaction, observability, and sandbox policy

## Harness architecture (control plane)

Upper-level layering for hooks, continuity artifacts, and evidence flows lives in [`harness_architecture.md`](harness_architecture.md) (L1–L5 model, extension rules). **Closed-set host ids** and install/sync alignment with manifests: `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported` (see [`host_adapter_contract.md`](host_adapter_contract.md)). Operator nudge strings for RFV / Autopilot hooks are loaded from `configs/framework/HARNESS_OPERATOR_NUDGES.json` (`harness_operator_nudges`); default hook output only uses compact status nudges, while long math/retrieval/strict hints stay in docs/schema. **`ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE`**、**`ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE`** 与 **`ROUTER_RS_QODER_REVIEW_GATE_DISABLE`** 均使用 **`router_rs_env_enabled_default_false`** (`scripts/router-rs/src/router_env_flags.rs`): only `1`/`true`/`yes`/`on` disables the gate; other non-empty values leave it enabled. **`ROUTER_RS_CURSOR_REVIEW_GATE_STOP_MAX_NUDGES`**（同文件 `router_rs_cursor_review_gate_stop_max_nudges_cap`）限制 Cursor `Stop` 上完整 `REVIEW_GATE` 硬行重复次数（默认与 harness §5 表一致；`0`/`false`/`off`/`no` 关闭降频）。**Cursor review-routing regexes** ship as **`include_str!("…/REVIEW_ROUTING_SIGNALS.json")`** in `review_routing_signals.rs` (build-time snapshot; changing the JSON on disk alone does not change hook behavior until `router-rs` is rebuilt). **`router_rs_observation`** (`scripts/router-rs/src/router_rs_observation.rs`) labels outbound hook JSON with **`cursor` / `codex` / `claude-code` / `qoder`**; `router-rs claude hook` / `router-rs qoder hook` CLI attaches it after the shared stdio-agent hook dispatch, while each host keeps distinct state directories, env names, gate tokens, and projection paths. **`host_projections.*.capabilities`** today mixes product-facing affordances (e.g. MCP, supervisor) with harness expectations; **`host_projections.*.harness_capabilities`** is the explicit harness-semantics slice (routing/continuity/closeout/review-gate observation, etc.) and must not duplicate product-only tokens such as **`mcp_servers`**—see [`host_adapter_contract.md`](host_adapter_contract.md) **§0** / **§3.2** and `tests/policy_contracts.rs` **`runtime_registry_host_projections_split_harness_capabilities`**. Treat absent product keys as “not claimed for that host” rather than silently assuming cross-host parity. Rust contracts below remain the implementation authority.

## Current Boundary

Rust owns the default runtime and contract path.

- `router-rs route <query>` owns route decisions; route diagnostics use the Rust stdio route policy/report operations.
- `router-rs profile emit` and `router-rs profile artifacts` own the shared framework profile plus explicit Codex projection artifacts.
- Rust stdio `execute` operation owns the live/dry-run execution response contract.
- `router-rs framework doctor`（人读路径/钩子文件/同步提示）、`router-rs framework snapshot`, `contract-summary`, `session-artifact-write`, `hook-evidence-append`, and `prompt-compression` own framework runtime read/write/policy surfaces. Cursor `PostToolUse` normalizes stdin via `hook_posttool_normalize::synthetic_post_tool_evidence_shape` before append and may emit `cursor_post_tool_verification` rows (terminal tools + verification-shaped commands) alongside Codex `codex_post_tool_verification` and `rust-lint`’s `cursor_rust_lint` hook evidence.
- Cursor `review_gate` / Codex **`codex hook`** / **`router-rs claude hook`** / **`router-rs qoder hook`** 出站 JSON 可含顶层 **`router_rs_observation`**（`scripts/router-rs/src/router_rs_observation.rs`；宿主 id 与闭集一致：`cursor` / `codex` / `claude-code` / `qoder`）；可选 `correlation.session_id` / `correlation.task_id`。
- Stdio op `framework_hook_evidence_append` mirrors `router-rs framework hook-evidence-append --input-json …` for scripted callers appending rows to `EVIDENCE_INDEX.json` under continuity (same payload shape as the CLI).
- `router-rs codex sync` and **`router-rs framework sync-entrypoints`** remain compatible CLIs for repo host-entrypoint materialization; internally, `host_entrypoint_sync` is the shared sync engine and `codex_hooks` supplies the `codex provider` for `.codex/hooks.json`, `AGENTS.md` bootstrap, and Codex skill surface refresh. Full sync applies to the current root; matched sibling worktrees receive JSON hook/manifest updates only, so local policy text entrypoints are not overwritten across worktrees.
- `router-rs framework host-integration ...` owns native install/status/remove, bootstrap, projection, and related host integration flows. `router-rs codex host-integration ...` is a thin compatibility alias only.

## Current Status Ledger

### 当前真源

- Routing authority is Rust.
- **Hook observation gate 分类**真源为 [`configs/framework/ROUTER_RS_HOOK_OBSERVATION_RULES.json`](../configs/framework/ROUTER_RS_HOOK_OBSERVATION_RULES.json)（`schema_version`: **`router-rs-hook-observation-rules-v1`**）；由 `scripts/router-rs/src/hook_observation_rules.rs` **`include_str!`** 编译期嵌入 `router-rs`，驱动 `router_rs_observation` 对 `followup_message` / `additional_context` 的匹配顺序与 `router-rs <token>` → `gate.code` 映射。仅改工作区 JSON 不重建二进制则 hook 行为不变。
- **路由启发式切片（第一批）**真源为 [`configs/framework/ROUTING_SIGNAL_MARKERS.json`](../configs/framework/ROUTING_SIGNAL_MARKERS.json)（`schema_version`: **`routing-signal-markers-v1`**）；`scripts/router-rs/src/route/signals.rs` 以 **`include_str!`** 嵌入，供给 **`is_meta_routing_task`**、`build_route_context` 使用的 completion / supervisor marker 字符串表。契约去重见根 `tests/policy_contracts.rs` **`routing_signal_markers_json_unique_nonempty_lists`**。
- **NL 热路由 per-record suppress/boost** 真源为 [`configs/framework/NL_ROUTE_ADJUSTMENTS.json`](../configs/framework/NL_ROUTE_ADJUSTMENTS.json)（`schema_version`: **`nl-route-adjustments-v1`**）；由 `scripts/router-rs/src/route/nl_route_adjustments.rs` **`include_str!`** 嵌入，在 `score_route_candidate` 中与 `ROUTING_SIGNAL_MARKERS` 分层使用（前者：按 skill 记录的条件动作；后者：跨查询短语 marker）。
- **`skills/SKILL_PLUGIN_CATALOG.json`** 中 `skills.<slug>.host_support.platforms` 由 **`scripts/skill-compiler-rs`** 从各 **`skills/<slug>/SKILL.md`** 的 `platforms` / `metadata.platforms` 生成并归一到闭集宿主 id；**不要**手改 JSON 作为宿主列表真源。契约测试：`tests/policy_contracts.rs` 的 **`runtime_host_support_platforms_are_registry_closed_and_match_skill_md`**。
- Live execution and dry-run preview use Rust stdio.
- Runtime control plane publishes Rust-owned authority for `router`, `state`, `trace`, storage, and `background`.
- Framework snapshot, contract summary, session artifact writing, hook evidence append (CLI + stdio), and prompt policy use direct `router-rs` surfaces.
- Host entrypoint sync and native integration are Rust-owned through `router-rs`; the **closed-set supported hosts** are defined by **`configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`** (install-skills/tool spellings derive from `framework_host_targets` in router-rs). `host_projections` is the profile/projection payload set, `framework_commands.*.host_entrypoints` is the explicit command-entrypoint set, and `SKILL_PLUGIN_CATALOG.json` `skills.<slug>.host_support.platforms` is the skill-body support set. Older docs and onboarding examples sometimes mention only `codex-cli` and `cursor`; **that is not an alternate host-id enumeration**—the authoritative closed-set ids are **only** whatever appears under `host_targets.supported` in the checked-in registry JSON.
- `HostProjectionAdapter` remains the thin Rust adapter table for projection install/status/remove side effects; the registry still owns the closed host ids and install-tool spellings.
- Runtime traces expose resumable `seq` / `cursor` metadata, transport binding artifacts, handoff descriptors, and process-external attach resolution.
- Harness trajectory diagnostics reuse `TRACE_EVENTS.jsonl`; `router-rs eval harness-contract` defines the required payload convention and failure taxonomy, while `router-rs framework step-ledger` owns task-scoped `STEP_LEDGER.jsonl` append/summary for long-task recovery.
- Runtime storage exposes backend-family capability discovery, digest verification, and fail-closed alignment between store/checkpointer/trace/state families.
- SQLite is the strongest local backend for WAL, consistent append, compaction, and snapshot-delta support; filesystem remains the safe default storage.
- Session supervisor and background state expose Rust-owned tmux/session/rate-limit/resume control-plane records without external runtime dependency.
- Observability vocabulary, exporter descriptor, metric catalog, dashboard schema, and metric record payloads are Rust-owned.
- Sandbox lifecycle contract is frozen and has a minimal Rust-owned control-plane surface.

### 默认面边界

- Do not add a second route authority, default artifact emitter, host-specific generated layer, or parallel runtime state root.
- Generated host entrypoints are limited sync outputs, not hand-authored truth.
- Historical migration inventory belongs in `docs/history/`, not in steady-state contracts.

### 下一 safe slice

- Harden remote-capable attach/handoff/binding/replay semantics so every consumer uses the same descriptor contract.
- Deepen backend-family compaction and snapshot-delta behavior without changing logical state meaning.
- Expand sandbox lifecycle enforcement without claiming a remote sandbox backend before it exists.
- Keep host integration Rust-only and fail if generated entrypoints drift.
- Refresh docs and generated routing outputs only when contract changes require it.

## Contract Rules

- Contract changes must be explicit and versioned.
- Rust may replace implementations, not silently redefine semantics.
- Host-private fields stay under explicit host projection payloads such as `codex_profile.codex_host_payload`; they must not enter framework core truth.
- Active contracts must describe current owners and outputs, not migration inventory.
- Any alternate runtime, routing, artifact, hook, or host-integration implementation is a regression unless explicitly approved as a host-private edge script.

## Host Projection Invariants

- The shared framework core is the profile authority; host projections are closed-set and explicit.
- Supported hosts are **exactly** the ids enumerated under **`configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`**. `host_projections` is a narrower generated payload/projection set and must not be read as a second closed-set host registry.
- **Profile bundle vs host registry:** `build_profile_bundle` (`scripts/router-rs/src/framework_profile.rs`) derives `host_payloads` from `RUNTIME_REGISTRY.host_projections` while preserving legacy `codex_profile` / `full_codex_profile` artifacts for Codex consumers. `codex-app` may be present in `host_targets.supported` and skill `host_support.platforms` without a separate `host_projections` payload or framework-command entrypoint; that is a projection-family distinction, not a second host registry.
- `codex_profile` is the Codex projection artifact and may carry Codex-private payload fields.
- Generated host projections are disposable install targets and must remain thin bootstrap pointers to the Rust core.
- `framework host-integration remove` removes only framework-owned projection files and manifest-recorded settings keys; user-authored files and unrelated settings are preserved.
- `framework host-integration compatibility-aliases` is the machine-readable inventory for retained aliases such as `install-skills`, `codex host-integration`, and `--repo-root`; each entry must include owner, reason, primary command, kept policy, removal condition, and `independent_behavior: false`.
- `configs/framework/GENERATED_ARTIFACTS.json` declares checked-in generated artifacts with schema `framework-generated-artifacts-manifest-v1`; `framework host-integration generated-artifacts-status` is a manifest-backed byte-for-byte drift gate that regenerates declared artifacts in an isolated temporary root, compares manifest-declared outputs, reports undeclared generated framework artifacts across reverse-reference surfaces, and rejects expanded host-private paths in shared artifacts.

## Route Contract

Required route result fields:

- `task`
- `session_id`
- `selected_skill`
- `overlay_skill`
- `layer`
- `score`
- `reasons`
- `prompt_preview`
- `route_engine`
- `diagnostic_route_mode`
- `route_diagnostic_report`

Invariants:

- exactly one primary owner
- at most one overlay
- `route_engine` and primary authority stay Rust
- unknown selected skills fail closed in consumers
- fallback selection may choose a safe owner from `SKILL_MANIFEST.json`, but must not introduce a second route authority
- generated framework command aliases must name an existing manifest owner as `canonical_owner`; deleted historical owners may only appear under `docs/history/`

## Runtime Control Contracts

Runtime control-plane payloads must keep these owner markers stable:

- `rust-route-core`
- `rust-route-compiler`
- `rust-runtime-control-plane`
- `rust-runtime-storage`
- `rust-runtime-trace-io`
- `rust-framework-runtime-read-model`
- `rust-framework-session-artifact-writer`
- `rust-framework-prompt-policy`

## Portability and environment (`router-rs`)

- **Non-Unix**: Hook helpers that depend on POSIX process semantics (for example lock staleness or `kill(pid, 0)`) use conservative defaults under `cfg(not(unix))` so builds stay green; behavior may differ from Linux/macOS until those paths are specialized.
- **`libc` and `unsafe`**: Codex/Cursor hooks use narrow `unsafe` blocks for `flock`, `kill`, and related syscalls. Call sites are responsible for invariants; errors surface as structured hook outcomes, not panics, except where tests explicitly exercise failure injection.
- **`ROUTER_RS_*` flags**: Parsing, default-on/default-off policy, and naming for environment toggles should stay in [`scripts/router-rs/src/router_env_flags.rs`](../scripts/router-rs/src/router_env_flags.rs) so new flags do not sprawl across the crate.
- **Browser MCP**: Steady-state control for Browser MCP stdio in this repo is the Rust implementation (`scripts/router-rs/src/browser_mcp/` and CLI wiring). The [`tools/browser-mcp/`](../tools/browser-mcp/) TypeScript package is auxiliary (for example dev or replay); treat Rust as the default product path unless documentation explicitly scopes a TS-only workflow.

## External Benchmark

DeerFlow 2.0 remains a useful benchmark for decomposition ideas:

- harness/app split
- explicit run-manager conflict semantics
- resumable stream bridge
- unified store/checkpointer seams
- sandbox lifecycle boundaries

It is not a template to copy directly. This repo keeps its own Rust-owned state
machine and avoids LangGraph-shaped or reflection-heavy runtime assumptions.
