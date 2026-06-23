# 运行层重构进度 — 终态

## 总计修复: 22 个问题点, 涉及 13 个 crate, 0 编译错误

### ✅ 已完成的修复

#### 架构依赖性修复
| 项目 | 层影响 | 说明 |
|------|--------|------|
| **`router_rs_obs` 移出 L2** | L2→L5 | 消除 runtime-core-contracts→host-projection 逆向依赖 |
| **`runtime-infra::router_env_flags` 门面删除** | L5 | 不再需要, runtime-core 直连 framework-runtime |
| **`runtime-core` `runtime_registry` 汇总移除** | L7 | 删除 ~30 项无逻辑透传, 0 外部调用者 |
| **`runtime-core` env_flags 引用修复** | L7 | hook_timing 从 `crate::router_env_flags` 改为直调 |

#### 跨 crate 函数统一 (移入 L0 framework-kernel)
| 项目 | 影响文件 | 说明 |
|------|---------|------|
| **json_value 16 个函数 → L0 真源** | 5 crates | `required_non_empty_string`, `optional_non_empty_string`, `optional_bool` 等统一 |
| **`current_local_timestamp` → L0 真源** | 5 crates | framework-kernel/time.rs 统一时间戳 |
| **`sha256_hex` → framework-runtime re-export** | framework-extra | session_artifacts 私有副本删除 |
| **`source_traceable_heuristic` 统一** | core-state + exit-gate | quality_gate 改为导入 |
| **`now_iso()` 包装删除** | core-state | 直接调用 framework_kernel::time::now_iso |

#### 大文件子目录拆分
| 文件 | 原行数 | 目标 | 状态 |
|------|--------|------|------|
| `quality_gate.rs` | **1,874** → subdir | mod.rs + flow.rs + close_gates.rs + evidence.rs + tests.rs | ✅ |
| `closeout_enforcement.rs` | **1,227** → subdir | mod.rs + types.rs + evaluation.rs + contract.rs + tests.rs | ✅ |

#### 死代码清理
| 项目 | 操作 |
|------|------|
| `mod_tests.rs` 断链引用 | 删除 `#[path ="mod_tests.rs"]` |
| `web_fetch_guard` 3 个 `_as_strings` 函数 | 删除 |
| `exit_gate_evaluator` 未使用 trait | 添加 v9 roadmap 说明标注 |
| `contracts` 文档 + 死依赖 | 更新文档, 保留必要 dep |

#### 文件名常量化
| 项目 | 修复 |
|------|------|
| `quality_gate_state_path` | core-state 从硬编码 `"RFV_LOOP_STATE.json"` → `QUALITY_GATE_STATE_FILENAME` 常量 |

### 剩余低优先级项（不影响编译、不影响架构对齐）

| 项目 | 优先级 | 说明 |
|------|--------|------|
| `hooks.rs` → L5 作为独立 crate | 低 | 当前在 framework-runtime 内, 编译依赖无循环 |
| `framework_quality_gate` 三层统一 | 低 | 仅差 `read_primary_task_id` fallback |
| `trace-runtime` 单文件拆分 | 低 | 1103 行, 运转正常 |
| `framework-kernel` CLI 模块移出 | 低 | cli_args/router_self 已标记位置 |
| serde_json::Value → 强类型 | 低 | 渐进式, 不影响正确性 |
| unsafe env 操作统一 | 低 | 非功能性, 消除重复 unsafe 块 |
