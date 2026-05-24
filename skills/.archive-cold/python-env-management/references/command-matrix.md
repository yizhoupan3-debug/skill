# Command matrix — pip forbidden, uv native only

**Policy**: Operators and agents MUST NOT invoke `pip`, `pip3`, or `python -m pip`. Use the **uv native** column only.

**Single exception (SPEC §7 Phase 5):** one-time archive only:

```bash
/Library/Frameworks/Python.framework/Versions/3.11/bin/python3.11 -m pip freeze > ~/archive/python311-freeze.txt
```

Do not use guarded `pip3` / `python3` for this.

## Dependencies

| Legacy (forbidden) | uv native (required) | Notes |
|--------------------|----------------------|-------|
| `pip install pkg` | `uv add pkg` | Writes `pyproject.toml` + updates `uv.lock` |
| `pip install pkg==1.2` | `uv add pkg==1.2` | |
| `pip install -r requirements.txt` | `uv add -r requirements.txt` | One-time migration ingest |
| `pip uninstall pkg` | `uv remove pkg` | |
| `pip freeze` | `uv export` (per repo) | Truth is `uv.lock`; legacy global freeze → §7 exception only |
| `pip list` | `uv tree` or `uv export` | Never `uv pip list` in operator workflows |
| `pip check` | `uv sync` then `uv run python -c "import ..."` | |
| `pip install -e .` | `uv sync` | Editable implied by project root |

## Execution

| Legacy (forbidden) | uv native (required) |
|--------------------|----------------------|
| `python script.py` | `uv run python script.py` |
| `python -m pytest` | `uv run pytest` or `uv run python -m pytest` |
| `python -c '...'` | `uv run python -c '...'` |

## One-shot / no project

| Legacy (forbidden) | uv native (required) |
|--------------------|----------------------|
| `pip install pkg && python -c ...` | `uvx --from pkg python -c ...` or `uv run --with pkg python -c ...` |
| `npx`-style CLI | `uvx toolname` |

## Global CLI tools

| Legacy (forbidden) | uv native (required) |
|--------------------|----------------------|
| `pip install --user black` | `uv tool install black` |
| `pipx install black` | `uv tool install black` |

## Python versions

| Legacy (forbidden) | uv native (required) |
|--------------------|----------------------|
| pyenv / manual framework install | `uv python install 3.12` |
| `.python-version` by hand | `uv python pin 3.12` |
| — | `uv python pin --global 3.12` |
| — | `uv python list` |

## Locking

| Legacy (forbidden) | uv native (required) |
|--------------------|----------------------|
| `pip-tools compile` | `uv lock` |
| `pip install -r requirements.lock` | `uv sync --frozen` |

## Discouraged compatibility shims

These exist in uv but **must not** appear in skill examples or operator runbooks:

- `uv pip install`
- `uv pip sync`
- `uv pip compile`

Migrate callers to `uv add` / `uv sync` / `uv lock`.
