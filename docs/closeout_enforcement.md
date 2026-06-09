---
last_verified: "2026-06-02"
depends_on:
  - harness_architecture/index.md
---

# Closeout Enforcement

This document is the operator-facing reference for the harness-level evidence
gate (schema + evaluator rules).

Documentation index (steady-state vs archive): [`README.md`](README.md).

**Skill / AGENTS norms:** Skills that own task execution (`/implementx`, `/verifyx`, `/workflow`,
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
  `core/router-rs/src/closeout_enforcement.rs`)
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

**Source of truth:** This document tracks the Rust implementation
(`closeout_enforcement.rs`) and the contract schema. When in doubt, the
implementation is authoritative — update this document to match the code, not the
other way around.

## Enforcement rules

The evaluator returns `closeout_allowed: false` when any rule fires. Rules are
listed below in evaluation order; rule IDs match the contract
(`closeout_enforcement_contract().rules`).

- **`schema_version_mismatch`** — record uses a schema other than
  `closeout-record-v1`, or `schema_version` is missing/empty.
- **`task_id_missing`** — `task_id` is empty after trimming.
- **`summary_missing`** — `summary` is empty after trimming.
- **`verification_status_invalid`** — `verification_status` is non-empty but not
  one of `passed | failed | partial | not_run`.
- **`verification_status_missing`** — `verification_status` is empty.
- **`claimed_done_without_evidence`** — `summary` contains a completion keyword
  (`done | finished | completed | passed | succeeded | 已完成 | 完成 | 通过 | 搞定`)
  but `verification_status=not_run` and no `risks`/`blockers` were declared.
- **`changed_files_without_command_or_risk`** — `changed_files` non-empty but
  `commands_run` empty AND `risks` empty.
- **`verification_passed_with_failed_command`** — `verification_status=passed`
  but at least one entry in `commands_run` has non-zero `exit_code`.
- **`invalid_command_evidence`** — `commands_run` contains a row with an empty
  `command` string (serde defaults must not turn `{}` into success evidence).
  Also checked at the raw JSON level before serde parsing.
- **`verification_passed_with_missing_artifact`** — `verification_status=passed`
  but at least one `artifacts_checked` entry has `exists=false`.
- **`not_run_without_blockers_or_risks`** — `verification_status=not_run` with
  no `blockers` and no `risks` (closeout must declare why it didn't verify).
- **`claimed_done_with_failed_verification`** — summary claims completion but
  `verification_status=failed` and no `risks`/`blockers`.
- **`claimed_passed_without_evidence`** (R7) — `verification_status=passed` but
  `commands_run`, `artifacts_checked`, `risks`, and `blockers` are all empty.
  Pure self-attestation is insufficient; supply at least one command, artifact
  check, risk, or blocker.
- **`task_id_context_mismatch`** — record `task_id` does not match the expected
  task id supplied in the evaluation context (context-aware path only; also
  checked at the raw JSON level).
- **`claimed_passed_without_evidence_index_rows`** (R8) — context-aware:
  `verification_status=passed` with empty `commands_run` AND zero successful
  rows in the task's `EVIDENCE_INDEX.json`. Artifact existence alone does not
  constitute executable verification.
- **`parse_error`** — serde deserialization failed (e.g. unknown fields rejected
  by `deny_unknown_fields`). The detail message includes the serde error.

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

Each owner skill (implementx, verifyx, workflow, gh-fix-ci, systematic-debugging,
deepinterview, loop) MUST:

1. **Materialize a record** at task end into
   `artifacts/closeout/<task_id>.json`.
2. **Run the evaluator** (`closeout_evaluate` stdio or
   `router-rs closeout evaluate`) before printing a final user-facing summary.
3. **Refuse to declare completion** if `closeout_allowed=false`; instead either
   add the missing evidence or downgrade `verification_status` to `partial`
   /`not_run` and record a `risk`/`blocker`.
4. **Surface the violations** to the user in the closeout message so the gap is
   visible, not silent.

**verifyx** (my-lifecycle ship) additionally MUST, after a successful closeout
evaluate:

5. **Purge** `artifacts/current/<task_id>/` (closeout JSON first, then delete
   the task dir). Optional purge intent in the closeout record `notes` field
   (e.g. `task_artifacts_purged; task_dir_removed`) — not separate schema fields.
   See [`skills/verifyx/SKILL.md`](../skills/verifyx/SKILL.md) §Post-verify
   task-dir purge.

Skills with smaller surface (gitx, slides one-shot rebuild) should still emit
records when they touch files; the evaluator's bar for partial/risk-only
closeouts is intentionally low.

## Hook integration（已接线）

- **Codex Stop** — 在 `ROUTER_RS_CLOSEOUT_ENFORCEMENT` 硬 tier（CI 或显式开启）且助手宣称完成时：`decision: block` + `CLOSEOUT_FOLLOWUP`（见 `hosts/codex_hooks/mod.rs`）。
- **Cursor Stop** — 同源评估（`stop_hard_closeout_followup_for_assistant_response` → `closeout_followup_for_completion_claim`；task id 与 hydration 指针一致）；硬 tier 注入 `followup_message`；本地 solo 默认**软** tier（unset 且非 CI 可不附 record）。
- **Session write** — `write_framework_session_artifacts` 在硬 tier 下可拒写（`enforce_closeout_for_session_payload`）。

CLI / stdio `closeout evaluate` 仍为裁判真源；hook 为投影，见 [`harness_architecture/02-data-flows.md`](harness_architecture/02-data-flows.md) §3.2。

## Tests

- Module unit tests (21 cases): `core/router-rs/src/closeout_enforcement.rs`
  `mod tests`.
- Contract + CLI integration tests (5 cases): `tests/policy_contracts.rs`
  `closeout_*`.
- Schema presence test: `tests/policy_contracts.rs`
  `closeout_record_schema_is_published`.

Run:

```bash
CARGO_TARGET_DIR=/tmp/skill-cargo-target \
  cargo test --manifest-path core/router-rs/Cargo.toml --bin router-rs closeout_enforcement::

CARGO_TARGET_DIR=/tmp/skill-cargo-target \
  cargo test --test policy_contracts closeout
```

## Roadmap

- **Done**: schema, evaluator, CLI, stdio op, unit + integration tests.
- **Next slice**: hook installer wiring (M3), behavioral eval harness (M5-M8),
  per-skill closeout templates once the in-progress workspace edits land.
