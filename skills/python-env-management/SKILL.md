---

description: 'macOS Python governance with uv-only (Rust) packaging: PATH single-track, per-project uv.lock, pip ban, migration runbook.'
metadata:
  platforms:
  - supported
  tags:
  - python
  - uv
  - packaging
  - macos
  - path
  - venv
  - reproducibility
  version: '1.1.1'
name: python-env-management
scene: general
risk: medium
routing_gate: none
routing_layer: L4
routing_owner: owner
routing_priority: P3
session_start: n/a
source: local
trigger_hints:
- '.python-version'
- astral uv
- macos python setup
- pip 禁用
- python PATH
- python 版本混乱
- python 环境管理
- python 长期治理
- uv 包管理
- uv.lock
- 不用 pip
- 全局 python 包
- .python-version
- python-env-management
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

- The task is **running ML under Apple Silicon memory pressure** → answer in the current implementation context after the env is on uv（`` 已归档）
- The task is **experiment seeds / DVC / MLflow reproducibility** without env restructuring → use `$experiment-reproducibility`
- The task is **notebook authoring pedagogy** only → answer in the current context; install Jupyter via **`uv add --dev`** + **`uv sync --all-groups`**, not pip（`` 已归档）
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
- ``（已归档）在当前上下文中处理训练代码的内存约束；先用本 skill 修复环境。

## Routing

This skill is on the **cold** manifest surface. For machine governance, prefer explicit **`$python-env-management`** so routing does not fall through to generic owners.

## Maintenance

When changing normative behavior, edit **`SPEC.md` first**, then align references and run:

```bash
cargo run --manifest-path core/router-rs/Cargo.toml -- \
  framework skills refresh --framework-root "$PWD" --write --write-companions
cargo run --manifest-path core/router-rs/Cargo.toml -- \
  framework skills validate --framework-root "$PWD"
```
