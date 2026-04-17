# Advanced Git Operations Cheatsheet

## git bisect — Binary search for a bug-introducing commit

```bash
# Start bisect session
git bisect start
git bisect bad                 # Current commit is broken
git bisect good <known-good>   # Last known working commit

# Git checks out a midpoint; test it, then mark:
git bisect good   # or
git bisect bad

# Automate with a test script:
git bisect run <test-script>

# When done:
git bisect reset
```

**Key rules:**
- Always `bisect reset` before doing other work
- The test script must exit 0 for good, 1-127 (except 125) for bad, 125 for skip

---

## git reflog — Recover lost commits and branches

```bash
# Show reflog for HEAD (default)
git reflog

# Show reflog for a specific branch
git reflog show <branch>

# Recover a dropped commit
git checkout <reflog-sha>
git branch recovery-branch     # Save it to a branch

# Undo an accidental reset
git reflog                      # Find the SHA before reset
git reset --hard <sha>
```

**Key rules:**
- Reflog entries expire after 90 days (reachable) or 30 days (unreachable)
- Only exists locally — not shared via push/fetch

---

## git worktree — Multiple working trees

```bash
# Add a new worktree for a branch
git worktree add ../feature-path feature-branch

# Add a new worktree with a new branch
git worktree add -b new-branch ../new-path

# List worktrees
git worktree list

# Remove a worktree
git worktree remove ../feature-path

# Prune stale worktree info
git worktree prune
```

**When to use:** Review a PR while keeping your current branch's work intact.

---

## git cherry-pick — Apply specific commits

```bash
# Apply a single commit
git cherry-pick <sha>

# Apply a range (exclusive start, inclusive end)
git cherry-pick A..B

# Cherry-pick without committing (stage only)
git cherry-pick --no-commit <sha>

# Abort a conflicting cherry-pick
git cherry-pick --abort

# Continue after resolving conflicts
git cherry-pick --continue
```

**Key rules:**
- Creates new commits; do not cherry-pick the same commit twice on the same branch
- Prefer merge/rebase for multiple commits when possible

---

## git sparse-checkout — Partial repo checkout

```bash
# Enable sparse-checkout (cone mode, recommended)
git sparse-checkout init --cone

# Set directories to include
git sparse-checkout set <dir1> <dir2>

# Add more directories
git sparse-checkout add <dir3>

# Disable sparse-checkout
git sparse-checkout disable

# List current sparse patterns
git sparse-checkout list
```

**When to use:** Large monorepos where you only need specific directories.

---

## git stash — Advanced stash workflows

```bash
# Stash with a message
git stash push -m "WIP: feature X"

# Stash specific files
git stash push -m "partial" -- file1.ts file2.ts

# Stash including untracked files
git stash push --include-untracked

# Apply and drop in one step
git stash pop

# Apply without dropping
git stash apply stash@{2}

# Create a branch from a stash
git stash branch new-branch stash@{0}
```
