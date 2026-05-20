# Per-repository project contract

Every **governed** Python repository MUST satisfy this contract before merge or before long-running jobs.

## Required files

| File | Git | Created by |
|------|-----|------------|
| `pyproject.toml` | track | `uv init` or manual `[project]` |
| `.python-version` | track | `uv python pin <ver>` |
| `uv.lock` | track | `uv lock` |
| `.venv/` | **ignore** | `uv sync` |

### Minimal `pyproject.toml` (greenfield)

```toml
[project]
name = "my-project"
version = "0.1.0"
requires-python = ">=3.12,<3.13"
dependencies = []

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"
```

**Existing repos** (e.g. setuptools + `src/` layout): **keep** the current `[build-system]`; only add `.python-version`, run `uv lock`, `uv sync`. Do not swap hatchling in place of setuptools unless the user requests it.

### Dev dependency group

```toml
[dependency-groups]
dev = ["pytest", "ruff"]
```

```bash
uv add --dev pytest ruff    # preferred: records dev deps in pyproject
uv sync --all-groups
uv run pytest
```

### `.python-version`

Single line, e.g.:

```
3.12
```

## Standard workflows

```bash
# Clone
git clone <repo> && cd <repo>
uv sync

# Develop
uv add polars
uv run pytest
uv run python scripts/main.py

# Upgrade one package
uv lock --upgrade-package pandas
uv sync
```

## `requires-python` policy

| Project class | Pin |
|---------------|-----|
| New | `>=3.12,<3.13` |
| Legacy 3.11 | `>=3.11,<3.12` until uplifted; then move to 3.12 |
| Experimenting with 3.13+ | explicit repo pin only; never global default |

## Legacy `requirements.txt`

| State | Action |
|-------|--------|
| Exists, no lock | `uv add -r requirements.txt && uv lock` |
| Exists, lock present | Stop editing requirements; optional delete after team ack |
| CI still uses `pip install -r` | Migrate to `uv sync --frozen` (see `ci-contract.md`) |

## Layout variants

### Application with `src/`

```toml
[tool.setuptools.packages.find]
where = ["src"]
```

Run: `uv run python -m mypkg` or `uv run pytest` with `pythonpath` in `[tool.pytest.ini_options]`.

### Scripts-only repo (no package)

Use `[project.scripts]` or document `uv run python scripts/foo.py`; still require lockfile.

### Monorepo / workspace (v1.1+)

uv supports `[tool.uv.workspace]` with member packages. v1.0 runbooks assume **one lock per repo root**; for monorepos, add a follow-on doc or extend SPEC when a workspace is adopted — do not split locks across members without workspace metadata.

## Anti-patterns

| Anti-pattern | Fix |
|--------------|-----|
| `PYTHONPATH=src python3 ...` | `uv run python ...` with pytest/pythonpath config |
| `../.venv` from wrong directory name | Recreate: `rm -rf .venv && uv sync` |
| Committing `.venv` | Add to `.gitignore` |
| Multiple lockfiles | One `uv.lock` per repo root |

## Verification checklist

- [ ] `uv sync` succeeds on clean clone
- [ ] `uv run python -c "import sys; print(sys.executable)"` points inside `<repo>/.venv`
- [ ] No `pip` in project README install steps
- [ ] CI uses `uv sync --frozen`
