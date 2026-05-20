# CI and automation contract

Automation MUST use **uv**, not `pip install` or `setup-python` + pip, for Python jobs in governed repos.

## GitHub Actions (canonical)

```yaml
jobs:
  test:
    runs-on: macos-latest   # or ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install uv
        uses: astral-sh/setup-uv@v5
        with:
          enable-cache: true

      - name: Sync dependencies
        run: uv sync --frozen

      - name: Test
        run: uv run pytest -q
```

**Rules**

- Commit `uv.lock`; CI uses `--frozen` so lock drift fails the build.
- Pin runner Python is **not required**; uv brings project interpreter per `.python-version`.
- Cache: rely on `setup-uv` cache or `actions/cache` on `~/.cache/uv`.

## Pre-commit / local hooks

```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
uv sync --frozen
uv run pytest -q
```

## Cursor / agent background commands

**Bad**

```bash
cd repo && python3 scripts/long_job.py
```

**Good**

```bash
cd repo && uv sync && uv run python scripts/long_job.py
```

## This framework repository (`skill`)

Current workflows use bare `python3` for JSON/hook tests. Migration path:

1. Add minimal `pyproject.toml` at repo root or under `.cursor/hook-tests/`.
2. `uv lock && uv sync` in CI.
3. Replace:

```yaml
run: python3 .cursor/hook-tests/test_install_codex_cli_hooks.py
```

with:

```yaml
- uses: astral-sh/setup-uv@v5
- run: uv run python .cursor/hook-tests/test_install_codex_cli_hooks.py
```

Until then, comment in workflow: interpreter MUST match `python-env-management` spec when local PATH is migrated.

## Environment variables

| Variable | Use |
|----------|-----|
| `UV_PYTHON_PREFERENCE=managed` | CI agents prefer uv Pythons |
| `UV_LINK_MODE=copy` | Rare macOS volume issues |
| `UV_CACHE_DIR` | Custom cache location (optional) |

Do **not** set `PIP_*` in CI for governed projects.

## Matrix builds

If testing multiple Python versions:

```yaml
strategy:
  matrix:
    python: ["3.11", "3.12"]
steps:
  - uses: astral-sh/setup-uv@v5
  - run: uv python install ${{ matrix.python }}
  - run: uv sync --frozen
  - run: uv run pytest
```

Each matrix leg should use the same `uv.lock` only if `requires-python` bracket allows (e.g. `>=3.11,<3.13` for both 3.11 and 3.12 jobs). Otherwise:

- use separate workflow jobs per series with different locks, or
- widen `requires-python` and re-lock once, then run `uv python install` per matrix row before `uv sync --frozen`.

`uv sync --frozen` **will fail** if the lock was resolved for 3.12 but the job interpreter is 3.11.
