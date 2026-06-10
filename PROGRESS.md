# 进度总览

> **最后更新：2026-06-10** · v6 Phase 1 物理迁移完成，技术债全部清零

## 当前状态：v6 Phase 1 完成

### 九板块物理拆分状态

#### 已完成板块（独立 Cargo crate，DAG 无环）

| 板块 | crate | 状态 | v6 Phase 1 迁移 |
|------|-------|------|----------------|
| **B0 core-state** | `core/core-state` | ✅ 独立 | 状态管理 |
| **B0 core-policy** | `core/core-policy` | ✅ 独立 | Hook 策略 |
| **B0 core-math** | `core/core-math` | ✅ 独立 | 形式化工具链 |
| **B0 framework-kernel** | `core/framework-kernel` | ✅ 独立 | +skill_repo, +stdio_payload_types, +runtime_registry, +host_targets |
| **B1 路由引擎** | `core/routing-engine` | ✅ 独立 | +route/ 全量迁移 (6,785L)，7 hooks，63 tests |
| **B3 运行时核心** | `core/runtime-core` | ✅ 独立 | 瘦身 ~25K 行，810 tests |
| **B4 宿主投射** | `core/host-projection` | ✅ 独立 | +hosts/ 全量迁移 (~23K)，82 hooks，433 tests |
| **B5 浏览器 MCP** | `core/browser-mcp` | ✅ 独立 | +browser_mcp/ 迁移 (4,752L)，dispatch hook，8 tests |
| **B7 CLI 薄壳** | `core/router-rs-cli` | ✅ 独立 | — |
| **B8 调研** | `core/autoresearch-rs` | ✅ 独立 | — |
| **B10 代码图谱** | `core/codegraph-rs` | ✅ 独立 | — |
| **B11 自进化** | `core/evolution-rs` | ✅ 独立 | — |

#### 新增 crate

| crate | 行数 | 来源 |
|-------|------|------|
| `core/framework-profile` | 1,692 | §2.1 从 runtime-core 迁出 |

### Cargo DAG（实际）
```
router-rs-cli → {runtime-core, routing-engine, host-projection, browser-mcp}
host-projection → {core-state, core-policy, framework-kernel, routing-engine}
runtime-core → {core-state, core-policy, core-math, framework-kernel, routing-engine, framework-profile}
browser-mcp → {runtime-core, routing-engine}
routing-engine → 独立（零内部依赖）
framework-profile → 独立（serde only）
```

---

## 测试基线（2026-06-10）

| crate | passed | failed | ignored | 备注 |
|-------|--------|--------|---------|------|
| **runtime-core** | **810** | 0 | 13 | 全绿 |
| **router-rs** | **271** | 0 | 1 | 全绿 |
| **host-projection** | **433** | 0 | 0 | 全绿（从 30 failed 修复到 0） |
| **routing-engine** | **63** | 0 | 12 | 全绿 |
| **framework-profile** | **12** | 0 | 0 | 全绿 |
| **browser-mcp** | **8** | 0 | 1 | 全绿 |
| **总计** | **1,597** | **0** | 27 | |

---

## v6 Phase 1 完成清单（§2）

| 任务 | 状态 | 行数 | 关键变更 |
|------|------|------|---------|
| §2.1 framework_profile.rs 迁移 | ✅ | 1,692 | 独立 crate，serde only |
| §2.2 route/ → routing-engine | ✅ | 6,785 | 7 函数指针 hooks，fn ptr 注册表 |
| §2.3 hosts/ → host-projection | ✅ | ~23,000 | 82 OnceLock hooks，5 mirror types |
| §2.4 browser_mcp/ → browser-mcp | ✅ | 4,752 | dispatch hook 解耦循环依赖 |
| §2.5 codex sync 统一化 | ✅ | — | `--host-id` 泛化，CodexSubcommand::Sync 删除 |
| §2.6 HostCapabilities 扩展 | ✅ | 6 字段 | codex=S档(ci/batch)，其余=A/B档 |
| §2.7 中间代码去宿主化 | ✅ | — | ANEMIC_MCP 删除，registry capability 驱动 |

### 架构改进
- **hooks 解耦**：routing-engine (7 fn ptr) + host-projection (82 OnceLock)，runtime-core 在启动时注册实现
- **循环依赖打破**：browser_dispatch_hook + 函数指针注册表
- **去宿主化**：pre_tool_use_guard 改用 registry capability
- **codex sync 统一**：`framework sync-entrypoints --host-id codex`

---

## 下一步：v6 Phase 2（工具链激活）

| 任务 | 工时 | 意图 |
|------|------|------|
| §3.0 CodeGraph caller bug 修复 | 2d | I9 P0 |
| §3.1 CodeGraph 默认启用 | 1d | I9 |
| §3.2 framework_snapshot + codegraph | 1d | I9 |
| §3.3 活跃 rust_tools MCP 化 | 3d | I10 |
| §3.4 孤立 crate 清理 | 1d | I10 |
| §3.5 pdf/pptx agent 读取 | 2d | I10 |

---

## 历史基线（归档）

<details>
<summary>v5 测试基线（2026-06-09）</summary>

| crate | passed | failed | ignored |
|-------|--------|--------|---------|
| router-rs --lib | 1007 | 0 | 14 |
| runtime-core | ~100 | 0 | — |
| routing-engine | 20 | 0 | — |
| codegraph-rs | 25 | 0 | — |

</details>
