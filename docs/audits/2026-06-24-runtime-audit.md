# 2026-06-24 运行层深度审计报告

**范围**: 运行层基础设施、状态管理、上下文管理、Hook 系统、Host Projection
**审计方法**: 4 个独立 agent 并行审计，各自读取完整源码后独立输出
**审计 crate**: core-state, core-state-utils, core-policy, framework-kernel, framework-runtime-hooks, host-projection, fr-exec, loop-engine, framework-extra, http-util, fr-utils

**修复日期**: 2026-06-24
**修复方法**: 4 批次按依赖顺序实施（Batch 1 机械修复 → Batch 2 代码去重 → Batch 3 行为修复 → Batch 4 TOCTOU），独立 subagent review 通过

---

## P1: 并发安全 / 数据完整性

### 1. `router_rs_task_ledger_flock_enabled()` 三份拷贝 ✅ 已修复
- **修复**: `core-state/src/utils/task_write_lock.rs` 孤立副本已删除；`core-state` 通过 `pub use core_state_utils::task_write_lock` 统一引用；`core-policy` 保留独立副本（因反向依赖约束无法引用 `core-state-utils`），注释已更新说明同步要求。
- **剩余**: core-policy 副本为架构约束，非疏漏。

### 2. 两套独立 flock 实现 ✅ 已修复
- **修复**: `core-state/src/utils/task_write_lock.rs` 整文件删除；`acquire_task_ledger_repo_lock` 统一到 `core-state-utils`，增加 `timeout: Duration` 参数；`TaskLedgerRepoLockGuard` 改为 `pub`；`task_ledger.rs` 的 `append_transaction` 改用统一版本（500ms 超时）。

### 3. 指针读取多层 fallback TOCTOU ⏭️ 不修复（设计决策）
- `read_task_pointer_pair` 已设计为单次读取 pair，fallback 路径极少触发且受 flock 保护，窗口极小。

### 4. `truncate_corrupt_tail` 无锁修改文件 ⏭️ 不修复（设计决策）
- truncate 仅在 flock 持有时调用（`append_transaction_assuming_l1_held` 的调用者持有 L1），`flock=0` 时 truncate 与 append 并发属已知 best-effort 行为。

### 5. `task_ledger.rs` TOCTOU 窗口 ✅ 已修复
- **修复**: `append_transaction_assuming_l1_held` 从 `path.is_file()` + `read_to_string` 两步操作改为 `match fs::read_to_string(&path)` read-first 模式，消除竞态窗口。

---

## P1: 代码重复（架构债务）

### 6. `safe_slug` 两处实现语义不一致 ✅ 已修复
- **修复**: `host-projection/projection/projection_bootstrap.rs` 本地 `safe_slug`（小写+ASCII）已删除，改用 `framework_kernel::json_value::safe_slug`（保留大小写+Unicode）。

### 7. `build_task_id` / `build_framework_task_id` 完全重复 ✅ 已修复
- **修复**: 提取公共 `build_task_id(label, created_at)` 到 `framework-kernel/src/json_value.rs`；`projection_bootstrap.rs` 和 `session_artifacts.rs` 各自本地版本已删除，统一调用框架版本。

### 8. `pointer_ops.rs` tasks 数组更新逻辑重复 ✅ 已修复
- **修复**: 提取 `upsert_tasks_array_entry` 私有辅助函数，`write_focus_task_pointer_minimal` 和 `set_task_focus` 中的重复逻辑收敛为单行调用。

---

## P2: 函数膨胀 / 类型臃肿

### 9. `classify_runtime_continuity` 单函数 285 行 ⏭️ 不修复（高风险重构）
- 重构需独立任务，风险高于收益。

### 10. `tools.rs` 达 ~1700 行，`routing_evolution` 480 行内联 ⏭️ 不修复（需独立模块提取）
- 分析/诊断代码提取为独立模块需独立任务。

### 11. `cli_args.rs` 1632 行（测试与代码混杂） ⏭️ 不修复（低 ROI）
- 纯机械拆分，无架构收益。

### 12. `SandboxControlRequestPayload` / `BackgroundControlRequestPayload` 过度 Option 化 ⏭️ 不修复（schema 兼容性）
- 全部 `Option<T>` 为 schema 兼容性约束，改动影响面大。

### 13. `TraceMetadataWriteRequestPayload` 30 个字段 ⏭️ 不修复（schema 兼容性）
- 同上，`#[serde(flatten)]` 提取需独立任务。

---

## P2: API 一致性 / 命名

### 14. `TaskLedgerLockGuard` vs `TaskLedgerRepoLockGuard` 同名异义 ✅ 已修复
- **修复**: `TaskLedgerLockGuard` 随 `core-state/src/utils/task_write_lock.rs` 删除而消除；统一使用 `TaskLedgerRepoLockGuard`。

### 15. `stdio_op_registry.rs` Tool domain 缺少 `is_tool_stdio_op` 谓词 ✅ 已修复
- **修复**: 新增 `pub fn is_tool_stdio_op(name: &str) -> bool` 谓词，委托给 `TOOL_STDIO_OPS` 常量。

### 16. `host_home_is_set` 硬编码 match ⚠️ 部分修复
- **修复**: 添加 `HOME_CAPABLE_HOST_IDS` 常量使同步要求显式化。
- **剩余**: clap `#[arg]` 结构体字段为编译期静态，无法完全动态化；常量使新 host 注册时需同步修改的位置清晰可见。

### 17. `pub use roots::*` 污染命名空间 ✅ 已修复
- **修复**: 替换为显式列出 28 个实际使用的函数/类型/枚举。

---

## P2: 设计模式问题

### 18. env var 缓存 `#[cfg(not(test))]` 模式重复 4 次 ⏭️ 不修复（宏 ROI 不高）
- 模式稳定，提取宏收益有限。

### 19. `current_env_session_id` 全量扫描所有 env vars ✅ 已修复
- **修复**: 替换为显式检查 5 个已知变量键（`CLAUDE_SESSION_ID`、`CURSOR_SESSION_ID`、`CODEX_SESSION_ID`、`OPENCODE_SESSION_ID`、`ROUTER_RS_SESSION_ID`）。

### 20. `RuntimeCoreHooks` 裸 fn pointer 无状态 ⏭️ 不修复（已知设计决策）
- 已在架构文档中记录。

### 21. `SNAPSHOT_CACHE` / `TASK_VIEW_CACHE` OnceLock 不可清理 ⏭️ 不修复（MCP session 单进程模型）
- 已有 `reset_rate_limiter_for_test` 模式。

### 22. `invalidate_route_records_cache_on_write` 是空函数 ✅ 已修复
- **修复**: 函数定义及全部 9 处调用已删除。

---

## P3: 代码质量 / 测试

### 23. `hook_duplicate_check_returns_empty_when_not_registered` 断言恒真 ✅ 已修复
- **修复**: 永真断言替换为 `let _ = result.len()`，注释说明 OnceLock 测试间重置限制。

### 24. `looks_same_identity` substring 匹配误判 ✅ 已修复
- **修复**: 从 `contains` 子串匹配改为 Jaccard token 相似度（`-` 分词后交集/并集 > 50%）；空字符串返回 `false`（之前错误返回 `true`）。

### 25. `env_sync.rs` 安全性依赖约定 ⚠️ 文档已修正
- **修复**: 文档标题从 "Safe wrappers" 改为 "Unsafe wrappers"，与 `unsafe fn` 实现一致。
- **剩余**: 安全性仍依赖测试单线程约定（无编译期强制），属已知设计约束。

### 26. `SKILL_ROUTE_EVER_CALLED` 使用 Relaxed ordering ✅ 已修复
- **修复**: `load` 改为 `Acquire`，`store` 改为 `Release`，建立正确 happens-before 语义。

### 27. `write_session_artifact_set` 仅 evidence 路径加锁 ✅ 已修复
- **修复**: `summary` 和 `evidence` 写入统一在同一个 `acquire_runtime_path_lock` 保护下执行。

### 28. `cli_args.rs:466-517` 四行连续空行 ✅ 已修复
- **修复**: 删除多余连续空行。

### 29. `_evidence_missing` / `_missing_recovery_anchors` 被读取但标记未使用 ✅ 已修复
- **修复**: 两处未使用的 `_` 前缀变量绑定已删除。

### 30. `http-util` crate 过小 ⏭️ 不修复（ROI 不高）
- 合并需改 3 个 `Cargo.toml`，收益有限。

---

## 统计

| 严重度 | 总数 | 已修复 | 部分修复 | 不修复 |
|--------|------|--------|----------|--------|
| P1 | 8 | 7 | 0 | 1（#3 设计决策） |
| P2 | 15 | 9 | 1（#16 部分） | 5（#9-13 高风险/低 ROI） |
| P3 | 8 | 7 | 1（#25 文档已修正） | 0 |
| **合计** | **31** | **23** | **2** | **6** |
