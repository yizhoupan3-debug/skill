---
name: gsd-ship
description: |
  Final delivery gate with adversarial review and multi-worktree merge.
  Use when ready to ship. Runs 5-gate delivery checklist: tests, code quality,
  documentation, git clean, multi-worktree review. Mandatory 3-round RFV adversarial
  review with full lens coverage (Correctness, Security, Performance, etc.).
routing_layer: L1
routing_owner: owner
routing_gate: evidence
routing_gate_evidence: "WAVE_STATE.json global_status=completed, EVIDENCE_INDEX has entries"
routing_priority: P1
session_start: n/a
user-invocable: true
trigger_hints:
  - /gsd-ship
  - gsd ship
  - deliver
  - release
  - merge to main
metadata:
  version: "0.1.0"
  platforms: [supported]
  tags: [gsd, delivery, ship, adversarial-review, worktree]
---

# gsd-ship

Final delivery gate with adversarial review and multi-worktree merge.

## Philosophy

Ship only when:
- All tests pass
- All quality gates pass
- All documentation updated
- Git is clean
- Adversarial review passed

No shortcuts. No "we'll fix it later."

## Pre-Conditions

1. All waves completed (WAVE_STATE.json global_status=completed)
2. Verification passed (EVIDENCE_INDEX has entries)
3. Feature branch exists and is ready
4. Task directory is writable

## 5-Gate Delivery Checklist

### Gate 1: Test Coverage

```bash
# Unit tests
cargo test --all 2>&1 | tee test-results.log

# Coverage
cargo tarpaulin --out Html 2>&1 | tee coverage-results.log

# Verify thresholds
# - Unit tests: 100% pass
# - Integration tests: 100% pass
# - Coverage: ≥ 80%
```

### Gate 2: Code Quality

```bash
# Linter
cargo clippy --all-targets -- -D warnings 2>&1 | tee clippy-results.log

# Formatter
cargo fmt -- --check 2>&1 | tee fmt-results.log

# Security audit
cargo audit 2>&1 | tee audit-results.log

# Verify: 0 warnings, 0 vulnerabilities
```

### Gate 3: Documentation

```bash
# API docs
cargo doc --no-deps 2>&1 | tee doc-results.log

# Check required docs exist
ls README.md CHANGELOG.md docs/

# Verify: docs build clean, all required docs exist
```

### Gate 4: Git Clean

```bash
# Check for uncommitted changes
git status

# Verify branch name
git branch --show-current

# Verify: no uncommitted, correct branch naming
```

### Gate 5: Multi-Worktree Review

See [worktree-flow.md](worktree-flow.md) for detailed flow.

```bash
# Create review worktree
git worktree add ../worktree-review-<task_id> <branch-name>

# In worktree: full test suite
cd ../worktree-review-<task_id>
cargo test --all

# In worktree: adversarial code review
# See adversarial-review.md for full lens coverage

# Merge to main
git checkout main
git merge --no-ff <branch-name>

# Cleanup
git branch -d <branch-name>
git worktree remove ../worktree-review-<task_id>
```

## MANDATORY: Adversarial Review

Before merge, run 3-round RFV adversarial review:

### Required Lenses (from code-review-deep)

| Lens | Focus |
|------|-------|
| Correctness | Logic, edge cases, error handling |
| Security | Injection, auth, sensitive data |
| Performance | Complexity, resource leaks, caching |
| Maintainability | Readability, coupling, test coverage |
| Reliability | Error recovery, idempotency, monitoring |
| Supply Chain | Dependency audit, license compliance |

### RFV Loop Setup

```bash
printf '%s\n' '{"id":1,"op":"framework_rfv_loop","payload":{"operation":"start","repo_root":"<path>","goal":"Adversarial review before ship","max_rounds":3,"allow_external_research":true,"review_scope":"<changed-files>","fix_scope":"<scope>","verify_commands":["cargo test","cargo clippy"],"stop_when":["all lenses covered","P0/P1 findings zero","3 rounds complete"]}}' | router-rs --stdio-json
```

### Round Structure

**Round 1**: Internal review (read-only subagent)
- Correctness lens
- Security lens

**Round 2**: External research (parallel subagent)
- Performance lens
- Supply chain lens

**Round 3**: Integration review
- Maintainability lens
- Reliability lens
- Final synthesis

## SHIPPING_STATE.json

Update throughout ship process:

```json
{
  "schema_version": "gsd-shipping-state-v1",
  "task_id": "<task_id>",
  "gates": {
    "test_coverage": {
      "status": "pass|fail|pending",
      "coverage_percent": 85,
      "test_results": "path/to/test-results.log"
    },
    "code_quality": {
      "status": "pass|fail|pending",
      "clippy_results": "path/to/clippy-results.log",
      "security_scan": "path/to/audit-results.log"
    },
    "documentation": {
      "status": "pass|fail|pending",
      "checked_files": ["README.md", "CHANGELOG.md", "docs/"]
    },
    "git_clean": {
      "status": "pass|fail|pending",
      "branch_name": "feature/<task_id>",
      "uncommitted_files": []
    },
    "worktree_review": {
      "status": "pass|fail|pending",
      "worktree_path": "../worktree-review-<task_id>",
      "merge_commit": null,
      "findings": []
    }
  },
  "overall_status": "pass|fail|blocked",
  "started_at": "ISO8601",
  "completed_at": null
}
```

## Ship Complete

When all gates pass AND RFV loop closes:

1. Update SHIPPING_STATE.json with completed_at
2. Write final evidence to EVIDENCE_INDEX.json
3. Write closeout summary to SESSION_SUMMARY.md
4. Mark GOAL_STATE as completed

```bash
# Complete goal
printf '%s\n' '{"id":2,"op":"framework_autopilot_goal","payload":{"operation":"complete","repo_root":"<path>"}}' | router-rs --stdio-json

# Close RFV
printf '%s\n' '{"id":3,"op":"framework_rfv_loop","payload":{"operation":"close","repo_root":"<path>"}}' | router-rs --stdio-json
```

## Anti-Patterns

- Don't skip adversarial review
- Don't merge without all gates passing
- Don't skip multi-worktree review
- Don't skip documentation check
- Don't skip security audit
- Don't skip any verification

## Delivery Checklist

```
Pre-Ship:
□ All waves completed
□ Verification passed
□ Feature branch ready

Gate 1 - Tests:
□ Unit tests: 100% pass
□ Integration tests: 100% pass
□ Coverage ≥ 80%
□ Evidence logged

Gate 2 - Quality:
□ Clippy: 0 warnings
□ Fmt: clean
□ Audit: 0 vulnerabilities
□ Evidence logged

Gate 3 - Docs:
□ README updated
□ CHANGELOG updated
□ API docs build clean
□ Evidence logged

Gate 4 - Git:
□ No uncommitted changes
□ Correct branch name
□ Commit messages clean

Gate 5 - Worktree:
□ Worktree created
□ Full tests pass
□ Adversarial review: 3 rounds
□ All lenses covered
□ P0/P1: zero
□ Merged to main
□ Worktree cleaned

Final:
□ SHIPPING_STATE.json updated
□ EVIDENCE_INDEX complete
□ GOAL_STATE completed
□ SESSION_SUMMARY written
```
