# router-rs Harness 缓存命中率优化计划

## Context

router-rs 处于 L3 层（MCP stdio），不直接控制 API 调用。Anthropic API 的 `cache_control` 由宿主侧管理。harness 层优化空间集中在：**tool response payload 稳定性**、**token 估算精度**、**压缩策略改进**。深度调研（9 个子代理、87 万 token）确认了 6 个可行方案，Resonix 不存在于 AI/LLM 领域。

## 关键文件

| 文件 | 变更内容 |
|------|---------|
| `core/router-rs/src/framework_runtime/runtime_view.rs` | T1-1: collected_at session 级缓存 |
| `core/router-rs/src/framework_runtime/alias.rs` | T2-1: estimate_token_count Unicode 加权; T1-2: LazyLock 缓存; T2-2: entry_prompt 分离 |
| `core/router-rs/src/framework_runtime/prompt_compression.rs` | T2-3: context_window_size 参数 |
| `core/router-rs/src/session_call_tracker.rs` | T1-3: token_usage 反馈回路 |

## Phase 1: Quick Wins（3.5 人天，无互相依赖，可并行）

### T1-1: collected_at 降级为 session 级不变值 (0.5d)

**文件**: `runtime_view.rs:216`

- 引入 `static SESSION_COLLECTED_AT: OnceLock<String>`
- 新增 `fn session_collected_at() -> &'static str`，首次调用时 `get_or_init(|| current_local_timestamp())`
- 行 216 改为 `collected_at: session_collected_at().to_string()`
- 复用已有 `LazyLock` 模式（`OnceLock` 同属 `std::sync`）

**验证**: 连续两次 `load_framework_runtime_view` 断言 `collected_at` 相同; `cargo test -p router-rs`

### T2-1: estimate_token_count Unicode 加权 (0.5d)

**文件**: `alias.rs:588-595`

替换为字节扫描：
```rust
pub(super) fn estimate_token_count(text: &str) -> usize {
    let trimmed = text.trim();
    if trimmed.is_empty() { return 0; }
    let mut weight: u64 = 0;
    for b in trimmed.bytes() {
        weight += if b >= 0x80 { 3 } else { 1 };
    }
    ((weight / 4).max(1)) as usize
}
```

- ASCII 字节权重 1，多字节 UTF-8 首字节权重 3
- 纯英文结果不变（~len/4），纯中文从 chars/4 提升到 ~3*char_count/4
- 调用点：`alias.rs:103`、`prompt_compression.rs:30,94`

**验证**: 单测覆盖纯英文/纯中文/混合/空串; `cargo test -p router-rs`

### T1-2: RUNTIME_REGISTRY.json LazyLock 内存缓存 (1d)

**文件**: `alias.rs` 中 `load_framework_alias_record`（行 158-184）

- 引入 `static REGISTRY_CACHE: LazyLock<Mutex<Option<RegistryCache>>>`
- `RegistryCache { mtime: Option<SystemTime>, data: Value }`
- 读取时先检查 mtime，未变则从缓存 clone；否则重新读取
- 复用 `codex_hooks/mod.rs:1292` 的 `DRIFT_CACHE` 模式

**验证**: 功能正确性测试; 1000 次调用耗时对比; `cargo test -p router-rs`

### T2-3: context_window_size 传递给 prompt_compression (1.5d)

**文件**: `prompt_compression.rs:11-27` + 调用方

- 签名变更：`build_framework_prompt_compression_envelope(payload, context_window_size: Option<usize>)`
- token_budget 解析增加回退：`payload["token_budget"].or_else(|| context_window_size.map(|s| s / 4))`
- 调用方（`dispatch.rs`、`runtime_ops.rs`）适配新签名

**验证**: 单测覆盖 context_window_size 有/无、优先级; `cargo test -p router-rs`

## Phase 2: Feedback Loop（3 人天，依赖 T2-1 完成）

### T1-3: token_usage 反馈回路 (3d)

**文件**: `session_call_tracker.rs:62-78,156-166` + 3 个宿主 hook 调用方

- 新增 `pub struct CacheStats { cache_read, cache_creation, input_tokens, output_tokens }`
- 扩展 `record_tool_call` 签名增加 `cache_stats: Option<CacheStats>`
- 调用点暂传 `None`（MCP stdio 当前不暴露 usage 字段）
- `default_payload` 的 `token_usage` 增加 `cache_read`、`cache_creation` 字段

**验证**: 传 None 不变行为; 传 Some 累加正确; `cargo test -p router-rs`

### T2-2: entry_prompt 字段分离 (1.5d)

**文件**: `alias.rs` 的 `build_framework_alias_envelope`（行 76-107）+ `render_framework_alias_prompt`（行 531-587）

- 新增 `fn render_framework_alias_prompt_parts(entry_contract: &Value) -> (String, String)`
- `stable_prefix`: summary + route_rules + guardrails + acceptance + skill_fallback_path
- `dynamic_context`: task / phase / status 行
- 保留 `entry_prompt` 不变（向后兼容），新增 `stable_prefix` + `dynamic_context` 字段

**验证**: stable_prefix 不含 task/phase/status; entry_prompt == stable_prefix + dynamic_context; compact 模式不输出拆分字段

## Phase 3: Data-Driven（持续，依赖 T1-3）

- 基于 `SESSION_CALL_TRACKER.json` 的 `token_usage` 数据量化验证 Phase 1 效果
- 观测指标：cache_read/total_input、stable_prefix/entry_prompt 比例

## 依赖图

```
Phase 1 (并行):
  T1-1 (0.5d) ─┐
  T2-1 (0.5d) ─┤
  T1-2 (1d)   ─┼─→ Phase 2
  T2-3 (1.5d) ─┘

Phase 2 (依赖 T2-1):
  T1-3 (3d)   ─┐
  T2-2 (1.5d) ─┼─→ Phase 3 (量化验证)
```

## 实施顺序建议

1. **T2-1** 先行（0.5d）— 改进估算精度，Phase 2 依赖
2. **T1-1 + T1-2 + T2-3** 并行（取最长 1.5d）
3. **T1-3 + T2-2** 并行（取最长 3d）
4. 总计 ~5 天（非严格串行）

## 验证策略

- 每个任务完成后 `cargo test -p router-rs` 全量通过
- Phase 1 完成后：手动启动 stdio transport，对比 MCP response payload 稳定性
- Phase 2 完成后：检查 `SESSION_CALL_TRACKER.json` 中 `token_usage` 有非零值
- 全程：不新增外部依赖，MCP tool response schema 不变
