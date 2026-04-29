# Concurrency Bottleneck Plan

## Scope

Review only; do not modify runtime code in this pass. The goal is to identify where concurrent Claude Code/tool invocations can amplify latency or serialize work in the current router/host-integration changes.

## Findings

### 1. PreToolUse hook invokes full host-integration status

- Location: `scripts/router-rs/src/host_integration.rs:1912`
- Location: `scripts/router-rs/src/host_integration.rs:1918`
- Current hook command runs `run_router_rs.sh ... framework host-integration status ...` before tools.
- Bottleneck mode: every tool call can fork a shell, run the launcher, exec router-rs, and perform status reads. Under multi-agent or high-frequency tool use, this becomes linear process and filesystem amplification.
- Priority: high.

Recommended direction:

- Replace the hook target with a minimal purpose-built check that only verifies the specific pre-tool invariant needed by the hook.
- Avoid full projection status from pre-tool hooks.
- Keep richer status output for explicit user/admin commands.

### 2. Launcher build lock serializes cold or stale-binary starts

- Location: `scripts/router-rs/run_router_rs.sh:14`
- Location: `scripts/router-rs/run_router_rs.sh:70`
- Location: `scripts/router-rs/run_router_rs.sh:107`
- The launcher uses a shared target directory and `.router-rs-build.lock` when the binary is missing or stale.
- Bottleneck mode: concurrent hook/status invocations queue on the build lock and poll every 100ms. This is most visible after Rust source changes, cold caches, or CI-like environments.
- Priority: high when combined with hook usage; medium otherwise.

Recommended direction:

- Keep the lock for correctness, but reduce how often hook paths reach the launcher.
- Prefer prebuilt/current router binaries for hook paths.
- Consider a fast failure or stale-binary-tolerant mode only for non-mutating hook checks, if acceptable.

### 3. Claude settings status rereads the same files several times

- Location: `scripts/router-rs/src/host_integration.rs:1382`
- Location: `scripts/router-rs/src/host_integration.rs:1405`
- Location: `scripts/router-rs/src/host_integration.rs:1406`
- Location: `scripts/router-rs/src/host_integration.rs:1407`
- Location: `scripts/router-rs/src/host_integration.rs:1579`
- Location: `scripts/router-rs/src/host_integration.rs:1611`
- `claude_code_projection_status()` calls `claude_settings_status()`, then calls `claude_settings_runtime_status()` twice. Each runtime status calls `claude_settings_status()` again and rereads settings JSON.
- Bottleneck mode: one status call repeatedly reads/parses the same manifest and settings file. If status is called by hooks, this repeats on every tool invocation.
- Priority: high.

Recommended direction:

- Read the projection manifest and settings payload once per status command.
- Pass the parsed settings payload into hooks/statusLine schema checks.
- Keep the JSON response shape unchanged unless a contract update is required.

### 4. Host skill surface install scans and mutates directories serially

- Location: `scripts/router-rs/src/host_integration.rs:2287`
- Location: `scripts/router-rs/src/host_integration.rs:2311`
- Location: `scripts/router-rs/src/host_integration.rs:2326`
- Location: `scripts/router-rs/src/host_integration.rs:2371`
- `ensure_host_skill_surface()` scans the generated surface, removes stale entries, and creates links/generated command files one by one.
- Bottleneck mode: install/sync work is O(number of surface entries + desired skills). Concurrent installs targeting the same surface can also contend through filesystem mutations.
- Priority: medium for install/sync; low for normal routing if not on the hot path.

Recommended direction:

- Treat this as an install-time cost, not the first optimization target.
- If install becomes frequent, consider computing a plan first, then applying only changed paths.
- Avoid parallel writes unless ownership and atomicity are explicit.

### 5. Memory/bootstrap paths synchronously launch nested router commands

- Location: `scripts/router-rs/src/host_integration.rs:983`
- Location: `scripts/router-rs/src/host_integration.rs:2669`
- Location: `scripts/router-rs/src/host_integration.rs:2801`
- Location: `scripts/router-rs/src/host_integration.rs:3689`
- `run_router_rs_json()` synchronously starts router-rs through the launcher and waits for output.
- Bottleneck mode: memory automation can spawn nested router processes for memory recall and runtime snapshot. Concurrent automation runs multiply process startup and filesystem work.
- Priority: medium for automation; low for normal hook/status unless indirectly invoked.

Recommended direction:

- Prefer in-process calls where feasible for memory recall/snapshot operations owned by the same binary.
- If process isolation is required, batch related calls to avoid repeated launcher startup.

## Suggested Implementation Order

1. Add or identify a lightweight hook verification command that avoids full `host-integration status`.
2. Refactor Claude projection status to read manifest/settings once and reuse parsed payloads.
3. Measure hook latency before and after the above two changes.
4. Only then optimize install-time skill surface generation if measurements show it matters.
5. Review nested `run_router_rs_json()` use in memory automation separately from the hook path.

## Validation Plan

- Run existing host integration tests after any code changes.
- Add focused tests for status output compatibility if `claude_code_projection_status()` internals change.
- Benchmark or time:
  - one `framework host-integration status` call,
  - repeated status calls in a loop,
  - first run after touching router source,
  - hook-equivalent lightweight check after introducing it.

## Non-goals

- Do not parallelize filesystem mutations before ownership and atomic write behavior are explicit.
- Do not remove the build lock; it protects concurrent cargo builds.
- Do not change generated host surfaces unless the projection contract is intentionally updated.
