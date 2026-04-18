---
name: python-pro
description: Deliver production-grade Python 3.12+ code with clean async boundaries, strict typing, and modern tooling.
metadata:
  version: "2.0.0"
  platforms: [codex, antigravity, claude-code]
  tags:
    - python
    - fastapi
    - async
    - typing
    - uv
    - ruff
risk: medium
source: local
routing_layer: L4
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: n/a
trigger_hints:
  - python
  - fastapi
  - async
  - typing
  - uv
  - ruff
---

# python-pro

This skill owns Python-first engineering work across scripts, services, libraries, data pipelines, and tooling.

## When to use

- The user wants to build, refactor, or debug a Python codebase
- The task involves Python 3.12+ features, async patterns, typing, or Python-specific engineering
- The task involves FastAPI, Django, Pydantic, SQLAlchemy, or other Python frameworks
- The user wants modern tooling setup (uv, ruff, mypy, pyright, pytest)
- Best for requests like:
  - "写一个 Python 脚本/服务"
  - "用 FastAPI 做后端接口"
  - "帮我重构这个 Python 服务"
  - "uv/ruff/pytest 怎么配置"

## Do not use

- The main task is JavaScript/TypeScript without Python involvement → use `$javascript-pro` or `$typescript-pro`
- The task is primarily SQL query design rather than Python ORM/data work → use `$sql-pro`
- The task is Jupyter notebook workflow rather than Python engineering → use `$jupyter-notebook`
- The task is ML model architecture design rather than Python implementation → use `$ai-research`
- The main task is performance-first acceleration via library or algorithm substitution such as pandas → polars → use `$code-acceleration`

## Task ownership and boundaries

This skill owns:
- modern Python code structure and patterns
- async/sync design and runtime-safe refactors
- type system usage (type hints, generics, Protocol, dataclasses, Pydantic)
- Python-specific package/runtime/tooling configuration
- testing strategy with pytest and related ecosystem
- lightweight Python profiling and idiomatic local optimizations when no library/algorithm reroute is justified

This skill does not own:
- non-Python stacks
- pure SQL query optimization detached from Python
- Jupyter-specific notebook workflow
- ML model architecture design

### Overlay interaction rules

- Language-idiomatic error handling (try/except, custom Exception classes, contextlib) is owned by this skill
- Cross-language error **architecture** (error taxonomies, retry/circuit-breaker, error code systems) → `$error-handling-patterns`
- Python-specific profiling and optimization (cProfile, async, caching) is owned by this skill unless the task is performance-first acceleration dominated by hot-path rewrites or faster-library substitution, in which case use `$code-acceleration`
- Web frontend performance audits → `$performance-expert`
- **Dual-Dimension Audit (Pre: Pyproject-Config/Typing, Post: Package-Integrity/Perf-Metric Results)** → `$execution-audit-codex` [Overlay]

If the task shifts to adjacent skill territory, route to:
- `$sql-pro`
- `$jupyter-notebook`
- `$ai-research`
- `$node-backend`

## Required workflow

1. Confirm the task shape:
   - object: Python file, module, package, service, script, data pipeline
   - action: build, refactor, debug, optimize, review, migrate
   - constraints: runtime version, dependencies, deployment target, async requirements
   - deliverable: code change, optimization, fix, review guidance, or migration plan
2. Check Python version and runtime assumptions before importing or using syntax features.
3. Verify dependency management strategy (uv, pip, poetry) before adding packages.
4. Use type hints and modern patterns per the project's established conventions.
5. Validate the resulting code with the project's test and lint toolchain.

## Core workflow

### 1. Intake
- Identify Python version, package manager, and virtual environment strategy.
- Check existing project conventions (linting, type checking, testing framework).
- Inspect current module structure before imposing a new pattern.

### 2. Execution
- Prefer modern Python idioms and stdlib utilities over external dependencies.
- Use type hints consistently; prefer `typing` generics for 3.9+ and native generics for 3.12+.
- Design async boundaries explicitly; don't mix sync and async without clear reason.
- Structure modules for testability: separate I/O from logic.
- Use dataclasses or Pydantic models for structured data rather than raw dicts.

### 3. Validation / recheck
- Run type checker (`mypy` or `pyright`) and linter (`ruff`) on changed files.
- Verify compatibility with the project's minimum supported Python version.
- If performance-sensitive, profile before and after changes.
- Run pytest on affected test files.

## Capabilities

### Modern Python Features
- Python 3.12+ features including improved error messages and type system enhancements
- Advanced async/await patterns with asyncio, aiohttp, and trio
- Dataclasses, Pydantic models, and modern data validation
- Pattern matching (structural pattern matching)
- Type hints, generics, and Protocol typing
- Descriptors, metaclasses, and advanced OOP patterns
- Generator expressions, itertools, and memory-efficient processing

### Modern Tooling & Development Environment
- Package management with **uv** (fastest Python package manager)
- Code formatting and linting with **ruff** (replacing black, isort, flake8)
- Static type checking with mypy and pyright
- Project configuration with pyproject.toml
- Pre-commit hooks for code quality automation

### Testing & Quality Assurance
- Comprehensive testing with pytest and plugins
- Property-based testing with Hypothesis
- Coverage analysis with pytest-cov
- Performance benchmarking with pytest-benchmark
- Integration testing and test databases

### Profiling & Local Optimization
- Profiling with cProfile, py-spy, and memory_profiler
- Async programming for I/O-bound operations
- Multiprocessing and concurrent.futures for CPU-bound tasks
- Caching strategies with functools.lru_cache
- lightweight local Python-path optimization when the dominant win is not a library or algorithm swap

### Web Development & APIs
- **FastAPI** for high-performance APIs with automatic documentation
- Django for full-featured web applications
- Pydantic for data validation and serialization
- SQLAlchemy 2.0+ with async support
- Background task processing with Celery and Redis

### Data Science & Machine Learning
- NumPy and Pandas for data manipulation
- Matplotlib, Seaborn, and Plotly for visualization
- Scikit-learn for machine learning workflows
- Jupyter notebooks for interactive development
- Integration with PyTorch, TensorFlow

## Output defaults

Default output should contain:
- Python context and runtime assumptions
- code/refactor approach
- validation notes and compatibility risks

Recommended structure:

````markdown
## Python Summary
- Runtime: ...
- Package manager: ...
- Key dependencies: ...

## Changes / Guidance
- ...

## Validation / Risks
- Checked: ...
- Compatibility notes: ...
````

## Hard constraints

- Do not introduce Python 3.12+ syntax when the project targets earlier versions without flagging.
- Do not mix package managers (pip + poetry + uv) without explicit alignment.
- Do not add external dependencies when stdlib provides equivalent functionality.
- Do not suppress type errors with `# type: ignore` without a justifying comment.
- If async is introduced, make the async/sync boundary explicit and intentional.
- Do not use bare `except:` or `except Exception:` without re-raising or logging.
- In this repository, follow [`RTK.md`](/Users/joe/Documents/skill/RTK.md) for noisy `pytest`, lint, or tooling runs when compact output is enough.
- Preserve public module contracts unless the user asks to break them.
- **Superior Quality Audit**: For production-grade Python services, trigger `$execution-audit-codex` to verify against [Superior Quality Bar](../execution-audit-codex/references/superior-quality-bar.md).

## Trigger examples

- "Use $python-pro to build a FastAPI service with async SQLAlchemy."
- "帮我用 Python 写一个数据处理脚本。"
- "这个 Python 项目怎么配 uv + ruff + pytest？"
- "优化这段 Python 服务代码的结构和运行方式。"
- "强制进行 Python 库/服务深度审计 / 检查配置完整性与运行性能结果。"
- "Use $execution-audit-codex to audit this Python service for performance-metric idealism."
