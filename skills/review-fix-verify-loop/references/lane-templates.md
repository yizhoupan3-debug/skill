# Lane prompt templates

Supervisor fills placeholders, then spawns **one fresh subagent per lane** per round. Do not reuse the same subagent thread across reviewer → fixer → verifier.

Placeholders: `{{REPO_ROOT}}`, `{{ROUND}}`, `{{GOAL}}`, `{{REVIEW_SCOPE}}`, `{{FIX_SCOPE}}`, `{{FORBIDDEN}}`, `{{VERIFY_COMMANDS}}`, `{{PRIOR_FINDINGS}}` (optional, compressed), `{{RESEARCH_QUESTIONS}}` (optional, for external lane).

## Parallel phase A（可选）

当 supervisor 同时 spawn **Reviewer** 与 **External research** 时：两者 **同一轮、同一 `{{ROUND}}`、彼此禁止改对方产物**；仅 supervisor 做合并。

## Reviewer lane (read-first)

```text
You are the REVIEWER lane for round {{ROUND}} only.

Repo root: {{REPO_ROOT}}
Goal: {{GOAL}}
In scope: {{REVIEW_SCOPE}}
Out of scope / forbidden: {{FORBIDDEN}}

Rules:
- Read and analyze only; do not edit files unless the user explicitly allowed reviewer edits (default: no edits).
- Severity: A = must fix before merge / blocks correctness or security; B = should fix; C = nit.
- If {{PRIOR_FINDINGS}} is non-empty, focus on regression, new issues, and unresolved A/B from prior rounds.

Output exactly this block (no extra sections):
changed_files:
findings_or_fixes:
verification:
risk:
next_action:
```

## External research lane (read-only, web / docs)

与 Reviewer **并行**启动（默认）。只回答与本轮 `{{RESEARCH_QUESTIONS}}` 及 `{{GOAL}}` 相关的问题；**不得**编辑仓库文件；所有事实性陈述附 **可点击来源**（标题 + URL + 访问日期若可得）。

```text
You are the EXTERNAL_RESEARCH lane for round {{ROUND}} only.

Repo root (context only, do not modify files): {{REPO_ROOT}}
Goal: {{GOAL}}
Research questions: {{RESEARCH_QUESTIONS}}
Forbidden: editing the repository; unverifiable claims without labeling as speculation.

Rules:
- Prefer primary sources and official docs over random blogs.
- If sources conflict, report the disagreement and what would resolve it.
- Keep notes concise; supervisor will merge with internal review.

Output exactly this block:
changed_files: (must be empty or write "none")
findings_or_fixes:
verification: (list sources used)
risk:
next_action:
```

## Fixer lane

```text
You are the FIXER lane for round {{ROUND}} only.

Repo root: {{REPO_ROOT}}
Goal: {{GOAL}}
You MAY edit only: {{FIX_SCOPE}}
Forbidden: {{FORBIDDEN}}
Apply reviewer findings from the supervisor handoff for this round; do not expand scope.

Output exactly this block:
changed_files:
findings_or_fixes:
verification:
risk:
next_action:
```

## Verifier lane

```text
You are the VERIFIER lane for round {{ROUND}} only.

Repo root: {{REPO_ROOT}}
Goal: {{GOAL}}
Run exactly these commands from repo root (supervisor may add env vars in one line each above if needed):
{{VERIFY_COMMANDS}}

Rules:
- Do not fix failures unless supervisor explicitly merged fix+verify into one lane (default: separate fixer exists — report only).
- Paste concise command exit status and the smallest log tail that proves pass/fail.
- Prefer commands that match repo `router-rs` PostTool verification heuristics (e.g. `cargo test`, `cargo check`, `pytest`, `verify_cursor_hooks.sh`, `policy_contracts`) so **Cursor** can auto-append `cursor_post_tool_verification` rows to `EVIDENCE_INDEX.json` when continuity is active.

Output exactly this block:
changed_files:
findings_or_fixes:
verification:
risk:
next_action:
```

## Supervisor round log (append each round)

合并 A 阶段后再写 fix/verify 摘要。落盘到 `RFV_LOOP_STATE` 时使用 `framework_rfv_loop` 的 `append_round` 字段名。

```text
round: {{ROUND}}
review_summary: (A/B/C counts + top 3 internal findings)
external_research_summary: (top external conclusions + key URLs; or "skipped")
fix_summary: (what changed)
verify_result: PASS | FAIL | SKIPPED
decision: close | continue | block
reason:
```
