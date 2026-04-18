---
name: git-workflow
description: |
  安全执行 Git：commit、push、rebase、tag、recover、分支策略与推送失败排查。
  Use when the user wants to publish or repair repository state without risking unrelated work.
metadata:
  version: "2.0.0"
  platforms: [codex, antigravity, claude-code]
  tags:
    - git
    - branching
    - commit
    - remote
    - push
    - cherry-pick
    - bisect
    - reflog
    - worktree
    - submodule
    - gitignore
    - hooks
    - branch-strategy
routing_layer: L2
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
short_description: Safely execute Git operations and remote sync
trigger_hints:
  - 整理提交推送
  - 分支为什么推不上去
  - git push 失败
  - rebase
  - bisect
allowed_tools:
  - git
  - shell
approval_required_tools:
  - git push
  - destructive git
filesystem_scope:
  - repo
network_access: conditional
artifact_outputs:
  - SESSION_SUMMARY.md
bridge_behavior: mobile_complete_once
---
# git-workflow

This skill owns practical Git execution when repository state, branch safety, and publishability matter.

## When to use

- The user wants to initialize or inspect a Git repository
- The user wants to branch, commit, rebase, merge, stash, tag, or push safely
- The user wants to connect or repair a remote workflow
- The user wants help publishing or synchronizing local work
- The user needs advanced Git operations: cherry-pick, bisect, reflog, worktree, submodule, subtree, sparse-checkout
- The user wants to design or evaluate a branching strategy (trunk-based, GitHub Flow, gitflow)
- The user wants to set up or debug `.gitignore` rules
- Best for requests like:
  - "把这个目录初始化成 git 仓库"
  - "整理这些改动并提交推送"
  - "这个分支为什么 push 不上去"
  - "用 bisect 定位这个 regression"
  - "帮我配 .gitignore"
  - "该用 trunk-based 还是 gitflow？"

## Do not use

- The main task is PR review-comment handling → use `$gh-address-comments`
- The main task is failing GitHub Actions triage → use `$gh-fix-ci`
- The main task is release engineering (versioning, changelog, publish orchestration) → use `$release-engineering`
- The main task is writing git hooks tooling (husky, lint-staged, commitlint) → use `$git-hooks-quality-gate` (when available)
- The user wants non-Git source control help
- The task is broad release/project planning rather than Git operations

## Task ownership and boundaries

This skill owns:
- repository state inspection
- branch and commit preparation
- remote setup and publish flows
- safe synchronization and rollback planning
- conflict-aware Git operations
- advanced operations: cherry-pick, bisect, reflog, worktree, stash workflows
- submodule and subtree management
- sparse-checkout configuration
- `.gitignore` design and debugging
- branching strategy design and evaluation (trunk-based, GitHub Flow, gitflow)

This skill does not own:
- PR review-thread management
- CI log diagnosis
- code implementation unrelated to Git workflow
- release versioning, changelog, and publish orchestration
- git hooks tooling (husky, lint-staged, commitlint)

If the task shifts to adjacent skill territory, route to:
- `$gh-address-comments` for PR comments
- `$gh-fix-ci` for PR check failures
- `$release-engineering` for versioning and release pipeline design
- `$github-actions-authoring` for CI/CD workflow files

## Required workflow

1. Inspect repository state before changing history.
2. Preserve unrelated user work unless explicitly told otherwise.
3. Use explicit refs, branch names, and remotes.
4. Confirm ahead/behind/diverged status before publishing.
5. Prefer reversible operations unless the user explicitly asks for destructive cleanup.

## Fast path — 简单 commit + push

Use this when the user clearly wants a routine commit-and-push:

1. `git status --short --branch`
2. `git add ... && git commit -m "..."`
3. `git push` (or `git push -u origin <branch>` on first publish)

Use the fast path only when there is no branching/rebase/conflict/history-rewrite
request and the status is not ambiguous. If `git status` shows divergence,
detached HEAD, or conflicts, switch to the detailed workflow in the references.

## Output defaults

Default output: repo/branch state, actions taken, current sync status, and any
follow-up risk. Use the compact markdown templates in the references when the
result needs more structure.

## Reference map

- [references/git-execution-workflow.md](references/git-execution-workflow.md) — detailed intake / publish / sync / rollback flow, fast-path notes, output template, release appendix
- [advanced-operations.md](references/advanced-operations.md) — bisect, reflog, worktree, cherry-pick, sparse-checkout, stash
- [branching-strategies.md](references/branching-strategies.md) — trunk-based, GitHub Flow, gitflow comparison
- [submodule-subtree.md](references/submodule-subtree.md) — submodule and subtree workflows

## Hard constraints

- Do not discard or overwrite unrelated user changes without explicit approval.
- Do not push blindly without understanding branch/remote state.
- Do not use destructive Git operations by default.
- In this repository, follow [`RTK.md`](/Users/joe/Documents/skill/RTK.md) when `git status`, `git diff`, or similar commands would otherwise dump large low-signal output.
- Use explicit branch and remote names in commands when publishing.
- If the repo state is unclear, inspect first and say so.
```bash
git cherry-pick <sha>             # apply single commit to current branch
git cherry-pick <sha1>..<sha3>    # range (exclusive start)
git cherry-pick --no-commit <sha> # stage changes without committing
```
- Use for hotfix backports; prefer merge/rebase for regular integration

### `git worktree` — Parallel Working Directories
```bash
git worktree add ../feature-branch feature-branch
git worktree list
git worktree remove ../feature-branch
```
- Ideal for reviewing PRs while keeping current branch untouched

### `git submodule` / `git subtree`
- **submodule**: external repo as pinned dependency; `git submodule update --init --recursive`
- **subtree**: merge external repo into subdirectory; `git subtree add --prefix=vendor/lib <url> main --squash`
- Prefer subtree for simpler workflows; submodule for strict version pinning

### `git reflog` — Recovery
```bash
git reflog                        # show recent HEAD movements
git checkout <reflog-sha>         # recover lost commits
git branch recovery-branch <sha>  # save recovered state
```
- Reflog entries expire after 90 days (default); act promptly

### Interactive Rebase (`rebase -i`)
```bash
git rebase -i HEAD~5              # rewrite last 5 commits
# Commands: pick, reword, edit, squash, fixup, drop, reorder
```
- Use to clean up commit history before push
- Never rebase shared/public branches without team agreement
- `--autosquash` works with `fixup!` / `squash!` commit prefixes

### `.gitattributes` and Git LFS
```gitattributes
*.png filter=lfs diff=lfs merge=lfs -text
*.psd filter=lfs diff=lfs merge=lfs -text
```
- `git lfs install` → `git lfs track "*.psd"` → commit `.gitattributes`
- Use for binary assets > 1 MB to avoid repo bloat

## Release Management

### Semantic Versioning Workflow
1. Decide version bump: `major.minor.patch` based on change scope
2. Update version in code (package.json, pyproject.toml, etc.)
3. Create annotated tag: `git tag -a v1.2.0 -m "Release v1.2.0"`
4. Push with tags: `git push origin main --tags`

### Changelog Generation
- Conventional Commits: `feat:` / `fix:` / `breaking:` prefix convention
- Tools: `git-cliff`, `conventional-changelog`, `standard-version`, `release-it`
- Auto-generate: `npx git-cliff -o CHANGELOG.md`

### GitHub Release
```bash
gh release create v1.2.0 --title "v1.2.0" --notes-file CHANGELOG.md
gh release create v1.2.0-beta.1 --prerelease  # pre-release
```
- Attach build artifacts: `gh release upload v1.2.0 dist/*.tar.gz`

### Pre-release Management
- Use `-alpha.N`, `-beta.N`, `-rc.N` suffixes
- Keep pre-releases on feature/release branches; merge to main for stable

## Hard constraints

- Do not discard or overwrite unrelated user changes without explicit approval.
- Do not push blindly without understanding branch/remote state.
- Do not use destructive Git operations by default.
- Use explicit branch and remote names in commands when publishing.
- If the repo state is unclear, inspect first and say so.

## Trigger examples

- "Use $git-workflow to initialize this folder and publish it safely."
- "Create a branch, commit only the relevant files, and push."
- "检查这个仓库为什么和远程分叉了。"
- "用 git bisect 定位哪个 commit 引入了这个 bug。"
- "帮我设计分支策略，该用 trunk-based 还是 gitflow？"
- "这个 submodule 怎么更新到最新？"

## References

Detailed cheatsheets for advanced Git operations are in `references/`:

- [advanced-operations.md](references/advanced-operations.md) — bisect, reflog, worktree, cherry-pick, sparse-checkout
- [branching-strategies.md](references/branching-strategies.md) — trunk-based, GitHub Flow, gitflow comparison
- [submodule-subtree.md](references/submodule-subtree.md) — submodule and subtree workflows
