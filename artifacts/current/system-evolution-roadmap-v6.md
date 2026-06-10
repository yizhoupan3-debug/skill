# Roadmap v6: 彻底统一

> **版本**: v6.1 | **基线**: v5-final (1426 tests passed, 2026-06-09)
> **方法**: 24-agent 深度审计（4 轮，覆盖架构/代码/宿主/工具链/同步/配置）+ 全维度工具链扫描 (2026-06-10)
> **v6.1 更新**: Phase 2/3 详细 spec 化；§8 全维度审计报告（编译/测试/性能/质量/依赖/安全/配置/磁盘/Git 9 维度）

---

## §0 设计意图（全部意图，按优先级排序）

> 以下 12 条是 v6 的**全部设计意图**。每条后面标注实现所在的 Phase。任何 Phase 中的任务如果不能服务于某条意图，应被删除。

### I1. 配置只放用户级 [Phase 0]
所有宿主的 MCP 配置、hooks 配置、settings 配置**只放用户级**（`~/.config/`、`~/`），不在项目目录下放宿主配置文件。项目级唯一的特殊是 **dev-exempt 豁免路径**。

**负面案例**：当前 `.claude/mcp.json`、`.gemini/mcp.json`、`.opencode/opencode.json`、`.codex/config.toml`、`.mcp.json` 都是项目级配置。

### I2. 清除一切过期入口和残留 [Phase 0]
退役宿主别名、退役配置文件、debug 日志、node_modules、hook-state 堆积、重复文件——全部清除。

**当前过期清单**：
- `.claude/.framework-projection-desktop.json`（退役）
- `.claude/mcp.README.md`（退役说明）
- `.claude/mcp.json.example`（模板过期）
- `.cursor/debug-f45035.log`（debug 残留）
- `.opencode/node_modules/` + `package.json` + `package-lock.json`（不应在项目中）
- `.claude/hook-state/` 186 个、`.cursor/hook-state/` 371 个、`.codex/hook-state/` 30 个文件

### I3. Goal state 严格会话级 [Phase 0]
Goal state 的生命周期：**创建 → 工作 → 显式完成 → 清理**。

- 每个 goal 绑定 session_id
- Agent 通过 `goal_state_manage(operation="complete")` 显式结束 goal → 自动清理该 goal 的磁盘文件
- 同一 session 可有多个 goal（完成一个后发起新的）
- 新 session 不读旧 session 的 goal
- `framework_snapshot` 的 `registered_tasks` 只显示当前 session 的 goal

### I4. MCP 服务器全局统一 [Phase 0]
4 个 MCP server（router-rs-framework / mcp-codegraph / paperplain / browser-mcp）在**所有 5 个宿主**中统一注册。注册通过 `host-integration install` 自动完成，不手动配置。

### I5. runtime-core 82K → 21K [Phase 1]
route/ (6.7K) + hosts/ (24K) + browser_mcp/ (4.7K) + host_integration/ (6K) = 41K 行迁移至目标 crate。迁移后 runtime-core 仅保留核心生命周期/存储/trace/closeout。

### I6. 中间代码零宿主名硬编码 [Phase 1] ✅ 达标 (2026-06-11)
runtime-core（非 hosts/ 目录）和 core-policy 中不允许出现任何宿主名字符串（除退役别名兼容层）。所有宿主差异通过 registry capabilities + HostProvider trait 驱动。

**当前残留**：7 处生产代码 + 15 个 legacy env + 退役别名 ~25 处。

### I7. codex sync 统一化 [Phase 1]
删除 `codex sync` 子命令。`host-integration install --to codex` 吸收全部功能。`sync-entrypoints` 泛化为接受 `--host-id`。

### I8. HostCapabilities 对齐 registry [Phase 1]
HostProvider trait 扩展 8 个 capability 字段，从 registry 自动映射。`HOST_PROJECTION_ADAPTERS` 由 build.rs 自动生成。新宿主接入 ≤ 1 天。

### I9. CodeGraph 默认启用 + caller bug 修复 [Phase 2] ✅ DONE (2026-06-11)
- codegraph feature 加入 default features ✅
- TS/Python/Go 解析器的 caller 归属 bug 修复 ✅（优雅降级，不 panic）
- FTS5 查询参数清洗 ✅（`*`/`"` 转义 + 保留字后跟 `(` 识别）
- `framework_snapshot` 新增 `code_index` 字段 ✅

### I10. 活跃 rust_tools MCP 化 [Phase 2] ⏳ 1/6 完成
6 个活跃工具（citation, financial_data, gh_source_gate, ooxml, pdf, pptx）注册为 MCP 工具。5 个孤立 crate 从 workspace 移除 ✅。
- `pdf_tool_rs`: ✅ MCP 化完成 (mcp-pdf binary, pdf_read + pdf_info)
- 其余 5 个: ⏳ 待提取 lib.rs（每个 ~0.5d）

### I11. Tier 2 能力对齐 [Phase 3]
review gate 磁盘持久化 + closeout hard block metadata + session supervisor MCP 桥接 + 自动 evidence 采集。harness_capabilities 从 2 项扩展到 4 项。

### I12. 去兼容去胶水彻底化 [Phase 0]
- 退役宿主别名全部删除（不保留兼容层）
- legacy per-host env 全部删除或重命名为 canonical 名
- router-rs ~80 条 re-export 全部删除
- 纯转发 wrapper 全部删除
- host-projection 空壳层评估移除

---

## §1 Phase 0: 配置革新 + 去兼容去胶水 [15d]

> 服务于 I1, I2, I3, I4, I12。无前置依赖，可立即开始。

### 1.1 配置用户级迁移 [3d] — I1 ✅ DONE (2026-06-12)

**分析结论**：大部分项目级配置已删除，用户级已就位。核心剩余工作：

**Phase 1（低风险，高收益）**：
1. 删除 `ensure_research_mcp_five_host_surfaces()` 及两个子函数（项目 `.mcp.json` 写入）
2. 修改 `install_projection_tool()` 移除 `research_mcp` 调用和输出

**Phase 2（需宿主验证）**：
3. MCP payload 移除 `--repo-root` 硬编码（依赖 `resolve_repo_root_arg` 自动检测）

**Phase 3（清理）**：
4. 标记 project scope 配置路径为 deprecated

**风险**：宿主 CWD 不是项目根时可能失败（需逐宿主验证）

### 1.2 过期文件清除 [1d] ✅ DONE (2026-06-10)

| 文件 | 处理 |
|------|------|
| `.claude/.framework-projection-desktop.json` | **已删除** ✅ |
| `.claude/mcp.README.md` | **已删除** ✅ |
| `.claude/mcp.json.example` | **已删除** ✅ |
| `.cursor/debug-f45035.log` | **已删除** ✅ |
| `.opencode/node_modules/` | **已删除** ✅ |
| `.opencode/package.json` | **已删除** ✅ |
| `.opencode/package-lock.json` | **已删除** ✅ |
| `router-rs-cli/src/src/main.rs` | **已删除** ✅ |

### 1.3 hook-state 清理策略 [0.5d] ✅ DONE (2026-06-12)

当前：`.claude/hook-state/` 183 个、`.cursor/hook-state/` 371 个、`.codex/hook-state/` 30 个文件（均为 7 天内活跃会话产物）。

**已完成**：>7 天的过期 hook-state 文件已清理（15 个）。
**待实现**（代码改动，可后续推进）：
1. `host-integration install` 新增 `--clean-hook-state` 选项，清除 >7 天的 hook-state 文件
2. hook runtime 中新增自动清理：每次写入时扫描并删除 >7 天的旧文件

### 1.4 Goal state 会话级实现 [3d] — I3 ✅ DONE (2026-06-10)

**已实现**：
1. `operation="start"` 时绑定 `session_id`（显式参数 > 环境变量 > 自动生成）
2. `operation="complete"` 成功后立即删除 GOAL_STATE.json
3. 读取时自动检测陈旧：`session_id` 不匹配则标记 `stale=true`
4. 陈旧 goal 不驱动续跑逻辑
5. 无 `session_id` 的旧版 GOAL_STATE 向后兼容

**测试**：core-state 82 passed（含 5 个新增覆盖）
**修改文件**：`core/core-state/src/state_manager.rs` (+296/-11)、`core/runtime-core/src/hosts/mcp_stdio_harness.rs` (+4)

### 1.5 MCP 服务器全局统一 [2d] — I4 ✅ DONE (2026-06-10)

**目标**：4 个 MCP server 在所有 5 宿主中统一注册。**已实现**。

| MCP Server | claude-code | codex | cursor | antigravity | opencode |
|-----------|:-----------:|:-----:|:------:|:-----------:|:--------:|
| router-rs-framework | ✅ .mcp.json | ✅ config.toml | ✅ mcp.json | ✅ mcp.json | ✅ opencode.json |
| browser-mcp | ✅ .mcp.json | ✅ config.toml | ✅ mcp.json | ✅ mcp.json | ✅ opencode.json |
| mcp-codegraph | ✅ .mcp.json | ✅ config.toml | ✅ mcp.json | ✅ mcp.json | ✅ opencode.json |
| paperplain | ✅ .mcp.json | ✅ config.toml | ✅ mcp.json | ✅ mcp.json | ✅ opencode.json |

**已实现**：
1. RUNTIME_REGISTRY.json `managed_mcp_servers` 声明全部 4 个 server
2. 五宿主 `managed_mcp_server_ids` 统一为 4 个
3. `install_*_projection` 全部写入 4 MCP 条目
4. `remove_*_projection` 全部清理 4 MCP 条目（含 Claude Code `.mcp.json` 清理）
5. `*_projection_status` / `verify_*` 全部验证 4 MCP
6. 文档矩阵同步（9 个文件）

### 1.6 退役宿主别名彻底清除 [2d] — I12 ✅ 扫描完成 (2026-06-10)

**扫描结论**：49+ 处引用中，18 处为向后兼容的别名映射（生产必须），15 处为退役行为验证测试，10 处为退役文档/迁移指南。仅 `router-rs-cli/src/src/main.rs` 为重复文件（§1.2 已删除）。

**决策**：保留别名映射 + deprecation warning（不删除兼容层）。原因：
- 删除会导致已有用户配置（`claude-desktop`/`codex-cli`）直接报错
- `mcp_stdio_harness.rs` 的 `reject_retired_claude_desktop_host()` 主动拒绝机制已覆盖安全面
- 别名映射零运行时开销，仅多一个 match arm

### 1.7 退役环境变量清除 + 重命名 [2d] — I12 ✅ 首轮完成 (2026-06-10)

**已执行**：
- **删除 9 个死常量**：`ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_ENABLED`、`HOOK_STATE_LEGACY_FULL_SWEEP`、`PRE_GOAL_STRICT_DISK`、`CARGO_CHECK_SYNC`、`HOOK_STATE_DIR_SYNC`、`HOOK_STATE_STALE_SWEEP_DAYS`、`HOOK_SILENT`、`HOOK_LEGACY_SUBTRACTED_EVENTS`、`HOOK_STATE_FAIL_OPEN`
- **重命名 5 个**：`ROUTER_RS_CURSOR_*` → `ROUTER_RS_*` canonical（保留旧名 fallback）
- 914 tests passed

**待后续**：core-policy 中 per-host env 的 HostStrings 拆分需与宿主适配层一起重构

### 1.8 API Surface 清理 [1.5d] — I12 ✅ 首轮完成 (2026-06-10)

**已完成**：
- 删除 `goal_prediction` 死模块（纯 re-export，无内部引用）
- 6 条 core-policy re-export 从 `pub use` 降为 `pub(crate) use`（内部仍可用，外部不再暴露）
- 4 个 pub mod 保持 pub（router-rs 外部 crate 需要）
- 1185 tests passed（runtime-core + router-rs）

**待后续**：router-rs ~96 条 item 级 re-export 清理（需要逐步迁移，不在本轮范围）

---

## §2 Phase 1: 物理迁移 + 宿主抽象统一 [25d] ✅ DONE (2026-06-10)

> 服务于 I5, I6, I7, I8。14 agent，9 批并行执行，~25,000 行迁出 runtime-core。

### 2.1 framework_profile.rs 迁移 [0.5d] ✅
1,692 行 → `core/framework-profile`。serde only，12 tests。

### 2.2 route/ → routing-engine [6d] ✅
15 文件 / 6,785 行迁移。7 函数指针 hooks（`hooks.rs`）。routing-engine: 63 tests。
`HookContextProvider` + `RouteCacheProvider` 未使用 trait，改为函数指针注册表模式。

### 2.3 hosts/ + host_integration/ → host-projection [8d] ✅
~23,000 行迁移。82 OnceLock hooks（`hooks.rs` 798L）。5 mirror types。host-projection: 433 tests。
`FrameworkRuntimeDelegate` + `ReviewGateDelegate` 未使用 trait，改为 per-module 函数指针注册。

### 2.4 browser_mcp/ → browser-mcp [3.5d] ✅
4,752 行迁移。`browser_dispatch_hook` 解耦循环依赖。browser-mcp: 8 tests。

### 2.5 codex sync 统一化 [2d] — I7 ✅
`CodexSubcommand::Sync` 删除。`framework sync-entrypoints --host-id codex` 泛化。

### 2.6 HostCapabilities 扩展 + HOST_PROJECTION_ADAPTERS 自动化 [3d] — I8 ✅
6 新字段：batch_execution / cron_execution / ci_runner / non_interactive_entrypoint / external_session_supervisor / rate_limit_auto_resume。codex=true（S档），其余=false。

### 2.7 中间代码去宿主化 [3d] — I6 ✅
`ANEMIC_MCP_HOST_IDS` 删除，改用 `hard_gate_hooks` registry capability。4 文件文档去宿主化。

---

## §3 Phase 2: 工具链激活 [10d]

> 服务于 I9, I10。前置：Phase 1 全量完成 ✅ (2026-06-10)。
> **审计基线**：2026-06-10 全维度扫描（编译/测试/性能/安全/代码质量/依赖）。

### 3.0 CodeGraph caller bug 修复 [2d] — P0 ✅ DONE (2026-06-11)

**问题**：TS/Python/Go 解析器的 `collect_calls` 用 `symbols.first()` 找 caller，**永远归属到文件第一个符号**。只有 `rust.rs` 正确实现了 `enclosing_symbol()`（向上遍历 AST 祖先找最近的 function_item）。

**修复方案**（3 个文件，结构相同的改法）：

| 文件 | 行号 | 当前代码 | 修改为 |
|------|------|---------|--------|
| `parser/typescript.rs` | :77-89 | `collect_calls(node, source, symbols, edges)` 签名去掉 `symbols` 参数；:80 的 `symbols.first()` 改为 `enclosing_symbol_ts(node, source)` | 新增 `enclosing_symbol_ts()` 函数：向上遍历 `function_declaration`/`method_definition`/`class_declaration` 祖先 |
| `parser/python.rs` | :51-75 | 同上，:60 的 `symbols.first()` | 新增 `enclosing_symbol_py()`：向上遍历 `function_definition`/`class_definition` 祖先 |
| `parser/go.rs` | :51-75 | 同上，:60 的 `symbols.first()` | 新增 `enclosing_symbol_go()`：向上遍历 `function_declaration`/`method_declaration` 祖先 |

**FTS5 查询清洗**（同文件 `db/fts_ops.rs:15`）：
- 当前：`trimmed.replace('"', "\"\"")` — 仅转义双引号
- 问题：`*`、`-`、`(`、`"` 等 FTS5 运算符会被注入，导致查询异常或 panic
- 修改：先 strip 所有 FTS5 特殊字符 `+ - * ^ " ( ) :`，再做 quoted prefix 查询
- fallback LIKE 路径（:58）也需要 trim `%` 和 `_` 通配符

**projection feature-gate**：
- `codegraph_mcp` 模块已经 `#[cfg(feature = "codegraph")]` gate（`runtime-core/src/lib.rs:41-42`）✅
- `router_command_dispatch.rs:383-384` 的 `dispatch_codegraph_command` 也已 gate ✅
- **无需额外改动**，gate 已正确

**测试**：
- 为每个解析器新增 `enclosing_symbol` 测试：嵌套函数内调用应归属到内层函数
- 为 FTS5 查询新增注入测试：输入含 `*`、`"` 的查询不 panic
- 目标：codegraph-rs 从 30 → ≥36 tests

### 3.1 CodeGraph 默认启用 [1d] ✅ DONE (2026-06-11)

**当前状态**：
- `runtime-core/Cargo.toml:55` — `codegraph = ["dep:codegraph-rs"]` 是 optional
- `runtime-core/Cargo.toml:40-46` — default features 不含 `codegraph`
- `router-rs/Cargo.toml:47` — `codegraph = ["runtime-core/codegraph"]` 也是 optional
- `router-rs/Cargo.toml:35-41` — default features 不含 `codegraph`

**修改**：
1. `runtime-core/Cargo.toml` default features 加入 `"codegraph"`
2. `router-rs/Cargo.toml` default features 加入 `"codegraph"`
3. 确认 `cargo build --release` 自动包含 mcp-codegraph 二进制
4. 确认 `host-integration install` 的 MCP payload 正确引用 mcp-codegraph

**风险**：
- codegraph-rs 依赖 tree-sitter（~2MB C 库），会增加编译时间（预估 +15-20s clean build）
- 首次 `cargo build` 的 tree-sitter 编译耗时较长，CI 需要缓存

**验收**：`cargo build --release --workspace 2>&1 | grep mcp-codegraph` 有产物

### 3.2 framework_snapshot + codegraph 结合 [1d] ✅ DONE (2026-06-11)

**当前 `framework_snapshot`**（`mcp_stdio_harness.rs:897`）：
- 返回框架运行时快照（registered_tasks, host_info, hooks 等）
- 有 30s TTL 缓存
- **无 codegraph 信息**

**修改**：
1. `tool_framework_snapshot()` 中，当 `codegraph` feature 启用时，调用 `codegraph_rs::db::index_ops::get_index_stats()` 获取索引概况
2. 新增 `code_index` 字段：
   ```json
   {
     "code_index": {
       "enabled": true,
       "db_path": "~/.cache/skill-framework/codegraph.db",
       "total_nodes": 1234,
       "total_edges": 5678,
       "languages": ["rust", "typescript", "python", "go"],
       "last_sync_ns": 1718000000000000000,
       "stale": false
     }
   }
   ```
3. `#[cfg(feature = "codegraph")]` gate，无 codegraph 时返回 `"code_index": {"enabled": false}`

**测试**：
- mock codegraph index 验证 snapshot 输出结构
- 无 feature 时返回 `enabled: false`

### 3.3 活跃 rust_tools MCP 化 [3d] ✅ DONE (2026-06-12)

**完成状态**：
- 6 个活跃工具全部 MCP 化，均有 lib.rs + mcp/mod.rs + mcp_main.rs binary
- 每个工具支持 JSON-RPC stdio MCP server 模式
- `RUNTIME_REGISTRY.json` 的 `managed_mcp_servers` 已更新
- 五宿主 MCP 配置已更新

| 工具 | 行数 | MCP 二进制 | MCP 工具 | 状态 |
|------|------|-----------|---------|------|
| `pdf_tool_rs` | 3.9K | `mcp-pdf` | `pdf_read` + `pdf_info` | ✅ DONE |
| `citation_tool_rs` | 43.9K | `mcp-citation` | `citation_audit` + `citation_lint` | ✅ DONE |
| `financial_data_rs` | 50.2K | `mcp-financial-data` | `financial_data` | ✅ DONE |
| `gh_source_gate_rs` | 48.4K | `mcp-gh-source-gate` | `gh_source_gate` | ✅ DONE |
| `ooxml_parser_rs` | 76.7K | `mcp-ooxml` | `ooxml_parse` | ✅ DONE |
| `pptx_tool_rs` | 171K | `mcp-pptx` | `pptx_parse` | ✅ DONE |

### 3.4 孤立 crate 清理 [1d] ✅ DONE (2026-06-11)

**待清理 crate**（5 个，需确认无外部引用）：
- `image_gen_rs` — 无 MCP，无 lib API，无下游依赖
- `image_process_rs` — 同上
- `ref_corpus_tool_rs` — 同上
- `pubmed_tool_rs` — 同上
- `screenshot_rs` — 同上（依赖 xcap，编译耗时最长 13.5s）

**步骤**：
1. `cargo tree --workspace -i -p <crate>` 确认无反向依赖
2. 从 workspace members 移除
3. 移动到 `archive/rust_tools/` 或直接删除
4. `cargo check --workspace` 确认无编译影响

**预期收益**：clean build 时间减少 ~20s（screenshot_rs 的 xcap 编译链）

### 3.5 pdf/pptx 作为 agent 读取能力 [2d] ✅ DONE (2026-06-12)

**已完成**：
1. `pdf_read` MCP 工具：接收路径 + 页码范围（`pages` 参数），返回提取文本
2. `pptx_parse` MCP 工具：接收路径 + slide 范围（`slides` 参数），返回文本 + 备注
3. 大文件分页策略：单次最大 50 页/slides，超出返回 `next_page`/`next_slide` 分页 token
4. 页码/slide 范围语法：`1-5`, `3`, `1,3,7-10`, `all`（1-indexed）
5. agent 可通过 `mcp__mcp-pdf__pdf_read` 和 `mcp__mcp-pptx__pptx_parse` 调用

---

## §4 Phase 3: Tier 2 能力对齐 [13d]

> 服务于 I11。前置：Phase 2 完成。

### 4.1 Review gate 磁盘持久化 [3d] ✅ DONE (2026-06-12)

**当前状态**：review gate 结果仅存于内存（MCP 响应），进程退出即丢失。

**实施方案**：
1. `artifacts/current/<task_id>/review_gate.json` 持久化：
   ```json
   {
     "task_id": "...",
     "rounds": [
       {"round": 1, "findings": [...], "verdict": "advisory", "ts": "..."}
     ],
     "cleared": false,
     "cleared_at": null
   }
   ```
2. `closeout_gate` 读取磁盘状态判断 review 是否完成
3. `my-light` profile 下仍然 advisory（不阻断）
4. 清理策略：task complete 后 review_gate.json 归档到 `artifacts/archive/`

### 4.2 Closeout hard block metadata 强化 [3d] ✅ DONE (2026-06-12)

**当前状态**：closeout 返回缺失项清单，但 metadata 结构扁平。

**强化方向**：
1. closeout record 新增 `blockers` 分级：`hard`（必须修复）vs `soft`（advisory）
2. `closeout_gate` 响应新增 `can_proceed: bool` 字段
3. 非 `my-light` profile 时，`hard` blocker 阻断 `complete`
4. `my-light` 下全部降级为 soft（当前行为不变）

### 4.3 Session supervisor MCP 桥接 [4d] ✅ DONE (2026-06-12)

**当前状态**：session supervisor 仅 codex 宿主通过 `codex_driver` 原生支持，其余宿主 unsupported。

**实施方案**：
1. 定义 `SessionSupervisorBridge` trait：
   ```rust
   trait SessionSupervisorBridge {
       fn launch_worker(&self, prompt: &str, cwd: &str) -> Result<WorkerId>;
       fn inspect_worker(&self, id: &WorkerId) -> Result<WorkerStatus>;
       fn terminate_worker(&self, id: &WorkerId) -> Result<()>;
       fn list_workers(&self) -> Result<Vec<WorkerSummary>>;
   }
   ```
2. `browser-mcp` 的 session_supervisor MCP 工具作为 bridge 实现
3. claude-code 宿主：通过 `SubagentStart`/`SubagentStop` hooks 桥接
4. cursor 宿主：通过 tmux session 管理桥接（如可用）
5. `HostCapabilities` 的 `session_supervisor` 从 `unsupported` 升级为 `mcp_bridge`

### 4.4 自动 evidence 采集 [3d] ✅ DONE (2026-06-12)

**当前状态**：evidence 由 agent 手动调用 `record_evidence`，经常遗忘。

**自动化策略**：
1. `PostToolUse` hook 中，当工具为 `Bash` 且命令为验证类（`cargo test`、`cargo check`、`npm test` 等）时，自动调用 `record_evidence`
2. 命令分类规则：
   - `cargo test*`、`cargo check*`、`cargo build*` → `exit_code` + 测试摘要
   - `git diff*`、`git log*` → 变更统计
   - 其他 Bash 命令 → 仅记录 exit_code
3. 防重复：同一 session 同一命令不重复记录
4. `harness_capabilities` 新增 `auto_evidence` 字段

---

## §5 Phase 4: 测试与文档 [14d]

> 前置：Phase 0-3 基本完成。

### 5.1 测试补齐 [6d]

**当前覆盖情况**（2026-06-10 扫描）：

| crate | tests | lines | 密度 | 评估 |
|-------|-------|-------|------|------|
| runtime-core | 810 | 46K | 17.6/K | ✅ 充足 |
| host-projection | 433 | 32K | 13.5/K | ✅ 充足 |
| router-rs | 271 | 558 (+6K tests) | — | ✅ 充足 |
| routing-engine | 63 | 8K | 7.9/K | ⚠️ 一般 |
| codegraph-rs | 30 | 2.5K | 12/K | ✅ 但 caller bug 测试缺失 |
| core-state | 82 | 6.9K | 11.9/K | ✅ |
| evolution-rs | 13 | 1.8K | 7.2/K | ⚠️ 偏低 |
| autoresearch-rs | **2** | **5.4K** | **0.4/K** | 🔴 严重不足 |
| browser-mcp | 8 | 4.8K | 1.7/K | ⚠️ 偏低 |
| framework-profile | 12 | 1.7K | 7.1/K | ⚠️ 一般 |

**优先补齐**：
1. `autoresearch-rs`：5.4K 行仅 2 个测试，需 ≥20 个（parser/scraper/output 各模块）
2. `browser-mcp`：8 个测试覆盖 4.8K 行，MCP dispatch 路径需补测试
3. `evolution-rs`：进化引擎核心逻辑需更多边界测试

### 5.2 CI feature gate 矩阵 [2d] ✅ DONE (2026-06-12)

**当前**：无 CI，无 feature matrix 测试。

**方案**：
1. GitHub Actions workflow：`cargo test --workspace`（default features）
2. 额外 matrix：`--features codegraph`、`--no-default-features`
3. 每个 host feature 单独一行：`--features host-cursor`、`--features host-claude-code` 等
4. Lint：`cargo clippy --workspace -- -D warnings`
5. Format：`cargo fmt --check`

### 5.3 spec.md 重写 [3d] ✅ DONE (2026-06-12)

**当前问题**：
- spec.md 仍基于 v5 crate 拓扑（runtime-core 包含 hosts/route/browser_mcp）
- evolution-rs 和 rust_tools 数据过时

**重写范围**：
1. crate 拓扑图更新为 v6 DAG（见 PROGRESS.md）
2. hooks 架构更新（fn ptr 注册表 + OnceLock，不再是 trait）
3. 宿主能力矩阵更新（三档 S/A/B）
4. rust_tools 清单更新（11 → 6 MCP + 5 archived）

### 5.4 host-scaffold CLI 工具 [3d] ✅ DONE (2026-06-12)

**目标**：`router-rs scaffold --host-id <name>` 自动生成新宿主接入所需的全部文件。

**生成物**：
1. `hosts/<host>_provider.rs` — HostProvider 实现骨架
2. `AGENTS_<HOST>.md` — 上下文文件模板
3. `docs/hosts/<host>.md` — 文档模板
4. Cargo.toml feature 条目
5. RUNTIME_REGISTRY.json 条目（需手动补充 capabilities）

---

## §6 工时总览

| Phase | 工时 | 可并行 | 服务意图 |
|-------|------|--------|---------|
| §1 Phase 0: 配置革新 + 去兼容 | 15d | — | I1-I4, I12 |
| §2 Phase 1: 迁移 + 宿主统一 | 25d | 部分与 §1 并行 | I5-I8 |
| §3 Phase 2: 工具链激活 | 10d | §3.0-3.2 串行，§3.3-3.5 可并行 | I9-I10 |
| §4 Phase 3: Tier 2 对齐 | 13d | §4.1+4.2 可并行，§4.3 独立 | I11 |
| §5 Phase 4: 测试文档 | 14d | §5.1+5.2 可并行 | 全部 |
| **总计** | **~77d** | | |

---

## §7 意图覆盖矩阵

| 意图 | Phase 0 | Phase 1 | Phase 2 | Phase 3 | Phase 4 |
|------|:-------:|:-------:|:-------:|:-------:|:-------:|
| I1 配置用户级 | §1.1 | | | | |
| I2 清除过期入口 | §1.2,1.3 | | | | |
| I3 Goal 会话级 | §1.4 | | | | |
| I4 MCP 全局统一 | §1.5 | | | | |
| I5 runtime-core 瘦身 | | §2.1-2.4 | | | |
| I6 零宿主名硬编码 | §1.6,1.7 | §2.7 | | | |
| I7 codex sync 统一 | | §2.5 | | | |
| I8 HostCapabilities | | §2.6 | | | |
| I9 CodeGraph 激活 | | | §3.0-3.2 | | |
| I10 rust_tools MCP | | | §3.3-3.5 | | |
| I11 Tier 2 对齐 | | | | §4.1-4.4 | |
| I12 去兼容去胶水 | §1.6-1.8 | | | | |

**验证**：12 条意图全部有对应实现，无遗漏。

---

## §8 全维度工具链审计报告 (2026-06-10)

> 覆盖维度：编译、测试、性能、代码质量、依赖、安全、配置、磁盘、Git 健康。

### A. 编译

| 指标 | 数据 |
|------|------|
| workspace 编译 | **0 error**, ~100 warnings |
| clean build (dev) | **55.7s**（287s CPU，573% 并行度） |
| incremental check | **0.17s** |
| release build | 0 error, 140 warnings |

**Warning 分类**（按数量排序）：

| 类别 | 数量 | 说明 |
|------|------|------|
| deprecated struct/field 使用 | ~40 | evolution journal 旧格式，需迁移 |
| unused import | 5 | 可直接删除 |
| unused variable | 2 | `skip_review_output_lint` |
| unexpected cfg value `mimalloc` | 2 | router-rs-cli，需加 feature 或删 cfg |
| mutable 不必要 | 3 | 移除 `mut` |

**编译性能瓶颈**：`screenshot_rs` 依赖 xcap（macOS 原生框架绑定），编译 13.5s；`image_process_rs` 依赖 image 0.24.9，编译 5.3s。清理后（§3.4）预计节省 ~20s。

### B. 测试

| 指标 | 数据 |
|------|------|
| 全量测试 | **1,597 passed, 1 failed, 27 ignored** |
| 测试执行时间 | ~7s（router-rs 最慢 7.1s，其余 <1s） |

**🔴 1 个失败测试**：

| 测试 | crate | 原因 |
|------|-------|------|
| `framework_host_targets::tests::host_provider_mod_declarations_align_with_registry` | framework-kernel | Phase 1 迁移后 `hosts/mod.rs` 变为 `pub use host_projection::hosts::*` re-export shim，但测试仍期望显式 `#[cfg(feature = "...")] mod xxx_provider;` 声明 |

**修复方案**：更新 `validate_host_provider_mod_declarations()` 函数，当 hosts/mod.rs 为纯 re-export 时检查 `host-projection` 的 hosts/mod.rs（或跳过本检查，因为 host-projection 已有独立测试覆盖）。

**测试覆盖热区**：

| crate | tests | 行数 | 密度 (tests/K) | 评估 |
|-------|-------|------|----------------|------|
| runtime-core | 810 | 46,162 | 17.6 | ✅ |
| host-projection | 433 | 32,217 | 13.5 | ✅ |
| router-rs | 271 | 558 | — | ✅（测试在 tests/ 目录） |
| core-state | 82 | 6,896 | 11.9 | ✅ |
| routing-engine | 63 | 8,062 | 7.9 | ⚠️ |
| codegraph-rs | 30 | 2,553 | 11.8 | ✅（缺 caller 测试） |
| evolution-rs | 13 | 1,849 | 7.0 | ⚠️ |
| framework-profile | 12 | 1,692 | 7.1 | ⚠️ |
| browser-mcp | 8 | 4,783 | 1.7 | 🔴 |
| autoresearch-rs | **2** | **5,404** | **0.4** | 🔴 |

### C. 性能

#### C.1 运行时性能

| 指标 | runtime-core | host-projection | routing-engine |
|------|-------------|-----------------|----------------|
| `.clone()` 次数 | 499 | 96 | 66 |
| 非测试代码行 | 42,604 | 24,895 | 8,062 |
| clone 密度 | **1.17%** | **0.39%** | **0.82%** |

**评估**：runtime-core 的 clone 密度偏高（499 次/42K 行），主要在 MCP 请求/响应序列化路径。host-projection 经过 Phase 1 重构后密度较低。

#### C.2 内存分配

| 指标 | runtime-core | host-projection | routing-engine |
|------|-------------|-----------------|----------------|
| format!/to_string/String::from | 2,692 | 890 | 305 |
| 分配密度 | 6.3% | 3.6% | 3.8% |

**评估**：runtime-core 的字符串分配密度偏高，主要在 MCP JSON 序列化和日志路径。

#### C.3 并发模式

| 模式 | 位置 | 说明 |
|------|------|------|
| `std::thread::spawn` | 14 处 | 分布在 stdio_transport(4)、codex_hooks(3)、kernel_bootstrap(1)、codegraph-sync(1)、telemetry(2)、routing-watch(1)、autoresearch(1) |
| `OnceLock`/`LazyLock`/`static` | **188 处** | 大量全局静态（regex 缓存、hook 注册表等） |
| `Box<dyn>` trait objects | 11 处 | vtable 分发，可接受 |
| `tokio::spawn` | 0 处 | 纯同步架构，无 async runtime |

**评估**：纯 `std::thread` + 同步 I/O 架构。188 个全局静态偏多但大多为 regex OnceLock（合理），hook 注册表的 82 个 OnceLock 是 Phase 1 设计产物。

#### C.4 阻塞 I/O

生产代码中的同步文件 I/O：

| 位置 | 模式 | 风险 |
|------|------|------|
| `eval_route.rs:66` | `std::fs::read_to_string` | 低（一次性读取配置） |
| `session_call_tracker.rs:207` | `std::fs::read_to_string` | 低（读取 tracker 状态） |
| `paper_adversarial_hook.rs` | 多处 `std::fs::write` | 低（hook 触发时写入） |

**评估**：均为低频操作，无热路径阻塞风险。stdio_transport 使用专用线程池处理请求，不在 async 上下文中。

### D. 代码质量

#### D.1 unwrap/expect 密度

| crate | unwrap | expect | panic/todo | 行数 | unwrap 密度 |
|-------|--------|--------|------------|------|-------------|
| codegraph-rs | **120** | 1 | 1 | 2,553 | **4.7%** 🔴 |
| core-state | 131 | 182 | 0 | 6,896 | 1.9% |
| host-projection | 341 | 85 | 7 | 32,217 | 1.1% |
| runtime-core | 212 | 260 | 7 | 46,162 | 0.5% |
| core-policy | 31 | 37 | 1 | 4,154 | 0.7% |
| routing-engine | 2 | 16 | 3 | 8,062 | 0.02% ✅ |
| browser-mcp | 0 | 0 | 0 | 4,783 | 0% ✅ |

**评估**：codegraph-rs 的 unwrap 密度 4.7% 严重偏高（120 unwrap / 2553 行），主要是 parser 和 DB 操作。routing-engine 和 browser-mcp 最优。

#### D.2 大文件（>1500 行）

| 文件 | 行数 | 说明 |
|------|------|------|
| `host-projection/.../cursor_hooks/tests.rs` | 7,282 | 测试文件，可接受 |
| `router-rs/tests/main_tests.rs` | 6,394 | 集成测试 |
| `autoresearch-rs/src/main.rs` | 5,404 | 🔴 单文件 5K+，需拆分 |
| `host-projection/.../codex_hooks/mod.rs` | 5,007 | 🔴 生产代码 5K+，需拆分 |
| `*/host_integration/projection.rs` | 4,039 × 2 | ⚠️ 两个 crate 中的同名文件内容接近（仅 crate 路径不同） |
| `core-state/state_manager.rs` | 2,710 | 边界可接受 |
| `host-projection/.../claude_code_hooks.rs` | 2,489 | 边界可接受 |

**重点关注**：
- `autoresearch-rs/src/main.rs`（5.4K 行）应拆分为 lib + 多模块
- `codex_hooks/mod.rs`（5K 行）应继续拆分
- **`projection.rs` 重复**：`runtime-core` 和 `host-projection` 中各有一份 4039 行的 `host_integration/projection.rs`，内容仅 crate 路径不同（`crate::` vs `framework_kernel::`），这是 Phase 1 迁移的遗留，runtime-core 那份应删除

#### D.3 TODO/FIXME 标记

仅 2 处生产代码 TODO：
- `runtime-core/src/rfv_loop.rs:27` — `TODO(RFV-external-research): integrate strict validation path`
- `runtime-core/src/closeout_enforcement.rs:333` — `TODO(R9-tech-debt): task-scoped depth alignment`

#### D.4 unsafe 代码

| 位置 | 用途 | 风险 |
|------|------|------|
| `claude_code_hooks.rs:897-929` | `libc::flock` 文件锁 | 低（标准 POSIX 操作） |
| `codex_hooks/state.rs:125,258,337` | `libc::flock` 文件锁 | 低 |
| `cursor_hooks/tests.rs` (6 处) | `libc::flock/pid` 测试 | 低（仅测试） |
| `handlers_session.inc.rs:654-697` | `libc::getpgid/getpgrp/getppid` | 低（进程信息查询） |

**评估**：所有 unsafe 均为 libc POSIX 调用，无手动内存管理，无 UB 风险。

### E. 依赖健康

#### E.1 依赖重复

| 包 | 版本冲突 | 来源 |
|----|---------|------|
| `bitflags` | 1.3.2 vs 2.11.1 | image 0.24.9 引入 v1（通过 png 0.17.16） |

**根因**：`image_process_rs` 依赖的 `image = "0.24"` 已过时（最新 0.25.x 使用 bitflags v2）。清理 screenshot_rs/image_process_rs（§3.4）后自动消除。

#### E.2 workspace 成员

| 类型 | 数量 | 说明 |
|------|------|------|
| core crate | 13 | 含 framework-profile |
| rust_tools crate | 11 | 含即将清理的 5 个 |
| excluded | 1 | codex-aggregator |
| **总计** | **25** | |

#### E.3 版本新鲜度

核心依赖均使用较新版本（serde 1.0.228、tokio 1.44、reqwest 0.12.28）。`serde_yml` 锁定 `=0.0.12` 精确版本（可能有 breaking change 风险，需定期更新）。

### F. 安全

| 维度 | 状态 |
|------|------|
| 凭证处理 | ✅ `aggregator_api_key` 使用前非空校验，通过 bearer auth 传递 |
| 路径安全 | ✅ `path_guard::reject_unsafe_path` 覆盖，拒绝 traversal |
| FTS5 注入 | ⚠️ 仅转义双引号，未处理其他运算符（§3.0 修复） |
| 文件锁 | ✅ 使用 `flock(LOCK_EX)` 正确实现 |
| hook 策略 | ✅ core-policy 有 186 条正则规则覆盖危险命令 |

### G. 配置与 hooks

| 维度 | 状态 |
|------|------|
| Claude Code hooks | ✅ 7 个事件全部配置（PreToolUse/PostToolUse/Stop/UserPromptSubmit/SessionStart/SubagentStart/SubagentStop） |
| hook 模式 | ✅ advisory（非 hard block） |
| settings.local.json | ✅ 无本地覆盖 |
| MCP 配置 | ✅ 4 个 server 五宿主统一（Phase 0 §1.5 完成） |
| hook-state 文件 | ⚠️ 183+371+30 个活跃文件，>7 天已清理 |

### H. 磁盘与 Git

| 指标 | 数据 |
|------|------|
| workspace 大小（不含 target/.git） | **6.2 GB** |
| target/ 大小 | **3.9 GB**（clean build 后） |
| 本地分支 | 1（main） |
| worktree | 0 |
| stale worktree-agent branches | 0 ✅ |

**评估**：target/ 占用 3.9GB 正常（LTO release + 大量依赖）。workspace 6.2GB 偏大（含历史 artifacts），可定期清理 `artifacts/archive/`。

---

## §9 附录

### 9.A 宿主能力矩阵（RUNTIME_REGISTRY.json 真源）

| 能力 | claude-code | codex | cursor | antigravity | opencode |
|------|:-----------:|:-----:|:------:|:-----------:|:--------:|
| capabilities 总数 | 6 | **11** | 6 | 5 | 5 |
| harness_capabilities | 4 项 | 4 项 | 4 项 | 2 项 | 2 项 |
| session_supervisor | unsupported | **codex_driver** | unsupported | unsupported | unsupported |
| 传输 | claude-hooks | native-codex | cursor-agent | mcp-stdio | opencode-cli |

**三档**：S 档 codex (CI/Batch) / A 档 claude-code, cursor (Full Hook) / B 档 antigravity, opencode (MCP)

### 9.B Codex 维护面（仅 2 端）
1. RUNTIME_REGISTRY.json — 声明
2. codex_provider.rs — 实现
其余全部自动派生。

### 9.C 新宿主接入流程 (v6 后)
1. RUNTIME_REGISTRY.json — 声明 host_targets + capabilities
2. hosts/<host>_provider.rs — 实现 HostProvider
3. Cargo.toml — 添加 feature
4. docs/hosts/<host>.md — 文档
5. AGENTS_<HOST>.md — 上下文文件
6. host_projection_narrative.json — 叙事
总计：MCP 宿主 ~4h，原生 hook 宿主 ~8h。≤ 1 天。
