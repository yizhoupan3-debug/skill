# Shell profile specification (macOS zsh)

Applies to **login + interactive** shells. Goal: **uv-only Python resolution**, **pip commands fail closed**.

## Why both `.zprofile` and `.zshrc`

| File | When loaded | Typical risk |
|------|-------------|--------------|
| `~/.zprofile` | login shells | python.org `Python.framework` prepended here |
| `~/.zshrc` | **every interactive terminal** (Cursor, Ghostty, iTerm) | `export PATH="$HOME/Library/Python/3.11/bin:..."` prepended **after** login |

Fixing only `.zprofile` leaves interactive terminals on legacy `Library/Python` or framework `python3`.

## Files and precedence

| File | Role |
|------|------|
| `~/.zprofile` | Login PATH; Homebrew `brew shellenv`; **remove** python.org framework bins |
| `~/.zshrc` | Interactive; **rewrite** leading `export PATH=...` — no `Library/Python`, no framework |
| `~/.zprofile.local` | **Normative** uv pin + PATH prepend + pip guards (sourced at end of `.zprofile`) |

Do not scatter Python PATH edits across IDE-generated blocks; consolidate in `.zprofile.local` and minimal `.zshrc` fixes.

## REMOVE from `~/.zprofile`

Delete or comment:

```zsh
for framework_py in \
  /Library/Frameworks/Python.framework/Versions/3.14/bin \
  /Library/Frameworks/Python.framework/Versions/3.11/bin
do
  [[ -d "$framework_py" ]] && path=("$framework_py" $path)
done
```

## FIX `~/.zshrc` (interactive — required)

Find the line that prepends user paths, commonly:

```zsh
export PATH="$HOME/.local/bin:...:$HOME/Library/Python/3.11/bin:$PATH"
```

**Remove** `$HOME/Library/Python/3.11/bin` (and any `Python.framework` segment). Keep non-Python entries (npm-global, IDE bins, `SKILL_FRAMEWORK_ROOT`, etc.).

After login + local overrides, **prepend** only what you still need; do not re-add `Library/Python`.

**Also append** the same `path=(${path:#*Library/Python*})` / `path=(${path:#*Python.framework*})` block at the **end** of `~/.zshrc` (after IDE PATH lines). macOS `path_helper` in `/etc/zshrc` re-injects python.org paths even when `~/.zshrc` text has no `Library/Python`.

## ADD `~/.zprofile.local` (normative template)

Run once before or right after adding this file:

```bash
uv python install 3.12
uv python pin --global 3.12
# optional: uv python install --default
```

Template:

```zsh
# === python-env-management (uv-only) ===
typeset -U path PATH

# uv shims first
[[ -d "$HOME/.local/bin" ]] && path=("$HOME/.local/bin" $path)

export UV_PYTHON_PREFERENCE="${UV_PYTHON_PREFERENCE:-managed}"
# export UV_PYTHON_PREFERENCE=only-managed

# --- pip fail-closed guards ---
pip()  { echo "python-env-management: pip 禁用 → uv add / uv sync / uv tool install"; return 1; }
pip3() { pip "$@"; }
python3() {
  if [[ "$1" == "-m" && "$2" == "pip" ]]; then
    echo "python-env-management: python -m pip 禁用 → uv add / uv sync"; return 1
  fi
  command python3 "$@"
}

# Strip macOS path_helper re-injection (/etc/paths.d python.org)
path=(${path:#*Library/Python*})
path=(${path:#*Python.framework*})

export PATH
# === end python-env-management ===
```

## Verification (new interactive terminal)

```bash
echo "$PATH" | tr ':' '\n' | grep -E 'Python.framework|Library/Python' && echo "FAIL: legacy Python on PATH" || echo "OK: PATH clean"
which -a python3
python3 --version
python3 -c 'import sys; print(sys.executable)'
uv python find
type pip3
skills/python-env-management/scripts/health-check.sh   # from skill repo root
```

Expect: `python3` → 3.12.x; executable under `~/.local/share/uv/`; `pip3` → function or not found.

## Rollback

1. Restore framework loop in `~/.zprofile` from backup.
2. Restore `Library/Python/3.11/bin` in `~/.zshrc` PATH line.
3. Remove pip guards from `~/.zprofile.local`.

## IDE / 编辑器

IDEs may inject their own PATH. After editing shell files, **restart the IDE** so integrated terminals inherit the cleaned PATH.

## Non-login interactive shells

Some embedded terminals run **interactive zsh without login** (only `~/.zshrc`, no `.zprofile`). Then `.zprofile.local` guards never load.

**Mitigation (pick one):**

1. At end of `~/.zshrc`:

```zsh
[[ -f "$HOME/.zprofile.local" ]] && source "$HOME/.zprofile.local"
```

2. Or duplicate the pip/`python3` guard block from the template into `~/.zshrc` after PATH fixes.

## Non-interactive shells

Cron/scripts should `cd` to the project root and use `uv run python …`. Do **not** use bare `python3` or legacy `.venv/bin/python` paths. Operator-local helpers (e.g. repo-specific env wrappers) belong in **your** project, not in this framework skill tree. No pip guards apply in `#!/bin/sh` scripts unless they `source` a zsh profile.
