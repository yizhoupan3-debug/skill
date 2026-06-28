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

## Agent background commands

**Bad**

```bash
cd repo && python3 scripts/long_job.py
```

**Good**

```bash
cd repo && uv sync && uv run python scripts/long_job.py
```

## This framework repository (`skill`)


## Environment variables

| Variable | Use |
|----------|-----|
| `UV_PYTHON_PREFERENCE=managed` | CI agents prefer uv-managed Pythons (align with SPEC §5 / shell-profile; optional `only-managed` on operator machines) |
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
