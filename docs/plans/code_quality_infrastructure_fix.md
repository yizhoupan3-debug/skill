# 代码规范与质量基础设施修复计划

## Context

对 harness 体系做了深度 review，发现架构设计（五层模型、Review Gate 状态机、宿主适配契约）非常成熟，但基础设施层存在若干关键缺口：pre-commit 钩子未激活、无 rustfmt、clippy 只覆盖主 crate、无依赖审计、无构建系统快捷方式。本计划修复这些缺口。

---

## 1. 🔧 修复 pre-commit hooks（严重 · 零成本）

**问题**：`.githooks/pre-commit` 存在但 `.git/config` 的 `hooksPath` 指向 `.git/hooks`（sample 文件），钩子从未触发。

**改动**：一行 git config
```bash
git config core.hooksPath .githooks
```

**验证**：
- `git config core.hooksPath` → `.githooks`
- 修改 `skills/*/SKILL.md` 后 `git commit`，确认 skill compiler 触发

---

## 2. 🔧 CI 加 rustfmt 检查

**问题**：25,000+ 行 Rust 代码无格式化保障，CI 无 `cargo fmt --check`。

**改动**：
- 新建 `scripts/router-rs/rustfmt.toml`：
```toml
max_width = 100
tab_spaces = 4
edition = "2021"
```
- `.github/workflows/skill-ci.yml` 加一步：
```yaml
- name: Check Rust formatting
  run: cargo fmt --manifest-path scripts/router-rs/Cargo.toml -- --check
```

不创建根 `rustfmt.toml`：各 crate edition 不同（router-rs 2021，evolution-rs 2024），根级会导致冲突。

---

## 3. 🔧 CI clippy 扩展到所有 workspace crate

**问题**：clippy 只覆盖 router-rs，`skill-compiler-rs`（3047 行）、`evolution-rs`、`rust_tools/`（8 crates）均在 clippy 之外。

**改动**：`.github/workflows/skill-ci.yml` 追加：
```yaml
- name: Clippy skill-compiler-rs (deny warnings)
  run: cargo clippy --manifest-path scripts/skill-compiler-rs/Cargo.toml --all-targets -- -D warnings

- name: Clippy rust_tools workspace (deny warnings)
  run: cargo clippy --manifest-path rust_tools/Cargo.toml --all-targets -- -D warnings

- name: Clippy evolution-rs (deny warnings)
  run: cargo clippy --manifest-path scripts/evolution-rs/Cargo.toml --all-targets -- -D warnings
```

---

## 4. 🔧 CI 加 rust_tools crate 编译检查

**问题**：`rust_tools/` 下 8 个 crate 在 CI 中零测试。

**改动**：
```yaml
- name: Test rust_tools workspace
  run: cargo test --manifest-path rust_tools/Cargo.toml
```

---

## 5. 🔧 创建 rust-toolchain.toml

**问题**：MSRV 1.84 声明在 `Cargo.toml` 中但未被 `rust-toolchain.toml` 锁定。

**改动**：新建 `scripts/router-rs/rust-toolchain.toml`：
```toml
[toolchain]
channel = "1.84.0"
components = ["rustfmt", "clippy"]
```

仅锁定 router-rs：其它 crate 无 MSRV 声明，不应被约束。

---

## 6. 🔧 添加 deny.toml + CI dependency audit

**问题**：依赖了 `libc`, `reqwest`, `rusqlite`, `tungstenite` 等网络/系统级 crate，但无自动漏洞扫描。

**改动**：
- 新建 `scripts/router-rs/deny.toml`：
```toml
[advisories]
vulnerability = "deny"
unmaintained = "warn"
notice = "warn"

[licenses]
allow = ["MIT", "Apache-2.0", "ISC", "BSD-2-Clause", "BSD-3-Clause", "CC0-1.0", "Unicode-3.0"]
unlicensed = "deny"

[bans]
multiple-versions = "warn"
```
- CI step：
```yaml
- name: Dependency audit
  run: cargo deny --manifest-path scripts/router-rs/Cargo.toml check
```
- 使用 `taiki-e/install-action@cargo-deny` 安装

---

## 7. 🔧 创建根 Justfile

**问题**：无构建系统快捷方式，需记忆长 `cargo` 命令。

**改动**：新建 `Justfile`：
```just
fmt:
    cargo fmt --manifest-path scripts/router-rs/Cargo.toml

clippy:
    cargo clippy --manifest-path scripts/router-rs/Cargo.toml --all-targets -- -D warnings

test:
    cargo test --manifest-path scripts/router-rs/Cargo.toml

test-all:
    cargo test --manifest-path scripts/router-rs/Cargo.toml
    cargo test --test policy_contracts
    cargo test --test host_integration

audit:
    cargo deny --manifest-path scripts/router-rs/Cargo.toml check

check: fmt clippy test

compile-skills:
    cargo run --manifest-path scripts/skill-compiler-rs/Cargo.toml -- --skills-root skills --source-manifest skills/SKILL_SOURCE_MANIFEST.json --apply

ci: compile-skills test-all
```

---

## 8. 🔧 添加 dependabot.yml

**问题**：依赖更新全靠手动。

**改动**：新建 `.github/dependabot.yml`：
```yaml
version: 2
updates:
  - package-ecosystem: "cargo"
    directory: "/"
    schedule:
      interval: "weekly"
  - package-ecosystem: "github-actions"
    directory: "/"
    schedule:
      interval: "weekly"
```

---

## PR 顺序建议

| PR | 内容 | 风险 |
|----|------|------|
| #0 | `git config core.hooksPath .githooks` | 零（立即执行） |
| #1 | rustfmt.toml + CI fmt check + rust-toolchain.toml | 低 |
| #2 | CI clippy 扩展 + rust_tools test | 中（可能需现修 clippy 违规） |
| #3 | deny.toml + CI deny + Justfile + dependabot.yml | 中 |

## 受影响文件清单

| 文件 | 操作 |
|------|------|
| `.git/config` | 改 `hooksPath`（CLI 命令） |
| `.github/workflows/skill-ci.yml` | 加 5-6 个 step |
| `scripts/router-rs/rustfmt.toml` | 新建 |
| `scripts/router-rs/rust-toolchain.toml` | 新建 |
| `scripts/router-rs/deny.toml` | 新建 |
| `Justfile` | 新建 |
| `.github/dependabot.yml` | 新建 |

## 验证方案

1. **pre-commit**: 改 SKILL.md → git commit → 触发 compiler
2. **CI**: 推分支 → Actions 全部绿色
3. **Justfile**: `just check` → fmt + clippy + test 通过
4. **Dependabot**: 配置后 24h 内自动检测
