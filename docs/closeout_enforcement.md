# Closeout Enforcement

This document is the operator-facing reference for the harness-level evidence
gate (schema + evaluator rules).

Documentation index (steady-state vs archive): [`README.md`](README.md).

**Skill / AGENTS norms:** Skills that own task execution (autopilot, team,
gh-fix-ci, systematic-debugging, deepinterview, slides,
paper-*) should emit a closeout record consistent with this contract before
claiming completion — this stays true even when router-rs is in the **local soft**
programmatic tier (see below).

**Programmatic refusal:** Whether `write_framework_session_artifacts` **refuses**
a completion claim without a passing record depends on the enforcement tier
(`ROUTER_RS_CLOSEOUT_ENFORCEMENT` and CI/GitHub Actions detection); see
**Personal / local opt-out**.

## Authorities

- **Schema**: `configs/framework/CLOSEOUT_RECORD_SCHEMA.json`
  (`schema_version: closeout-record-v1`)
- **Evaluator**: `router-rs` Rust binary (module
  `scripts/router-rs/src/closeout_enforcement.rs`)
- **Owner authority**: `rust-closeout-enforcement`

## Personal / local opt-out

On a **local machine**, if `ROUTER_RS_CLOSEOUT_ENFORCEMENT` is **unset** and the
process is **not** in a CI-like environment, programmatic enforcement defaults
to **soft**: `write_framework_session_artifacts` will not reject completion
statuses solely for a missing `closeout_record`.

Router-rs treats programmatic enforcement as **strict** when it detects a CI-like
environment **unless** you explicitly disable it with
`ROUTER_RS_CLOSEOUT_ENFORCEMENT=0`/`false`/`off`/`no`:

- **`CI`**: non-empty after trimming, and not one of `0` / `false` / `off` / `no`
  (case-insensitive). An **empty** `CI` value does **not** count as CI.

- **`GITHUB_ACTIONS`**: implementation treats **only** the exact string `true`
  as GitHub Actions (`GITHUB_ACTIONS=true`). Other truthy-looking values are not
  recognized by router-rs today.

You can **explicitly** force strict behavior with `ROUTER_RS_CLOSEOUT_ENFORCEMENT=1`
(or `true`/`on`/`yes`). More generally, if the variable is **set** to anything
other than the four soft-disable tokens above (including **empty string**), the
completion write path behaves as **strict**.

**Note:** an **empty string** (`export ROUTER_RS_CLOSEOUT_ENFORCEMENT=`) is **not**
the same as “unset”; it does **not** match the soft-disable tokens and tends to
keep **strict** evaluation paths.

**Do not rely on soft defaults in CI, shared automation, or team workflows** —
keep evidence-backed completion there so "done" stays auditable.

## Why this exists

Without an evidence gate, agents can declare "done" without verification. The
harness then has no way to detect when a task ends in
`verification_missing`/`unverified_claim` state. Evidence enforcement converts
the soft-norm "give evidence on closeout" (in `AGENTS.md`) into a programmatic
contract that hooks, supervisors, and CI can all enforce identically.

## The record schema (v1)

Every closeout MUST emit JSON of this shape:

```json
{
  "schema_version": "closeout-record-v1",
  "task_id": "ppt-2026-05-09-fix-blurry-images",
  "started_at": "2026-05-09T05:00:00Z",
  "ended_at":   "2026-05-09T05:18:00Z",
  "summary": "Replaced PIL diagrams with native pptx tables; deck regenerates with 22 slides.",
  "verification_status": "passed",
  "changed_files": ["ppt/build_deck.py"],
  "commands_run": [
    {"command": "python build_deck.py", "exit_code": 0, "duration_ms": 2143}
  ],
  "artifacts_checked": [
    {"path": "ppt/deck_v3.pptx", "exists": true, "size_bytes": 8420191}
  ],
  "blockers": [],
  "risks":    [],
  "notes":    "Slide count and image counts not asserted yet; consider adding a smoke test."
}
```

Required fields: `schema_version`, `task_id`, `summary`, `verification_status`.
Allowed `verification_status` values: `passed | failed | partial | not_run`.

## Enforcement rules

The evaluator returns `closeout_allowed: false` when any rule fires:

- **`schema_version_mismatch`** — record uses a schema other than
  `closeout-record-v1`.
- **`task_id_missing`** / **`summary_missing`** /
  **`verification_status_missing`** / **`verification_status_invalid`** —
  required fields missing or malformed.
- **`claimed_done_without_evidence`** — `summary` contains a completion keyword
  (`done | finished | completed | passed | succeeded | 已完成 | 完成 | 通过 | 搞定`)
  but `verification_status=not_run` and no `risks`/`blockers` were declared.
- **`changed_files_without_command_or_risk`** — `changed_files` non-empty but
  `commands_run` empty AND `risks` empty.
- **`verification_passed_with_failed_command`** — `verification_status=passed`
  but at least one entry in `commands_run` has non-zero `exit_code`.
- **`verification_passed_with_missing_artifact`** — `verification_status=passed`
  but at least one `artifacts_checked` entry has `exists=false`.
- **`not_run_without_blockers_or_risks`** — `verification_status=not_run` with
  no `blockers` and no `risks` (closeout must declare why it didn't verify).
- **`claimed_done_with_failed_verification`** — summary claims completion but
  `verification_status=failed` and no `risks`/`blockers`.

## Calling the evaluator

### CLI

```bash
router-rs closeout evaluate --record-path artifacts/closeout/<task_id>.json
router-rs closeout evaluate --input-json '{"schema_version":"closeout-record-v1", ...}'
router-rs closeout contract     # print the rule list and authority info
```

### stdio JSON loop

When agents talk to `router-rs --stdio-json`, two ops are exposed:

- `closeout_evaluate` — payload is the closeout record body.
- `closeout_contract` — no payload; returns rule list and schema versions.

### Response shape

```json
{
  "schema_version": "router-rs-closeout-enforcement-response-v1",
  "authority": "rust-closeout-enforcement",
  "task_id": "...",
  "closeout_allowed": false,
  "claimed_completion": true,
  "verification_status": "not_run",
  "violations": [
    {"rule": "claimed_done_without_evidence", "severity": "block",
     "detail": "..."}
  ],
  "missing_evidence": ["validation_command_or_risk_acknowledgement"]
}
```

## How skills should use it

These items are **skill-level** obligations under `AGENTS.md`. Follow them even on
workstations where programmatic enforcement is **soft**: router-rs may not block
the artifact write locally, but emitting and evaluating records keeps completion
honest for operators and for CI.

Each owner skill (autopilot, team, gh-fix-ci, systematic-debugging,
deepinterview, loop) MUST:

1. **Materialize a record** at task end into
   `artifacts/closeout/<task_id>.json`.
2. **Run the evaluator** before printing a final user-facing summary.
3. **Refuse to declare completion** if `closeout_allowed=false`; instead either
   add the missing evidence or downgrade `verification_status` to `partial`
   /`not_run` and record a `risk`/`blocker`.
4. **Surface the violations** to the user in the closeout message so the gap is
   visible, not silent.

Skills with smaller surface (gitx, slides one-shot rebuild) should still emit
records when they touch files; the evaluator's bar for partial/risk-only
closeouts is intentionally low.

## Hook integration (planned)

- **Codex Stop hook** — fail-closed. Stop hook calls `router-rs closeout
  evaluate` against the record at `artifacts/closeout/<task_id>.json`; if
  `closeout_allowed=false`, returns `decision: block` with a `followup_message`
  listing violations.
- **Cursor stop hook** — soft gate (host limitation). Same evaluation, but
  surfaced via `followup_message` only.
- **Session supervisor** — `session_supervisor` will refuse to transition
  `completed-unintegrated → integrated` until the worker's closeout record
  passes evaluation.

These wiring points are tracked separately and are NOT yet active in
`.codex/hooks.json` / `.cursor/hooks.json`. The CLI and stdio ops are the
first-class surface; hook wiring is the next slice.

## Tests

- Module unit tests (13 cases): `scripts/router-rs/src/closeout_enforcement.rs`
  `mod tests`.
- Contract + CLI integration tests (4 cases): `tests/policy_contracts.rs`
  `closeout_*`.
- Schema presence test: `tests/policy_contracts.rs`
  `closeout_record_schema_is_published`.

Run:

```bash
CARGO_TARGET_DIR=/tmp/skill-cargo-target \
  cargo test --manifest-path scripts/router-rs/Cargo.toml --bin router-rs closeout_enforcement::

CARGO_TARGET_DIR=/tmp/skill-cargo-target \
  cargo test --test policy_contracts closeout
```

## Roadmap

- **Done**: schema, evaluator, CLI, stdio op, unit + integration tests.
- **Next slice**: hook installer wiring (M3), behavioral eval harness (M5-M8),
  per-skill closeout templates once the in-progress workspace edits land.
