# Submodule and Subtree Workflows

## Decision: Submodule vs Subtree

| Criterion | Submodule | Subtree |
|-----------|-----------|---------|
| Upstream contribution | ✅ Easy push back | ⚠️ Harder |
| Clone simplicity | ⚠️ Needs `--recurse-submodules` | ✅ Just clone |
| Pinned version | ✅ Exact commit | ⚠️ Manual |
| Merge complexity | Low (separate repo) | Higher (mixed history) |
| CI friendliness | ⚠️ Must init submodules | ✅ Self-contained |
| Best for | Shared libs, vendor deps | Embedding a project |

---

## Submodule Workflows

### Add a submodule

```bash
git submodule add <repo-url> <path>
git commit -m "feat: add <name> submodule"
```

### Clone a repo with submodules

```bash
git clone --recurse-submodules <repo-url>

# If already cloned without submodules:
git submodule update --init --recursive
```

### Update submodule to latest upstream

```bash
cd <submodule-path>
git fetch origin
git checkout <desired-branch-or-tag>
cd ..
git add <submodule-path>
git commit -m "chore: update <name> submodule to <version>"
```

### Update all submodules

```bash
git submodule update --remote --merge
```

### Remove a submodule

```bash
git submodule deinit -f <path>
git rm -f <path>
rm -rf .git/modules/<path>
git commit -m "chore: remove <name> submodule"
```

### CI configuration tip

```yaml
# GitHub Actions
steps:
  - uses: actions/checkout@v4
    with:
      submodules: recursive
```

---

## Subtree Workflows

### Add a subtree

```bash
git subtree add --prefix=<path> <repo-url> <branch> --squash
```

### Pull latest from upstream

```bash
git subtree pull --prefix=<path> <repo-url> <branch> --squash
```

### Push changes back to upstream

```bash
git subtree push --prefix=<path> <repo-url> <branch>
```

### Split subtree into its own branch (for extraction)

```bash
git subtree split --prefix=<path> -b <new-branch>
```

---

## Common Pitfalls

1. **Submodule not initialized** — Always use `--recurse-submodules` when cloning
2. **Detached HEAD in submodule** — Submodules check out specific commits, not branches; `cd` in and `git checkout <branch>` if you need to make changes
3. **Subtree merge conflicts** — Use `--squash` to keep history clean
4. **Forgetting to commit submodule pointer** — After updating a submodule, the parent repo sees a dirty state; always commit the pointer change
