---
name: python-env-management
description: |
  macOS Python environment governance with uv-only (Rust) package management:
  PATH single-track, per-project lockfiles, pip command ban, migration runbook,
  and health checks. Use when the user asks Python 环境管理, 版本混乱, 不用 pip,
  uv 包管理, .python-version, uv.lock, 全局包装太多, python PATH, 长期治理,
  or wants a machine-wide spec for Python on Apple Silicon—not one-off pip installs.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: true
disable-model-invocation: false
trigger_hints:
  - python 环境管理
  - python 版本混乱
  - 不用 pip
  - uv 包管理
  - uv.lock
  - .python-version
  - 全局 python 包
  - python PATH
  - pip 禁用
  - python 长期治理
  - macos python setup
  - astral uv
metadata:
  version: "1.1.1"
  platforms: [supported]
  tags:
    - python
    - uv
    - packaging
    - macos
    - path
    - venv
    - reproducibility
risk: medium
source: local

---

# python-env-management

**Owner** for machine- and repo-level Python governance on macOS: **uv is the only package manager**; `pip` / `pip3` / `python -m pip` are forbidden in operator workflows.

Canonical full specification: [`SPEC.md`](SPEC.md).

## When to use

- The user wants a **long-term** Python setup, not a one-off `pip install`
- Multiple Python versions on PATH (python.org, Homebrew, Xcode CLT, uv) cause confusion
- The user explicitly wants **Rust uv** instead of pip for dependencies
- Migrating research/course projects from global site-packages to isolated `uv.lock` environments
- CI or hooks should align with `uv run` instead of bare `python3`
- Auditing disk use from duplicate global `site-packages` trees

## Do not use

- The task is **running ML under Apple Silicon memory pressure** → use `$mac-memory-management` after the env is on uv
- The task is **experiment seeds / DVC / MLflow reproducibility** without env restructuring → use `$experiment-reproducibility`
- The task is **notebook authoring pedagogy** only → use `$jupyter-notebook`; install Jupyter via **`uv add --dev`** + **`uv sync --all-groups`**, not pip
- The task is **Rust application code** in this repo → use Cargo / `router-rs`, not Python
- The user only needs a single package in a throwaway REPL with no governance → still prefer `uvx`, never pip

## Authority stack

| Layer | Document |
|-------|----------|
| Full spec (normative) | [`SPEC.md`](SPEC.md) |
| Shell / PATH | [`references/shell-profile.md`](references/shell-profile.md) |
| Per-repo contract | [`references/project-contract.md`](references/project-contract.md) |
| pip → uv command map | [`references/command-matrix.md`](references/command-matrix.md) |
| Phased migration | [`references/migration-runbook.md`](references/migration-runbook.md) |
| CI / automation | [`references/ci-contract.md`](references/ci-contract.md) |
| Framework pip audit | [`references/framework-skill-audit.md`](references/framework-skill-audit.md) |

## Operator quick start

```bash
# Health (no pip on PATH, uv present)
skills/python-env-management/scripts/health-check.sh

# New project
mkdir -p ~/path/to/proj && cd ~/path/to/proj
uv init
uv python pin 3.12
uv add numpy pandas
uv sync
uv run python -c "import sys; print(sys.executable)"
```

## Hard rules (summary)

1. **Install / upgrade dependencies**: `uv add`, `uv sync`, `uv lock` — never `pip install`.
2. **Run code**: `uv run` or `uvx` — never bare `python3` in project dirs without checking `which python3`.
3. **CLI tools**: `uv tool install` — never `pip install --user`.
4. **Lock truth**: commit `uv.lock`; treat `requirements.txt` as import-only during migration.
5. **PATH**: uv shims first; no `Python.framework` or `Library/Python` on PATH — fix **both** `~/.zprofile` and `~/.zshrc` (interactive terminals).
6. **Default version**: pin **3.12** globally (`uv python pin --global 3.12`) unless a repo `.python-version` overrides.

## Cross-skill boundaries

- After `uv sync`, reproducibility layers in `$experiment-reproducibility` apply to **seeds, data, configs**—not to replacing uv.
- `$mac-memory-management` assumes training code already runs in the correct venv; fix env first with this skill.

## Routing

This skill is on the **cold** manifest surface. For machine governance, prefer explicit **`$python-env-management`** so routing does not fall through to generic owners.

## Maintenance

When changing normative behavior, edit **`SPEC.md` first**, then align references and run:

```bash
cargo run --manifest-path scripts/router-rs/Cargo.toml -- \
  framework skills refresh --framework-root "$PWD" --write --write-companions
cargo run --manifest-path scripts/router-rs/Cargo.toml -- \
  framework skills validate --framework-root "$PWD"
```
