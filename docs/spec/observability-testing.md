---
parent: docs/spec.md
version: unified-v7
---

## 15. 可观测性

> **deferred** — `trace_runtime` 已实现基础 JSONL 事件流；OTel 映射与核心指标计数器待 v8 规划。实现时在此补充 schema。

---

## 16. 存储压缩

> **deferred** — 快照/增量/世代滚动方案已设计，运行期契约冻结。实现时在此补充 schema 与压缩策略。

---

## 17. 测试契约

### 17.1 覆盖率现状（2026-06-08）

| Crate | LOC | #[test] | 评级 |
|-------|-----|---------|------|
| router-rs | 95K | ~1,577 | B+ |
| B0 core crates（core-state 等） | ~10K 合计 | ~161 | B |
| codegraph-rs | 2.3K | 25 | C |
| evolution-rs | 851 | 2 | D |
| research-harness | 6K | 8 | D |
| rust_tools (6) | ~16K | ~102 | C |
| **合计** | **130K** | **~1,850** | |

### 17.2 router-rs 子模块覆盖

| 模块 | #[test] | | 模块 | #[test] |
|------|---------|---|------|---------|
| runtime_storage | 117 | | hook_policy | 97 |
| route | 66 | | session_supervisor | 54 |
| review | 49 | | execution_contract | 43 |
| web_fetch_guard | 31 | | framework_runtime | 26 |
| rfv_loop | 26 | | framework_profile | 12 |
| paper_adversarial_hook | 14 | | harness_context_signals | 11 |
| session_call_tracker | 10 | | paper_prose_hook | 10 |
| router_env_flags | 9 | | background_state | 9 |
| router_rs_observation | 8 | | stdio_transport | 7 |
| harness_operator_nudges | 7 | | schema_drift | 6 |
| mcp_pre_guard | 6 | | browser_mcp | 5 |
| runtime_registry | 5 | | html_to_markdown | 4 |
| harness_contract | 2 | | framework_maint | 2 |
| **trace_runtime** | **0** | | **router_self** | **0** |
| **runtime_envelope_ids** | **0** | | **content_extract** | **1** |

### 17.3 P0 测试缺口

| 文件 | pub fn | 说明 |
|------|--------|------|
| `codegraph-rs` (全部) | 41 | 代码图核心逻辑 |
| `router_self.rs` | 14 | 二进制安装/验证/分发 |
| `state_manager/rfv_state.rs` | 10 | RFV 状态验证 |
| `state_manager/task_pointers.rs` | 7 | 任务指针读写同步 |
| `hook_policy/bash_guard.rs` | 6 | Bash 命令安全分类 |
| `trace_runtime.rs` | 0 | 追踪运行时（855 行零测试） |

### 17.4 Smoke Test 契约

**P0**：Subagent 生命周期（5）· 关闭流程（3）· Workflow 稳定性（4）

**P1**：跨宿主一致性（3）· 资源隔离（3）

### 17.5 Schema 校验测试

- 每个宿主投影 MCP key 名：happy-path + sad-path
- 测试夹具与生产共享 `make_<host>_payload()` 工厂函数

---

## 18. Schema 索引

| Schema | 版本 | 来源模块 |
|--------|------|----------|
| `runtime-sandbox-contract-v1` | §4 | 沙箱生命周期 |
| `multi-agent-orchestration-contract-v1` | §5 | 编排单元 |
| `framework-runtime-registry-v2` | §6 | 注册表 |
| `skill-routing-runtime-v3` | routing | 路由运行时 |
| `skill-manifest-v2` | routing | Skill manifest |
| `router-rs-route-decision-v1` | §8 | 路由决策 |
| `router-rs-execute-response-v1` | §9 | 执行响应 |
| `router-rs-hook-policy-v1` | §10 | Hook 策略 |
| `router-rs-harness-contract-v1` | §14 | Harness 契约 |
| `runtime-trace-v2` | §15 | 追踪事件 |
| `router-rs-background-state-store-v1` | §9 | 后台状态 |
| `router-rs-session-supervisor-response-v1` | §9 | Session Supervisor |
| `router-rs-rfv-loop-v1` | §12 | RFV 循环 |
| `router-rs-hook-observation-v1` | §14 | Hook 观测 |
| `schema-drift-baseline-v1` | §14 | Schema 漂移 |
| `loop-registry-v1` | loop | 循环注册表 |
| `loop-run-state-v1` | loop | 循环运行状态 |
| `nl-route-adjustments-v1` | routing | NL 路由调整 |
| `closeout-record-v1` | §12 | Closeout 记录 |

---
