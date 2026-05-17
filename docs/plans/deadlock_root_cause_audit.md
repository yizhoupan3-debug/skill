---
name: deadlock-root-cause-analysis
description: Claude Code模型卡死问题深度审计报告
metadata:
  type: project
---

# Claude Code 模型使用异常卡死 - 深度审计报告

## 审计范围
- `scripts/router-rs/src/*.rs` 核心钩子实现
- `.claude/hooks/router-rs-hook.sh` 钩子入口脚本
- `.claude/settings.json` 钩子超时配置

---

## P0: 直接导致卡死的系统性缺陷

### 1. "守卫者自守"死循环 (最严重)

**位置**: `claude_hooks.rs:558-599` (`run_stop`函数)

**问题描述**:
```rust
if matches!(review_load, AgentDiskState::Unreadable) {
    return block_stop(active_stdio_agent_hook_host().hook_state_unreadable());
}
```

当状态文件（`hook_state_*.json`或`review_gate_*.json`）损坏或权限不可读时：
1. Stop钩子调用`block_stop()`阻止模型结束会话
2. PreToolUse守卫阻止模型修改`.claude/`下的任何文件
3. 模型无法修复损坏的状态文件 → **死锁**

**触发条件**:
- `.claude/hook-state/*.json`权限问题（chmod 000）
- JSON损坏（非法UTF-8、truncated write）
- 磁盘I/O错误导致文件不可读

**修复方案**: 见 `PLAN_hook_system_deadlock_fix.md` P0-2

---

### 2. 文件锁竞争导致的阻塞

**位置**:
- `runtime_storage.rs:743-761` (`acquire_runtime_path_lock`)
- `hook_state_lock.rs:37-89` (`acquire_state_lock`)

**问题描述**:
两个锁系统使用不同参数：
```rust
// runtime_storage.rs: 30次 × 100ms = 3秒
for attempt in 0..30 {
    match file.try_lock_exclusive() {
        Ok(()) => return Ok(...),
        Err(_) => thread::sleep(Duration::from_millis(100)),
    }
}

// hook_state_lock.rs: 60次 × 50ms = 3秒
for _ in 0..60 {
    // ...
    thread::sleep(Duration::from_millis(50));
}
```

当多个钩子同时运行时（PreToolUse + PostToolUse并发），可能发生锁竞争，导致：
- 最多3秒阻塞等待
- 最终返回错误而非降级处理

**潜在问题**:
- 锁失败时`step_ledger.rs`静默跳过写入（无stderr警告）
- `runtime_storage.rs`返回错误会终止钩子，可能导致Claude Code等待

---

### 3. canonicalize() 未缓存的I/O阻塞

**位置**: 多处，关键在`claude_hooks.rs:199`

**问题描述**:
```rust
(candidate.canonicalize(), repo_root.canonicalize())
```

每次钩子调用都执行`fs::canonicalize()`，在以下场景可能阻塞数秒：
- NFS/CIFS网络挂载
- 符号链接异常（循环、目标不存在）
- 高延迟存储

**已存在缓存**: `claude_hooks.rs:679`有`CACHED_CANONICAL_REPO`，但`resolve_repo_root_arg`未使用。

---

### 4. sync_all() 强制磁盘同步阻塞

**位置**: `atomic_write.rs:52-63`, `step_ledger.rs:258`

**问题描述**:
```rust
file.write_all(line.as_bytes())
    .and_then(|_| file.sync_all())  // 每次追加都强制fsync
```

每次状态变更都调用`sync_all()`，强制等待磁盘确认。在：
- 慢速磁盘（HDD、网络存储）
- 高I/O负载时
- USB/外置存储

可能阻塞50-500ms每次写入。

---

## P1: 可能导致性能问题或间接卡死

### 5. 幂等键首次扫描O(n)复杂度

**位置**: `step_ledger.rs:222-260`

**问题描述**:
虽然已添加内存缓存`IDEMPOTENCY_CACHE`，但首次写入时仍需扫描整个JSONL文件：
```rust
fn step_ledger_contains_idempotency_key(path: &Path, idempotency_key: &str) -> Result<bool, String> {
    let file = fs::File::open(path)?;
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        // 逐行解析检查
    }
}
```

在长会话（1000+条记录）时首次扫描可能耗时100-500ms。

---

### 6. stdin读取无超时保护

**位置**: `claude_hooks.rs:344-365`

**问题描述**:
```rust
fn read_stdio_agent_stdin_limited<R: Read>(reader: &mut R) -> Result<String, String> {
    let mut input = String::new();
    let mut limited = reader.take(LIMIT);
    limited.read_to_string(&mut input)  // 无超时
}
```

理论上stdin应该立即有数据，但如果Claude Code进程异常，hook进程可能无限等待stdin。

---

### 7. 路径守卫过度阻止修改

**位置**: `claude_hooks.rs` PreToolUse钩子

**问题描述**:
PreToolUse守卫可能阻止模型修改：
- `.claude/hook-state/*.json`（导致无法修复Unreadable状态）
- `.claude/settings.json`（阻止配置调整）
- `artifacts/current/`下某些文件

这与P0-1的"守卫者自守"问题配合形成死锁。

---

## P2: 边缘问题

### 8. 依赖外部进程的潜在阻塞

**位置**: `host_integration.rs:1066`

```rust
thread::sleep(Duration::from_millis(100))  // 生成器等待
```

生成器超时300秒，在异常情况下可能长时间阻塞。

### 9. tmux session_supervisor阻塞

**位置**: `session_supervisor.rs:418,697`

依赖`tmux send-keys`外部进程，如果tmux异常可能阻塞。

---

## 锁层次与死锁风险分析

**锁获取顺序**（从外层到内层）:
1. `task_write_lock.rs` - 任务级别写锁（如果存在）
2. `acquire_runtime_path_lock` - 路径锁（3秒超时）
3. `acquire_state_lock` - 状态锁（3秒超时）
4. `IDEMPOTENCY_CACHE` Mutex - 幂等缓存锁

**潜在死锁场景**:
- 如果两个钩子以不同顺序获取锁 → 无明显死锁风险（顺序一致）
- 但锁竞争可能导致长时间等待

---

## 已缓解问题（settings.json已配置）

settings.json已为所有钩子添加`timeoutMs`：
- PreToolUse: 30000ms
- PostToolUse: 60000ms
- Stop: 15000ms
- UserPromptSubmit: 30000ms

但timeout只保护Claude Code侧，不解决内部阻塞根源。

---

## 修复优先级建议

| 优先级 | 问题 | 影响 | 修复难度 |
|--------|------|------|----------|
| P0-1 | Unreadable死循环 | 会话无法结束 | 中 |
| P0-2 | 锁竞争无降级 | 最多3秒阻塞 | 低 |
| P0-3 | canonicalize未缓存 | NFS场景秒级阻塞 | 低 |
| P0-4 | sync_all()强制同步 | 每次写入50-500ms | 低 |
| P1-5 | 幂等首次扫描O(n) | 长会话首次慢 | 低 |
| P1-6 | stdin无超时 | 边缘情况 | 中 |

---

## 结论

当前系统最严重的卡死风险来自**P0-1 "守卫者自守"死循环**：Stop钩子在状态不可读时阻塞，但模型无法修复状态文件。其次是I/O层面的阻塞：canonicalize、sync_all、锁竞争。

修复方案详见 `PLAN_hook_system_deadlock_fix.md`。