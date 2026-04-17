# Git Workflow — Execution Flow

Use this reference when the main `SKILL.md` is not enough and you need the
detailed operational path.

## Fast path — simple commit + push

Use only when:

- the user clearly wants routine commit/push
- no branching, rebase, merge conflict, or history rewrite is requested
- `git status --short --branch` shows a normal tracking branch state

Typical sequence:

```bash
git status --short --branch
git add <files-or-.>
git commit -m "<descriptive message>"
git push
```

Exit fast path if the repo is diverged, detached, conflicted, or otherwise
ambiguous.

## Detailed workflow

### 1. Intake

Check:

- `git status --short --branch`
- `git remote -v`
- relevant `git log`, `git diff`, or `git branch -vv`

If not a repo and init is requested, use `git init`.

### 2. Prepare a clean working tree

- identify staged / unstaged / untracked changes
- stage only requested scope
- inspect commit payload with `git diff --cached`

### 3. Branch and history operations

- create/switch branches explicitly
- use coherent commit messages
- split commits only when it improves reviewability
- treat destructive operations as explicit-consent steps

### 4. Remote and publish workflow

- inspect whether `origin` exists
- add or repair remotes explicitly
- first publish usually uses:

```bash
git push -u origin <branch>
```

- before pushing to an existing remote branch, fetch if remote drift is plausible

### 5. Synchronize safely

- if behind, usually prefer rebase for linear personal branches unless repo policy prefers merge
- if conflicts appear, summarize conflict files and resolve only owned/requested areas

### 6. Rollback and recovery

Prefer:

- `git restore`
- `git revert`
- corrective commits

Use destructive rollback only with explicit instruction.

## Output template

````markdown
## Git Summary
- Repository: ...
- Branch: ...

## Actions Taken
- ...

## Current State
- Ahead/behind/diverged: ...
- Remote: ...

## Follow-up / Risks
- ...
````

## Release appendix

### Semantic versioning flow

1. decide bump type
2. update version in code
3. create annotated tag
4. push branch and tags

### GitHub release

```bash
gh release create v1.2.0 --title "v1.2.0" --notes-file CHANGELOG.md
```

### Pre-releases

Use `-alpha.N`, `-beta.N`, or `-rc.N` suffixes and keep them off the stable
branch until promotion.
