# Claude Desktop Full Host Injection Plan

## Objective

Make Claude Desktop a first-class host target with parity to the existing Codex host flow, while preserving a Rust-owned runtime and avoiding unnecessary bridge layers. The end state is not a thin MCP client edge; Claude Desktop should consume the same skill routing, runtime contracts, workspace bootstrap, memory, continuity, artifact, and host-entrypoint surfaces as Codex, with host-specific capabilities represented explicitly.

Non-goals are equally important. Do not introduce Docker images, HTTP proxy services, Node or TypeScript runtime requirements, Python helper shims, duplicated skill registries, or a second runtime authority. Claude Desktop integration must remain local-first and stdio-first. Any host-specific layer must be a projection from the shared framework truth, not a parallel implementation.

## Current State

The repository is still effectively Codex-first. The runtime registry supports `codex-cli` and `codex-app` as host targets. Host sync currently materializes `AGENTS.md` and `.codex/host_entrypoints_sync_manifest.json`. Profile compilation produces Codex-oriented artifacts such as `codex_profile`. The minimal Claude Desktop work currently installs or documents a `browser-mcp` stdio configuration, but that is only an edge MCP client path, not a full framework host.

To reach Codex parity, Claude Desktop must be promoted from MCP client edge to host target. This means the registry, profile compiler, host-entrypoint sync, skill surface, MCP config installation, docs, and tests must all agree that `claude-desktop` is supported.

## Target Architecture

The target chain is: Claude Desktop reads its generated host policy, loads repo-managed MCP stdio servers, calls `router-rs` runtime surfaces directly, routes user requests through the same skill router, and writes/reads continuity through the same artifact and memory contracts.

The only required bridge should be Claude Desktop MCP stdio. The MCP servers should execute `router-rs` directly or call Rust functions internally. Avoid shelling through multiple wrappers where possible. Avoid starting HTTP services. Avoid using the TypeScript browser MCP development harness as a live runtime.

Claude Desktop should have a host profile analogous to Codex. A future `claude_desktop_profile` should be derived from the same shared contract as `codex_profile`, with host-private fields stored under a Claude-specific payload such as `claude_desktop_host_payload`. Shared semantics must stay in the shared framework profile.

## Phase 1: Promote Claude Desktop to Host Target

Update `configs/framework/RUNTIME_REGISTRY.json` so `host_targets.supported` includes `claude-desktop`. Replace Codex-only policy names with multi-host names where appropriate. Keep `codex-cli` and `codex-app` unchanged, but add a Claude Desktop host entrypoint.

Represent Claude Desktop as a host, not merely an `mcp_clients` edge record. If an `mcp_clients.claude-desktop` block exists from the minimal MCP experiment, migrate it into the host model or clearly mark it as a generated host-private MCP config surface. Avoid keeping two conflicting definitions.

Add Claude Desktop entries to framework command host mappings. For `autopilot`, `deepinterview`, and `team`, add `claude-desktop` under `host_entrypoints`. Prefer the same logical commands, such as `$autopilot`, `$deepinterview`, and `$team`, unless Claude Desktop requires slash-style invocation. If slash commands are used, document that difference in host-private entrypoint metadata.

Define a capability matrix. Codex may support CLI resume, CI, non-interactive execution, and tmux worker management. Claude Desktop may support MCP tools and interactive desktop workflows but not all Codex-native process controls. Do not claim unsupported capabilities. Parity means shared runtime and skill semantics, not false equivalence of host-native features.

## Phase 2: Generate Claude Host Entrypoint

Extend host-entrypoint sync so it generates `CLAUDE.md` from the same shared policy source used for `AGENTS.md`. Do not hand-maintain `CLAUDE.md`. Add it to protected generated paths so drift is detected and overwritten by sync.

Update the host sync manifest to list `claude-desktop` in `supported_hosts`, with `host_entrypoints.claude-desktop = "CLAUDE.md"`. The manifest should remain machine-readable and should not imply that Claude Desktop uses Codex-specific config paths.

Consider renaming `codex_hooks.rs` or extracting host sync logic into a neutral module such as `host_entrypoints.rs`. The important goal is conceptual clarity: hook cleanup and host entrypoint generation should not remain permanently branded as Codex-only if Claude Desktop is a supported host.

`CLAUDE.md` should instruct Claude Desktop to use the repo MCP servers and skill runtime surfaces. It should not introduce extra memory files, hidden mirrors, or independent policy copies. It should point back to the shared runtime truth.

## Phase 3: Add Claude Desktop Profile Projection

Extend `framework_profile.rs` so profile compilation emits a Claude Desktop host profile. The clean long-term shape is a `host_profiles` map keyed by host id, but a transitional `claude_desktop_profile` field is acceptable if it is simpler and tested.

The Claude profile should include context files, MCP server definitions, managed config paths, framework alias entrypoints, skill source references, memory mounts, artifact contract, execution protocol contract, delegation contract, and supervisor state contract. Shared fields must come from the existing shared contract builder.

Host-private Claude fields should live under a dedicated payload, for example `claude_desktop_host_payload`. This payload can include Claude Desktop config file names, MCP server names, local stdio command lines, and host capability notes. It must not redefine routing, skill ownership, memory semantics, or artifact layout.

Update profile artifact tests so `profile artifacts` emits both Codex and Claude Desktop surfaces and verifies they share the same core runtime contract.

## Phase 4: Install Claude Desktop Full Integration

Replace or supersede the minimal `install-claude-desktop-mcp` helper with `install-claude-desktop-integration`. The old command can remain as a compatibility alias, but the full command should be the documented path.

The installer should update `claude_desktop_config.json` idempotently. It must preserve existing user MCP servers and only manage repo-owned server names. It should write at least two stdio MCP servers: `browser-mcp` and a new `skill-runtime` or `framework-runtime` MCP server.

The `browser-mcp` server should continue to call `router-rs browser mcp-stdio --repo-root <repo-root>`. The `skill-runtime` server should call a new Rust stdio MCP entrypoint, for example `router-rs framework mcp-stdio --repo-root <repo-root>` or `router-rs skill mcp-stdio --repo-root <repo-root>`.

Installer output should report changed/unchanged status, managed server names, config path, repo root, bridge layers, and capability summary. It should explicitly report `image_required: false` and `transport: stdio`.

## Phase 5: Implement Skill Runtime MCP

Add a Rust MCP stdio server for skill and framework runtime access. Do not implement it in Node or Python. Do not make it call an HTTP endpoint. Prefer direct calls into existing Rust modules instead of spawning nested `router-rs` commands.

Initial tools should include `skill_route`, `skill_search`, `skill_manifest`, `skill_read`, `framework_snapshot`, `memory_recall`, `session_artifact_write`, and `runtime_status`. The first version can be read-heavy, but it must be sufficient for Claude Desktop to route a task to the same skill owner Codex would choose.

The `skill_route` tool should return the same selected skill, overlay skill, layer, score, reasons, and prompt preview as the existing route CLI. The `skill_read` tool should return the relevant `SKILL.md` content or a bounded excerpt. The MCP server must fail closed when a selected skill is unknown or missing.

Add protocol tests using raw MCP initialize/list-tools/call-tool messages. Verify the server exposes the expected tool list and that `skill_route` matches `router-rs route` for fixture queries.

## Phase 6: Align Skill Surface

Keep `skills/` as the source of truth. Avoid copying all skills into a Claude-specific mirror unless Claude Desktop absolutely requires file-based skill discovery. The preferred model is MCP-mediated skill access through `skill-runtime`, because it avoids a second projection tree.

If a file projection becomes necessary, generate it under a clearly named artifact directory such as `artifacts/claude-desktop-skill-surface/skills`. That projection must be generated from the same `skills/` source and have a manifest that records source paths and digests. It must not become an editable truth source.

Update registry defaults so skill source remains single-sourced. Do not add parallel workspace bootstrap tables for Claude Desktop.

## Phase 7: Align Memory and Continuity

Claude Desktop should use the same memory mounts and continuity artifacts as Codex. The host profile should reference the same artifact contract, supervisor state contract, and session artifact writer.

Be careful with host-native resume claims. Codex has CLI resume examples; Claude Desktop may not. Claude Desktop resume should be represented through runtime MCP status and recovery tools unless a real host-native resume path exists.

For team/autopilot workflows, keep worker lifecycle and continuity supervisor-owned. Claude Desktop should trigger or inspect those flows through MCP, not reimplement worker state management locally.

## Phase 8: Update Tests and Policy Contracts

Revise tests that currently prohibit Claude host files or configuration. Replace blanket assertions such as “no `CLAUDE.md`” with more precise assertions: `CLAUDE.md` must be generated, protected, and derived from shared policy; unmanaged `.claude/*` hooks or duplicate skill mirrors should still be prohibited unless explicitly introduced.

Add tests for registry support, profile generation, host sync, installer idempotency, MCP server shape, route parity, and no forbidden bridge layers. Tests should verify that Claude Desktop does not require Docker, HTTP proxy services, Node live runtime, Python shims, or duplicate runtime state roots.

Test names should make the intended boundary clear: Claude Desktop is a full host target, but `router-rs` remains the only runtime authority.

## Phase 9: Documentation and Migration

Update `docs/host_adapter_contracts.md` from Codex-only language to a multi-host contract. Document Codex CLI, Codex App, and Claude Desktop host responsibilities side by side.

Update `docs/rust_contracts.md` to state that host entrypoint sync and native integration are Rust-owned for supported hosts, including Claude Desktop. Remove or qualify old Codex-only claims.

Update `tools/browser-mcp/README.md` so the browser MCP remains documented as one server in the Claude Desktop integration, not the whole integration.

Add a migration note explaining that the earlier minimal Claude Desktop MCP edge has been promoted to a full host target. The note should clarify which files are now generated and which surfaces remain intentionally absent.

## Verification Checklist

A complete implementation is not done until all of these are true:

- `configs/framework/RUNTIME_REGISTRY.json` lists `claude-desktop` as a supported host target.
- `router-rs` host sync generates `AGENTS.md`, `CLAUDE.md`, and a manifest that includes Codex and Claude Desktop hosts.
- `CLAUDE.md` is generated from shared policy and protected from drift.
- Profile artifact generation emits a Claude Desktop profile derived from the same shared runtime contract as Codex.
- Claude Desktop installer writes `browser-mcp` and `skill-runtime` stdio servers idempotently.
- `skill-runtime` MCP exposes skill route/search/read and framework runtime tools.
- `skill_route` returns decisions consistent with the existing Rust route CLI.
- Claude Desktop uses the same `skills/` source of truth or a generated projection with a manifest and digests.
- Memory and continuity use the same runtime contracts and do not create a parallel state root.
- Tests confirm no Docker/image, HTTP proxy, Node live runtime, Python shim, or second route authority is required.

## Rollout Order

Do this in three implementation slices.

First, promote Claude Desktop to a host target and generate `CLAUDE.md`. This includes registry, host sync, manifest, docs, and tests.

Second, add Claude Desktop profile projection and full config installer. This includes `claude_desktop_profile`, `install-claude-desktop-integration`, idempotent config writes, and profile tests.

Third, add the Rust skill runtime MCP. This includes MCP protocol handling, route/search/read tools, framework runtime tools, route parity fixtures, and final documentation updates.

Do not collapse all phases into one large change unless necessary. Each phase should leave the repo internally consistent and testable.
