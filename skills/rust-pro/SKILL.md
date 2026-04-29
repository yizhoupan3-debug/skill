---
name: rust-pro
description: |
  Deliver ownership-correct Rust code that compiles without unnecessary clones,
  manages lifetimes explicitly, and keeps `unsafe` blocks minimal and justified.
  Produces crates that pass `cargo clippy`, `cargo test -race`, and have zero
  unchecked `unwrap()` in library code. Use when the user asks for Rust
  development, systems programming, async Tokio services, CLI tools, or phrases
  like "Rust 项目", "所有权", "生命周期", "async Tokio".
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - rust
    - ownership
    - async
    - tokio
    - systems
risk: medium
source: local
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - Rust 项目
  - 所有权
  - 生命周期
  - async Tokio
  - Rust development
  - systems programming
  - async Tokio services
  - CLI tools
  - rust
  - ownership

---

# rust-pro

This skill owns Rust-first engineering work: ownership-safe design, async runtime selection, trait-based abstraction, and systems-level optimization.

## When to use

- The user wants to build, refactor, or debug a Rust codebase
- The task involves ownership, lifetimes, borrowing, or trait design
- The task involves async with Tokio, Axum, or other async runtimes
- Building CLI tools, web services, or systems-level software in Rust
- Best for requests like:
  - "用 Rust 写一个 CLI 工具"
  - "这个 Rust 生命周期错误怎么解"
  - "帮我设计一个 trait 层次结构"
  - "Tokio async 服务怎么搭"

## Do not use

- The task is Python/JS/TS without Rust involvement → use language-specific skills
- The task is C/C++ without Rust context
- The task is pure algorithm design without language-specific needs

## Task ownership and boundaries

This skill owns:
- Rust ownership, borrowing, and lifetime design
- trait-based abstraction and generics
- async runtime selection and patterns (Tokio, async-std)
- error handling with Result/Option, thiserror, anyhow
- Cargo workspace and dependency management
- unsafe code review and FFI boundaries

This skill does not own:
- non-Rust language tasks
- pure algorithm design without Rust context
- infrastructure and DevOps

### Overlay interaction rules

- Language-idiomatic error handling (`Result<T,E>`, `Option<T>`, thiserror, anyhow) is owned by this skill
- Cross-language error **architecture** (error taxonomies, retry/circuit-breaker, error code systems) → `$error-handling-patterns`
- Rust-specific profiling (criterion, flamegraph, cargo bench) is owned by this skill
- Rust-specific safety, borrowing, and macro issues are owned by this skill.
- Rust-specific safety, borrowing, and macro issues are owned by this skill.
- **Dual-Dimension Audit (Pre: Cargo-Toml/Feat-Logic, Post: Build-Artifact/Binary-Size Results)** → runtime verification gate

If the task shifts to adjacent skill territory, route to:

## Required workflow

1. Confirm the task shape:
   - object: Rust file, module, crate, workspace, FFI boundary
   - action: build, refactor, debug, optimize, review, migrate
   - constraints: Rust edition, target platform, async runtime, MSRV
   - deliverable: code change, design, review guidance, or migration plan
2. Check Rust edition and MSRV before using nightly-only features.
3. For repo-local Rust helper binaries launched from Python or shell bridges, compare the built artifact against `Cargo.toml`, `Cargo.lock`, and `src/**/*.rs` before execution.
4. If sources are newer than the binary, fail closed and require a rebuild instead of logging a warning or silently running stale code.
5. Run `cargo check` and `cargo clippy` after modifications.
6. Verify with `cargo test` on affected code.

## Core workflow

### 1. Intake
- Identify Rust edition, MSRV, and target platform.
- Check Cargo.toml for workspace structure, features, and dependencies.
- Inspect existing patterns (error handling, async runtime, trait usage).

### 2. Execution
- Design with ownership and borrowing correctness first.
- Use enums and pattern matching for state machines.
- Prefer `impl Trait` over `dyn Trait` when monomorphization is acceptable.
- Handle errors explicitly with `Result<T, E>`; use `?` operator.
- Keep `unsafe` blocks minimal and well-documented.

### 3. Validation / recheck
- Run `cargo clippy -- -W clippy::all` for lint checks.
- Run `cargo test` for correctness.
- Check for unused dependencies with `cargo udeps` if available.
- Verify no accidental `unwrap()` in library code.

## Capabilities

### Core Language
- Ownership, borrowing, lifetimes, and the borrow checker
- Generics, trait bounds, associated types, and GATs
- Enums, pattern matching, and algebraic data types
- Closures, iterators, and zero-cost abstractions
- Procedural and declarative macros

### Async Ecosystem
- Tokio runtime, tasks, and channels
- Axum for web services
- Tower middleware and service patterns
- Async trait methods and Pin/Future

### Systems Programming
- FFI and unsafe code boundaries
- Memory layout and alignment
- no_std embedded programming
- WASM compilation targets

### Error Handling
- `Result<T, E>` and `Option<T>` patterns
- `thiserror` for library errors
- `anyhow` for application errors
- Custom error hierarchies

### Tooling
- Cargo workspaces and features
- Clippy lints and configuration
- Rustfmt formatting
- Miri for undefined behavior detection
- cargo-deny for dependency audits

## Output defaults

Recommended structure:

````markdown
## Rust Summary
- Edition: ...
- MSRV: ...
- Async runtime: ...

## Changes / Guidance
- ...

## Validation / Risks
- Clippy: ...
- Tests: ...
````

## Hard constraints

- Do not use `unwrap()` or `expect()` in library code without justification.
- Do not use `unsafe` without documenting safety invariants.
- Do not introduce nightly-only features without checking MSRV.
- Do not ignore clippy warnings without `#[allow()]` with justification.
- In this repository, follow [`RTK.md`](/Users/joe/Documents/skill/RTK.md) for noisy `cargo check`, `cargo clippy`, and `cargo test` runs when compact output is enough.
- Prefer `&str` over `String` in function parameters when ownership isn't needed.
- **Superior Quality Audit**: For production Rust binaries, apply the runtime verification gate to verify against [Superior Quality Bar](runtime verification criteria).
- Do not clone to satisfy the borrow checker without exploring alternatives first.
- Do not run repo-local prebuilt Rust binaries when the source tree is newer than the artifact; rebuild first and restart any long-lived bridge/client that keys off that binary.

## Trigger examples

- "Use $rust-pro to build an Axum web service."
- "帮我用 Rust 写一个高性能 CLI 工具。"
- "这个 Rust 生命周期错误怎么解？"
- "设计一个 trait-based 插件系统。"
- "强制进行 Rust 构建深度审计 / 检查特性开关与产物二进制结果。"
- "Use the runtime verification gate to audit this Rust project for binary-size idealism."
