# gh-address-comments — Detailed Guide

> Extracted from SKILL.md to reduce token consumption at routing time.

## Task ownership and boundaries

This skill owns:
- locating the current branch PR
- fetching review threads, review bodies, and conversation comments
- grouping comments into an actionable queue
- proposing which comments are must-fix vs optional vs clarify/disagree
- applying selected fixes and preparing a concise follow-up summary

This skill does not own:
- debugging broken checks as the main task
- generic git cleanup unrelated to PR feedback
- writing a brand-new feature plan from scratch

If the task shifts to adjacent skill territory, route to:
- `$gh-fix-ci` for CI failures
- `$gitx` for branch/push/rebase/publish work

## Required workflow

1. Confirm PR context and GitHub CLI access.
2. Fetch all current PR feedback with the local helper script first.
3. Convert raw comments into a numbered action list.
4. Ask the user which items to address if the selection is still ambiguous.
5. Apply the chosen fixes.
6. Re-check whether the addressed threads are now covered by the code changes.
7. Deliver a concise summary of what was fixed, what remains, and any suggested reply posture.

## Core workflow

### 1. Intake

- Work inside the target repository on the branch whose PR feedback should be addressed.
- Verify `gh` is authenticated:
  - `gh auth status`
- Resolve the current branch PR:
  - `gh pr view --json number,url,title`
- Use the bundled helper first:
  - `cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/gh_source_gate_rs/Cargo.toml --bin gh-source-gate -- fetch-comments --repo . --json`
  - add `--open-only` when you only need unresolved, non-outdated review threads

### 2. Fetch and classify feedback

- Run the helper to fetch:
  - review threads
  - review submission bodies
  - top-level PR conversation comments
  - a summary with total, unresolved, outdated, and actionable thread counts
- Turn raw feedback into numbered items with:
  - file/path context if available
  - short summary
  - likely action required
  - current status: unresolved / outdated / resolved

Classify each item into one of:
- `must-fix`
- `likely-fix`
- `clarify-with-reply`
- `disagree-with-reason`
- `already-covered`

### 3. Decide what to act on

- If the user already specified which comments to fix, proceed directly.
- If not, present a compact numbered list and ask which items to address.
- Prefer fixing:
  - correctness issues
  - security issues
  - test gaps
  - maintainability concerns with clear local impact

### 4. Implement selected fixes

- Change only what is required to address the selected feedback.
- Keep each change traceable to one or more numbered comment items.
- Avoid opportunistic unrelated refactors unless they are necessary to satisfy the review comment.
- If a comment cannot be fixed cleanly, explain why and propose the best reply stance.

### 5. Recheck and prepare follow-up

- Re-run the smallest relevant verification for the touched area:
  - tests
  - lint
  - typecheck
  - build
- Reconcile the final diff against the numbered comment list.
- Summarize:
  - fixed comments
  - comments still open
  - comments needing a human explanation instead of code

## Output defaults

Default output should contain:
- PR context
- numbered review items
- fix status

Recommended structure:

````markdown
## PR Review Summary
- PR: #123
- Scope: review threads + conversation comments

## Numbered Comment Queue
1. `[must-fix]` ...
2. `[clarify-with-reply]` ...

## Actions Taken
- Fixed: #1, #3
- Deferred: #2

## Verification
- Ran: ...
- Result: ...

## Remaining Risks / Reply Notes
- ...
````

## Hard constraints

- Do not skip fetching the actual PR feedback before proposing fixes.
- Do not mix unrelated cleanup into a review-driven patch without saying so.
- Do not silently mark a comment as fixed unless the code change clearly addresses it.
- If a comment is better answered with explanation than code, say that explicitly.
- If `gh` auth is missing or the branch has no PR, report that blocker clearly.

## Trigger examples

- "Use $gh-address-comments to summarize this PR's review threads and tell me what to fix."
- "Fetch the GitHub review comments on my current branch PR and address the important ones."
- "Number the open PR comments, let me choose, then apply the fixes."

## Optional supporting assets

- Rust CLI: `/Users/joe/Documents/skill/rust_tools/gh_source_gate_rs`
- `assets/`
- `agents/`
