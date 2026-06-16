# Spec: unsafe/unwrap 系统性清理

**状态**: ✅ 已完成
**日期**: 2026-06-16
**作者**: planx-r

---

## 1. 问题陈述

### 1.1 当前状况

| 指标 | 生产代码 | 测试代码 | 总计 |
|------|----------|----------|------|
| `unsafe` 块 | **~20** | ~719 | ~739 |
| `.unwrap()` | **~47** | ~1,473 | ~1,520 |
| `.expect()` | — | — | 2,362 |

> **关键发现**：生产代码中的 `unsafe` 块远少于预期（仅 ~20 处，全部是 libc/Windows FFI）。所有 `unsafe { std::env::set_var/remove_var }` 调用（约 720 处）均位于 `#[cfg(test)]` 模块内，生产代码中 **零** env 写入 unsafe。

### 1.2 生产代码 unsafe 分类（~20 处）

| 类别 | 数量 | 文件 | 风险 |
|------|------|------|------|
| libc flock（文件锁） | 5 | `codex_hooks/state.rs`, `file_state_lock.rs`, `claude_code_hooks.rs` | 低 |
| libc kill（进程探测） | 6 | `process.rs`, `codex_hooks/state.rs`, `review_gate.rs`, `handlers_session.inc.rs` | 低 |
| libc getpgid/getpgrp/getppid | 4 | `process.rs`, `handlers_session.inc.rs` | 低 |
| libc waitpid | 1 | `process.rs` | 低 |
| libc setsid（via pre_exec） | 1 | `process.rs` | 低 |
| Windows API（CreateMutexW 等） | 3 | `review_gate.rs` | 低 |

**所有 unsafe 块均有详细 SAFETY 注释，逻辑正确。**

### 1.3 生产代码 unwrap 分类（~47 处）

| 类别 | 估计数量 | 风险 |
|------|----------|------|
| JSON 字段访问 `.as_object_mut().unwrap()` / `.as_str().unwrap()` | ~6 | **高** |
| `host_home_root("x").unwrap()` | ~7 | **高** |
| 文件系统操作 `fs::create_dir_all().unwrap()` / `fs::write().unwrap()` | ~15 | 中 |
| JSON 序列化 `serde_json::to_string().unwrap()` | ~5 | 低 |
| 正则编译 `Regex::new().unwrap()`（硬编码 pattern） | ~8 | 低 |
| 其他（`serde_json::from_str`, `writeln!`, etc.） | ~6 | 混合 |

### 1.4 根本原因

1. **unsafe**：系统调用（libc/Windows）无 safe wrapper，不可避免；env 操作仅在测试中使用
2. **unwrap**：开发阶段快速迭代遗留，未系统性替换为 `?` 或 `expect()`

---

## 2. 设计原则

### 2.1 最小侵入原则

- libc/Windows FFI 的 unsafe → **保留**，已有 SAFETY 注释，不可消除
- 测试中的 unsafe/unwrap → **保留**，Rust 惯用法
- 仅改生产代码中可安全替换的 unwrap

### 2.2 错误语义原则

- 替换 unwrap 时必须保持或改进错误信息质量
- 不丢失上下文（如 `host_home_root("codex").unwrap()` → `.ok_or("host 'codex' not registered in registry")?`）

### 2.3 渐进式原则

- Wave 1：高风险 unwrap 替换（JSON 字段访问、host_home_root）
- Wave 2：中风险 unwrap 替换（文件系统操作）
- Wave 3：clippy 配置固化，防止回归

---

## 3. 实施方案

### 3.1 Wave 1: 高风险 unwrap 替换（~13 处）

**目标**：替换生产代码中可能导致 panic 的 JSON/配置访问 unwrap

**P0 - JSON 字段访问**：

| 文件 | 模式 | 改法 |
|------|------|------|
| `tools/autoresearch-rs/src/helpers.rs:386,412` | `hypothesis.as_object_mut().unwrap()` | `.ok_or("expected hypothesis object")?` |
| `tools/autoresearch-rs/src/claims.rs:488` | `hypothesis.as_object_mut().unwrap()` | `.ok_or("expected hypothesis object")?` |

> 注：`projection/mod.rs:187,194` 和 `hooks.rs:1200` 的 unwrap 已有前置 `is_object()` 检查或位于测试代码中，风险可控，不改。

**P0 - host_home_root 访问**：

| 文件 | 模式 | 改法 |
|------|------|------|
| `core/host-projection/src/host_integration/projection/projection_manifest.rs:181,196,210,223,272` | `.host_home_root("x").unwrap()` | `.ok_or_else(\|\| format!("host '{}' not in registry", host))?` |
| `core/host-projection/src/host_integration/projection/projection_host_ops.rs:284,302` | `.host_home_root("claude-code").unwrap()` | 同上 |

### 3.2 Wave 2: 中风险 unwrap 替换

**P1 - 文件系统操作**（仅关键路径）：

| 文件 | 模式 | 改法 |
|------|------|------|
| `core/host-projection/src/host_integration/mod.rs:274-275` | `fs::create_dir_all().unwrap()` + `fs::write().unwrap()` | 改为 `.map_err(\|e\| format!("..."))?` |
| `core/framework-kernel/src/router_self.rs:344,407,410,412` | `fs::create_dir_all().unwrap()` / `fs::write().unwrap()` | 同上 |
| `core/runtime-core/src/framework_runtime/mod.rs:340,373` | `.unwrap()` | 改为 `.map_err(...)` 或 `.expect("...")` |

**不改的 unwrap**（保留）：
- 测试函数中的所有 unwrap
- `Regex::new("硬编码pattern").unwrap()` — pattern 是字符串字面量，已测试
- `serde_json::to_string_pretty(&simple_struct).unwrap()` — 简单结构序列化不会失败
- `writeln!(f, ...).unwrap()` — 写入 `Vec<u8>` 或 `String` 不会失败

### 3.3 Wave 3: clippy 配置固化

**修改文件**：`clippy.toml`（已存在，追加配置）

```toml
# Warn on unwrap() usage to prevent regression
unwrap-used = "warn"
```

**不添加** `#![warn(clippy::unwrap_used)]` 到各 crate — 与 `clippy.toml` 重复。

---

## 4. 验证标准

### 4.1 编译验证

- [x] `cargo build --workspace` 无新 warning
- [x] `cargo clippy --workspace` 无新 error
- [x] `cargo test --workspace --no-run` 测试编译通过

### 4.2 测试验证

- [x] `cargo test --workspace` 全部通过（535 tests）
- [x] 现有测试数量不减少

### 4.3 量化目标

| 指标 | 清理前 | 清理后 |
|------|--------|--------|
| 生产代码 unsafe 块 | ~20 | ~20（不变，不可消除） |
| 生产代码高风险 unwrap | ~13 | **0** |
| 生产代码总 unwrap | ~47 | **~35**（减少 12 处） |

---

## 5. 风险评估

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| unwrap 替换改变错误语义 | 中 | 中 | 确保错误消息清晰，不丢失上下文 |
| clippy warn 导致 CI 失败 | 中 | 低 | 先修复所有目标 unwrap 再开启 warn |
| 遗漏高风险 unwrap | 低 | 中 | 手动审查 + clippy 回归检测 |

---

## 6. 不在范围内

- libc FFI 的 unsafe（flock/kill/waitpid/setsid/getpgid/getpgrp/getppid）— 不可消除
- Windows API 的 unsafe（CreateMutexW/WaitForSingleObject/ReleaseMutex/OpenProcess/CloseHandle）— 不可消除
- `CommandExt::pre_exec` — 不可消除
- 测试代码中的所有 unsafe 和 unwrap — 保持原样
- 测试中的 env set_var/remove_var unsafe — 保持原样（已有 `test_env_sync.rs` 封装）
- 引入新 crate（如 `nix`）替代 libc — 过度工程

---

## 7. 附录

### 7.1 生产 unsafe 完整清单

| 文件 | 行号 | 类型 | SAFETY 注释 |
|------|------|------|-------------|
| `core/host-projection/src/hosts/codex_hooks/state.rs` | 118 | `libc::flock(LOCK_UN)` | ✅ |
| `core/host-projection/src/hosts/codex_hooks/state.rs` | 250 | `libc::kill(pid, 0)` | ✅ |
| `core/host-projection/src/hosts/codex_hooks/state.rs` | 329 | `libc::flock(LOCK_EX|LOCK_NB)` | ✅ |
| `core/host-projection/src/hosts/file_state_lock.rs` | 130 | `libc::flock(LOCK_EX|LOCK_NB)` | ✅ |
| `core/host-projection/src/hosts/claude_code_hooks.rs` | 857 | `libc::flock(LOCK_UN)` | ✅ |
| `core/host-projection/src/hosts/claude_code_hooks.rs` | 894 | `libc::flock(LOCK_EX)` | ✅ |
| `core/session-supervisor/src/process.rs` | 49 | `cmd.pre_exec(setsid)` | ✅ |
| `core/session-supervisor/src/process.rs` | 77 | `libc::kill(pid, 0)` | ✅ |
| `core/session-supervisor/src/process.rs` | 108 | `libc::kill(pid, 0)` | ✅ |
| `core/session-supervisor/src/process.rs` | 192 | `libc::getpgid(pid)` | ✅ |
| `core/session-supervisor/src/process.rs` | 199 | `libc::kill(target, signal)` | ✅ |
| `core/session-supervisor/src/process.rs` | 224 | `libc::waitpid(pid, &mut status, flags)` | ✅ |
| `core/host-projection/src/hosts/cursor_hooks/handlers/review_gate.rs` | 194 | Windows `CreateMutexW` | ✅ |
| `core/host-projection/src/hosts/cursor_hooks/handlers/review_gate.rs` | 210 | Windows `ReleaseMutex/CloseHandle` | ✅ |
| `core/host-projection/src/hosts/cursor_hooks/handlers/review_gate.rs` | 367 | `libc::kill(pid, 0)` | ✅ |
| `core/host-projection/src/hosts/cursor_hooks/handlers/review_gate.rs` | 401 | Windows `OpenProcess/GetExitCodeProcess` | ✅ |
| `core/host-projection/src/hosts/cursor_hooks/handlers_parts/handlers_session.inc.rs` | 654 | `libc::getpgid(pid)` | ✅ |
| `core/host-projection/src/hosts/cursor_hooks/handlers_parts/handlers_session.inc.rs` | 666 | `libc::getpgrp()` | ✅ |
| `core/host-projection/src/hosts/cursor_hooks/handlers_parts/handlers_session.inc.rs` | 678 | `libc::getppid()` | ✅ |
| `core/host-projection/src/hosts/cursor_hooks/handlers_parts/handlers_session.inc.rs` | 706 | `libc::kill(target, signal)` | ✅ |
