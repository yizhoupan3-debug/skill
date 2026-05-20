# Multi-Worktree Review Flow

Perform full review in isolated worktree, then merge to main.

## Why Worktree?

1. **Isolation**: Review doesn't pollute main branch
2. **Parallelism**: Can run multiple worktrees for different review types
3. **Clean slate**: Fresh environment, no cached state
4. **Rollback**: Easy to discard and start over

## Worktree Lifecycle

```
┌─────────┐   create    ┌──────────┐   review    ┌──────────┐   merge   ┌─────────┐
│  NONE   │ ──────────▶ │ CREATED  │ ──────────▶ │ REVIEWED │ ────────▶ │ MERGED  │
└─────────┘              └──────────┘             └──────────┘           └─────────┘
                              │                       │                      │
                              │                       │                      │
                              │ discard               │ findings             │ cleanup
                              ▼                       ▼                      ▼
                         ┌──────────┐           ┌──────────┐          ┌─────────┐
                         │ DISCARD  │           │  FIXING  │          │ CLEANED │
                         └──────────┘           └──────────┘          └─────────┘
```

## Step-by-Step Flow

### Step 1: Create Feature Branch

```bash
# Ensure you're on main (or base branch)
git checkout main
git pull origin main

# Create feature branch
git checkout -b feature/<task-id>
git push -u origin feature/<task-id>
```

### Step 2: Create Review Worktree

```bash
# Create worktree directory
mkdir -p ../worktree-review-<task-id>

# Add worktree
git worktree add ../worktree-review-<task-id> feature/<task-id>

# Verify
git worktree list
```

### Step 3: In Worktree - Run Full Test Suite

```bash
cd ../worktree-review-<task-id>

# Full test suite
cargo test --all 2>&1 | tee test-results.log

# Check results
if [ $? -eq 0 ]; then
  echo "TESTS PASSED"
else
  echo "TESTS FAILED"
  # Log to SHIPPING_STATE.json
fi
```

### Step 4: In Worktree - Run Linters

```bash
cd ../worktree-review-<task-id>

# Clippy
cargo clippy --all-targets -- -D warnings 2>&1 | tee clippy-results.log

# Format
cargo fmt -- --check 2>&1 | tee fmt-results.log

# Audit
cargo audit 2>&1 | tee audit-results.log
```

### Step 5: In Worktree - Adversarial Code Review

See [adversarial-review.md](adversarial-review.md) for full lens coverage.

```bash
cd ../worktree-review-<task-id>

# Run code-review-deep on changed files
# Lens: Correctness, Security, Performance, etc.

# Log findings
echo "FINDINGS:" > findings.log
cat <<EOF >> findings.log
[P0] path/to/file:issue - severity - impact
[P1] path/to/file:issue - severity - impact
[P2] path/to/file:issue - severity - impact
EOF
```

### Step 6: Handle Findings

**P0/P1 Findings** (must fix):
```bash
cd ../worktree-review-<task-id>

# Fix each finding
$EDITOR path/to/file

# Re-run verification
cargo test path/to/file
cargo clippy -p package

# Commit fix
git add path/to/file
git commit -m "fix: address P0/P1 findings

- Fix issue 1
- Fix issue 2
"
```

**P2 Findings** (consider fixing):
```bash
# Document P2 findings
echo "P2 findings (accepted risk):" >> findings.log
cat <<EOF >> findings.log
- [P2] path/to/file:issue - accepted risk
EOF
```

### Step 7: Merge to Main

```bash
# Ensure main is up to date
git checkout main
git pull origin main

# Merge with merge commit
git merge --no-ff feature/<task-id> -m "Merge feature/<task-id>: <description>"

# Capture merge commit SHA
MERGE_COMMIT=$(git rev-parse HEAD)

# Push main
git push origin main
```

### Step 8: Cleanup

```bash
# Delete local branch
git branch -d feature/<task-id>

# Remove worktree
git worktree remove ../worktree-review-<task-id>

# Remove merge commit source branch from remote
git push origin --delete feature/<task-id>

# Verify cleanup
git worktree list
git branch -a
```

## Error Handling

### Worktree Already Exists

```bash
# List existing worktrees
git worktree list

# Remove stale worktree
git worktree remove ../worktree-review-<task-id> --force

# Create fresh
git worktree add ../worktree-review-<task-id> feature/<task-id>
```

### Merge Conflicts

```bash
# Check for conflicts
git merge feature/<task-id> --no-commit

# If conflicts exist
git status

# Resolve conflicts
$EDITOR conflicted-file
git add conflicted-file

# Complete merge
git commit -m "Merge feature/<task-id> with conflict resolution"
```

### Push Rejected

```bash
# Fetch and rebase
git fetch origin
git rebase origin/main

# Force push (if safe)
git push --force-with-lease origin feature/<task-id>
```

## State Updates

### SHIPPING_STATE.json Updates

```json
{
  "worktree_review": {
    "status": "completed",
    "worktree_path": "../worktree-review-<task-id>",
    "test_results": "test-results.log",
    "clippy_results": "clippy-results.log",
    "findings": {
      "P0": 0,
      "P1": 0,
      "P2": 2,
      "accepted_risks": 2
    },
    "merge_commit": "abc123def456",
    "cleanup_completed": true
  }
}
```

## Verification Commands

```bash
# Verify worktree created
git worktree list | grep "worktree-review"

# Verify branch exists
git branch -r | grep "feature/<task-id>"

# Verify tests pass in worktree
cd ../worktree-review-<task-id> && cargo test --all

# Verify merge
git log --oneline -1
git log --merges --oneline -1

# Verify cleanup
git worktree list | grep -v "worktree-review"
git branch -a | grep "feature/<task-id>" || echo "Branch cleaned"
```
