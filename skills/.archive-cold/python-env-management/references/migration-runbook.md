# Migration runbook — phased, reversible

Target end state: **uv-only**, python.org frameworks **off PATH**, active repos on **`uv.lock`**.

## Phase 0 — Stop-loss (day 0)

**Actions**

1. Install/upgrade uv: `curl -LsSf https://astral.sh/uv/install.sh | sh` or keep `~/.local/bin/uv`.
2. Add pip guards → `references/shell-profile.md` template into `~/.zprofile.local`.
3. Stop all optional long-running `Python.app` jobs (seed sweeps, full pipelines).

**Verify**

```bash
type pip3   # function + returns 1 on invoke
echo "$PATH" | tr ':' '\n' | grep -E 'Python.framework|Library/Python' && echo "WARN: fix .zshrc in Phase 1" || echo "OK"
```

**Rollback**: remove guards only.

---

## Phase 1 — PATH single-track (day 1)

**Actions**

1. Remove framework bins from `~/.zprofile` (see shell-profile).
2. Remove `~/Library/Python/3.11/bin` from `~/.zshrc`.
3. Run:

```bash
uv python install 3.12
uv python pin --global 3.12
uv python install --default   # optional
```

4. New terminal session.

**Verify**

```bash
skills/python-env-management/scripts/health-check.sh
```

**Exit**: `python3` is 3.12.x; pip blocked.

**Rollback**: restore PATH blocks from git/backup of dotfiles.

---

## Phase 2 — Pilot repository (week 1)

Pick one high-churn repo (e.g. `~/Documents/research/made/code`).

**Actions**

```bash
cd <pilot>
uv python pin 3.12
```

**If `pyproject.toml` already has `[project.dependencies]`** (e.g. `made/code`):

```bash
uv lock && uv sync
# Do NOT uv add -r requirements.txt — avoids duplicate/conflicting pins
```

**If no `pyproject.toml`:**

```bash
uv init
uv add -r requirements.txt   # if present
uv lock && uv sync
```

```bash
uv run pytest    # or project test command
```

Replace README / scripts: `python` → `uv run python`.

**Exit**: tests pass; no manual `pip` in docs.

---

## Phase 3 — Active repositories (week 2–4)

| Repo pattern | Steps |
|--------------|-------|
| Has `pyproject.toml` | `uv lock && uv sync` |
| Only `requirements.txt` | `uv init` + `uv add -r requirements.txt` |
| Stray `.venv` wrong path | delete `.venv`, `uv sync` |
| Cursor agent commands | prefix `uv run` |

**Inventory command** (operator):

```bash
find ~/Documents ~/Developer -maxdepth 4 -name pyproject.toml 2>/dev/null
```

**Exit**: all active repos have committed `uv.lock`.

---

## Phase 4 — Legacy runtime drain (week 4+)

**Actions**

1. `pgrep -lf 'Python.app'` until only uv-based runs remain.
2. Search dotfiles/scripts for `/Library/Frameworks/Python` hard paths.
3. Update IDE run configs to `uv run`.

**Exit**: no accidental framework `Python.app` for daily work.

---

## Phase 5 — Disk reclaim (month 2+, optional)

**Precondition**: 30 days without needing framework global packages.

**Actions**

1. **Archive global packages** (pick one):
   - **Preferred**: `uv export` from each migrated repo (truth already in `uv.lock`).
   - **One-time legacy exception** (SPEC §7): only if a framework install still exists and no repo lock covers those packages:

```bash
mkdir -p ~/archive
/Library/Frameworks/Python.framework/Versions/3.11/bin/python3.11 -m pip freeze \
  > ~/archive/python311-freeze-$(date +%Y%m%d).txt
```

   Use the **full framework path** — not guarded `python3` / `pip3`. Do not run this after uninstall or for routine work.

2. Remove `~/Library/Python/3.9`, `3.11`, `3.14` if empty usage.
3. Uninstall python.org 3.14 (and optionally 3.11) via macOS installer UI.
4. Optional: `brew uninstall python@3.12` if redundant.

**Verify**

```bash
uv python list
du -sh ~/.local/share/uv
```

**Rollback**: reinstall python.org + restore PATH (not recommended).

---

## Risk register

| Risk | Mitigation |
|------|------------|
| Kill running research job | Phase 0 wait for jobs |
| Wrong torch/CUDA wheel | `uv add torch` inside project; document platform in lock |
| CI still uses pip | Phase 3 includes workflow edits |
| Homebrew python needed by formula | Leave brew installed; keep off PATH |

---

## Success metrics

| Metric | Target |
|--------|--------|
| `pip3` on PATH | absent or guarded |
| Governed repos with `uv.lock` | 100% active |
| Global site-packages growth | zero new installs |
| Duplicate framework disk | reclaimed or quiescent |
