# Claude Code Hook 系统全量架构重构计划

## Context

基于 Claude Code vs Cursor hook 系统的深度对抗审阅，发现 Claude Code 实现存在：
- **P0 死锁风险**：状态文件不可读时 Stop block，PreToolUse 同时阻止修复 → 循环卡死
- **架构差距**：无 SessionStart/subagentStart 事件 → 连续性与 Review Gate 能力残缺
- **代码质量**：单一巨型文件（1937行）vs Cursor 模块化（11个文件）
- **平台兼容**：Windows 上文件锁过期检测失效

用户选择：**全量架构重构 + 降级兼容 + P0/架构并行修复**

---

## Phase 1: P0 基础修复（与 Phase 2 并行）

### 1.1 canonicalize 缓存（claude_hooks.rs:199）

**问题**：`repo_relative_slash_path()` 每次调用执行 `canonicalize()`，NFS 上可挂起。

**修复**：
```rust
// claude_hooks.rs 顶部添加：
static CACHED_REPO_CANONICAL: OnceLock<PathBuf> = OnceLock::new();

fn cached_canonicalize(repo_root: &Path) -> PathBuf {
    if let Some(cached) = CACHED_REPO_CANONICAL.get() {
        return cached.clone();
    }
    let normalized = repo_root.canonicalize().unwrap_or(repo_root.to_path_buf());
    let _ = CACHED_REPO_CANONICAL.set(normalized.clone());
    normalized
}
// 替换 line 199 的 (candidate.canonicalize(), repo_root.canonicalize())
```

### 1.2 session_key 缓存（claude_hooks.rs:714）

**修复**：同上模式，用 `OnceLock<String>` 缓存首次计算结果。

### 1.3 Windows 文件锁兼容（hook_state_lock.rs:116-135）

**问题**：非 Unix 平台 `is_process_alive` 永返 true，僵尸锁无法检测。

**修复**：
```rust
#[cfg(windows)]
fn is_process_alive(pid: u32) -> bool {
    use std::process::Command;
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH"])
        .output();
    match output {
        Ok(o) => !String::from_utf8_lossy(&o.stdout).contains("No tasks are running"),
        Err(_) => true,
    }
}

#[cfg(all(not(unix), not(windows)))]
fn is_process_alive(_pid: u32) -> bool { true }
```

**涉及文件**：
- `scripts/router-rs/src/claude_hooks.rs`（line 199, 686, 714）
- `scripts/router-rs/src/hook_state_lock.rs`（line 116-135）

**验证**：
```bash
cargo test -- claude_hooks::tests::repo_relative_slash_path
cargo test -- hook_state_lock::tests
```

---

## Phase 2: 模块化拆分

### 2.1 目标结构

创建 `scripts/router-rs/src/claude_hooks/` 目录：

```
claude_hooks/
  mod.rs                 (~120行) - StdioAgentHookHost enum, exports
  frag_01_host_types.rs  (~150行) - Host detection, env var helpers
  frag_02_paths_io.rs    (~200行) - Path helpers, canonicalize caching
  frag_03_state_disk.rs  (~150行) - AgentDiskState, load/save
  frag_04_review_gate.rs (~250行) - ReviewGateState, phase state machine
  frag_05_handlers.rs    (~400行) - run_pre_tool_use, run_user_prompt_submit, run_post_tool_use, run_stop
  frag_06_policy_guards.rs (~200行) - dangerous_bash_reason, path guards
  dispatch.rs            (~50行)  - event routing
  tests.rs               (~500行) - 提取测试
```

### 2.2 拆分步骤

**Step 1**: 创建 `claude_hooks/mod.rs`：
```rust
mod frag_01_host_types;
mod frag_02_paths_io;
// ... 其他模块

pub use frag_01_host_types::{StdioAgentHookHost, active_stdio_agent_hook_host};
pub use frag_04_review_gate::{ReviewGateState, bump_phase};
pub use dispatch::dispatch_claude_hook_event;
```

**Step 2**: 提取 `StdioAgentHookHost`（原 line 38-121）到 `frag_01_host_types.rs`

**Step 3**: 提取路径函数（原 line 138-210）到 `frag_02_paths_io.rs`

**Step 4**: 提取 `AgentDiskState<T>`（原 line 747-850）到 `frag_03_state_disk.rs`

**Step 5**: 提取 `ReviewGateState`（原 line 618-644）到 `frag_04_review_gate.rs`，并增强 phase 字段

**Step 6**: 提取四个 handler（原 line 435-610）到 `frag_05_handlers.rs`

**Step 7**: 提取 policy guards 到 `frag_06_policy_guards.rs`

**Step 8**: 创建 `dispatch.rs` 路由入口

**涉及文件**：
- 新建 `scripts/router-rs/src/claude_hooks/*.rs`（8个文件）
- 修改 `scripts/router-rs/src/main.rs` 引用新模块路径

**验证**：
```bash
cargo build --release
cargo test -- --test-threads=1
just check
```

---

## Phase 3: Phase 状态机增强

### 3.1 当前差距

| Claude | Cursor |
|--------|--------|
| 仅 `review_required`/`review_override` 二值 | phase 0→3 四阶段状态机 |
| Stop 后验 | subagentStart/Stop 实时门控 |

### 3.2 增强设计

```rust
// frag_04_review_gate.rs
struct ReviewGateState {
    pub version: u32,
    pub phase: u32,  // 0=未触发, 1=已armed, 2=检测到subagent调用, 3=证据满足
    pub review_required: bool,
    pub review_override: bool,
    pub independent_reviewer_seen: bool,
    pub detected_subagent_calls: Vec<String>,
    pub last_detected_at: Option<String>,
}

fn bump_phase(state: &mut ReviewGateState, target: u32) {
    state.phase = state.phase.max(target);
}
```

### 3.3 PostToolUse subagent 检测（降级方案）

```rust
// frag_05_handlers.rs
fn run_post_tool_use(repo_root: &Path, payload: &Value) -> Option<Value> {
    let tool_name = payload.get("tool_name").and_then(Value::as_str).unwrap_or("");

    if is_subagent_tool(tool_name) {
        let mut state = load_review_gate_disk(...);
        if state.phase == 1 && state.review_required {
            bump_phase(&mut state, 2);
            persist_review_gate_disk(...);
        }
    }
    None
}

fn is_subagent_tool(name: &str) -> bool {
    matches!(name, "Task" | "Agent" | "Subagent" | "delegate")
}
```

### 3.4 SessionStart 降级（UserPromptSubmit 首次注入）

```rust
// frag_05_handlers.rs
fn run_user_prompt_submit(repo_root: &Path, payload: &Value) -> Option<Value> {
    if is_first_submit_of_session(repo_root, payload) {
        let digest = compute_continuity_digest(repo_root);
        return add_context("UserPromptSubmit", &digest);
    }
    // ... review gate arm logic
}
```

**涉及文件**：
- `claude_hooks/frag_04_review_gate.rs` - phase 状态机
- `claude_hooks/frag_05_handlers.rs` - PostToolUse/UserPromptSubmit 增强

**验证**：
```bash
cargo test -- claude_hooks::tests::phase_transition
cargo test -- claude_hooks::tests::subagent_detection
```

---

## Phase 4: 环境变量统一

### 4.1 当前命名混乱

| 类型 | 示例 |
|------|------|
| 通用 | `ROUTER_RS_DISABLE_FSYNC` |
| Claude | `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE` |
| Cursor | `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE`, `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED` |
| Codex | `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX_BYTES` |

### 4.2 规范化

**新命名**：`ROUTER_RS_{HOST}_{FEATURE}_{ACTION}`

| 旧名 | 新名 |
|------|------|
| `ROUTER_RS_REVIEW_GATE_DISABLE` | `ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE` |
| `ROUTER_RS_CURSOR_HOOK_STATE_LEGACY_FULL_SWEEP` | `ROUTER_RS_CURSOR_HOOK_STATE_FULL_SWEEP_ENABLE` |

### 4.3 兼容过渡

```rust
// router_env_flags.rs
pub fn router_rs_claude_review_gate_disabled() -> bool {
    // 新名优先，旧名 fallback
    router_rs_env_enabled_default_false("ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE")
        || router_rs_env_enabled_default_false("ROUTER_RS_REVIEW_GATE_DISABLE") // legacy
}

// 启动时检测旧名，输出 deprecation warning
fn check_legacy_env_vars() {
    if std::env::var("ROUTER_RS_REVIEW_GATE_DISABLE").is_ok() {
        eprintln!("[router-rs] DEPRECATED: use ROUTER_RS_CLAUDE_REVIEW_GATE_DISABLE");
    }
}
```

**涉及文件**：
- `scripts/router-rs/src/router_env_flags.rs`
- `claude_hooks/frag_01_host_types.rs` - `review_gate_disable_env()` 返回值
- `cursor_hooks/frag_04_review_gate_runtime.rs` - env 引用

**验证**：
```bash
ROUTER_RS_REVIEW_GATE_DISABLE=1 cargo test -- review_gate_disabled
```

---

## Phase 5: 清理与测试

### 5.1 重复代码处理

`dangerous_bash_reason()` 与 Claude Code 原生权限系统重叠，但作为 defense-in-depth 保留。

添加注释说明：
```rust
/// Defense-in-depth policy check for dangerous bash commands.
/// Note: Claude Code has native permission system that may also block.
/// This hook provides earlier rejection and consistent cross-host behavior.
```

### 5.2 测试矩阵

| Event | Condition | Expected |
|-------|-----------|----------|
| PreToolUse | dangerous bash | deny |
| PreToolUse | framework path | deny |
| UserPromptSubmit | review prompt | arm (phase=1) |
| UserPromptSubmit | first submit | inject continuity |
| PostToolUse | Task tool | bump phase=2 |
| Stop | phase<3, armed | block_stop |
| Stop | Unreadable state | auto-repair, allow |
| Stop | phase>=3 | allow |

### 5.3 验证命令

```bash
# 全量测试
cargo test -- --test-threads=1

# 模块测试
cargo test -- claude_hooks::tests

# 死锁模拟
echo 'garbage' > .claude/hook-state/review_gate_test.json
cargo run -- claude hook --event=Stop
# 期望：stderr WARNING, 文件自动删除, 返回 allow

# 最终检查
just check
cargo clippy -- -D warnings
```

---

## Timeline

| Phase | Duration | Parallel |
|-------|----------|----------|
| Phase 1 (P0) | 2 days | √ 可与 Phase 2 并行 |
| Phase 2 (模块化) | 3 days | √ |
| Phase 3 (状态机) | 2 days | 依赖 Phase 2 |
| Phase 4 (Env) | 1 day | 可与 Phase 3 并行 |
| Phase 5 (清理) | 2 days | 依赖所有 |

**总计**：~10 days（并行可压缩至 ~7 days）

---

## Critical Files

| 文件 | 改动类型 |
|------|----------|
| `scripts/router-rs/src/claude_hooks.rs` → `claude_hooks/*.rs` | 拆分为 8 个模块 |
| `scripts/router-rs/src/hook_state_lock.rs` | Windows 兼容 |
| `scripts/router-rs/src/router_env_flags.rs` | 环境变量统一 |
| `scripts/router-rs/src/cursor_hooks/mod.rs` | 参考（不改） |
| `.claude/settings.json` | 确认 timeoutMs 已配置 |