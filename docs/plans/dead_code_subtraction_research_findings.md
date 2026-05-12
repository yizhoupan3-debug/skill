# 死代码与冗余设计调研 Findings（减法 / 第一性原理）

**路径说明（2026-05）**：下文「`cursor_hooks.rs`」均指重构前的单文件形态；现为 [`scripts/router-rs/src/cursor_hooks/`](../../scripts/router-rs/src/cursor_hooks/mod.rs) 目录（`mod.rs` + `frag_*.rs` + `dispatch.rs`）。

**日期**：2026-05-11  
**范围**：仓库内 `router-rs`、框架 JSON、宿主集成、`tools/browser-mcp`；对照 `docs/history/` 归档清单。  
**非目标**：本文件不执行删除 PR；结论以静态证据为准，动态覆盖率未测。

---

## 1. 减法锚点（Steady-state 主链 + Non-goals）

### 1.1 必须保留的控制面（第一性原理）

依据仓库根 [AGENTS.md](../AGENTS.md) 权威分层与 [docs/harness_architecture.md](../harness_architecture.md)：

| 层级 | 真源 / 行为 | 用户价值 |
|------|-------------|----------|
| 路由 | `skills/SKILL_ROUTING_RUNTIME.json` → 命中 `skill_path` | 最小必要 skill 面 |
| 框架命令 | `configs/framework/RUNTIME_REGISTRY.json` | 显式入口与宿主投影 |
| Hook 行为 | 各宿主 `hooks.json` + `router-rs`（`cursor_hooks` / `codex_hooks` / `claude_hooks`） | 注入、拦截、门控 |
| 程序化 schema | `configs/framework/*.json` + 对应 Rust 校验 | closeout / 契约一致 |
| 连续性 | `artifacts/current/` 与 harness 五层模型 | 跨会话可冷启动 |

**Verify**：`rg -n "权威分层|Skill Routing|steady-state"` AGENTS.md；`rg -n "L1|L2|continuity"` docs/harness_architecture.md | head`

### 1.2 Explicit non-goals（本轮调研排除）

- 不将 `docs/history/*` 的已完成 checklist 当作当前运行契约（见 `docs/README.md` 文档索引「历史迁移」边界）。
- 不对 `skills/` 下全文 skill 正文做人工通读；仅 JSON 真源与消费者引用。
- 不以「未跑到的集成测试」反证生产路径必死。

---

## 2. Rust：`#[allow(dead_code)]` 与重复实现

### 2.1 汇总表（`scripts/router-rs/src`）

| 文件 | 行级线索 | 类别 | 结论 |
|------|-----------|------|------|
| `framework_runtime/mod.rs` | 528–592, 700–704, 800–900+, 1394–1766 等大量 `allow(dead_code)` | **duplicate-island** | 与 `runtime_view.rs`、`session_artifacts.rs` 中**同名能力重复**；`load_framework_runtime_view` 已委托 `runtime_view::load_framework_runtime_view`（`mod.rs:377-382`），本模块内死区函数**无外部调用**。 |
| `cursor_hooks/`（重构前单行号，仅归档） | 3055–3079（历史） `read_json_strict`、`truncate_utf8_chars` | **duplicate** | 仅定义；正文使用 `truncate_utf8_chars_local`（3119/3138）。`read_json_strict` **零引用**。 |
| `closeout_enforcement.rs` | 25/41/60 整 struct、`CloseoutEvidenceContext` 327 | **serde / reserved** | 字段由 serde 与 `evaluate_closeout_record*` 路径消费；`evidence_rows_non_empty` 在 `framework_runtime/mod.rs`、`task_state` 等有赋值。`allow(dead_code)` 多为消除「仅反序列化使用」警告。 |
| `eval_route.rs` | 15/27/34 | **fixture** | 注释已说明 fixture 元数据。 |
| `cli/runtime_ops.inc` | 1749 `copy_text_to_clipboard` | **integration-test** | 注释写明保留给集成测试；`main_tests.rs:521` 调用。 |

**Verify**：`rg "#\\[allow\\(dead_code\\)\\]" scripts/router-rs/src`；`cargo test --manifest-path scripts/router-rs/Cargo.toml --quiet` → **509 passed**（2026-05-11）。

### 2.2 P0 结论（高置信死代码 / 可删候选）

1. **`framework_runtime/mod.rs` 大段重复逻辑**：`write_session_artifact_set`、`authoritative_route`、`normalize_supervisor_state`、`read_json_if_exists`（返回 `Value` 的包装）等与子模块 live 实现并存；属于典型合并残留，**删除前**需单次 PR 内 `rustfmt` + 全量 `cargo test` + 确认无 `pub use` 依赖这些私有符号。
2. **`cursor_hooks/`（原 `cursor_hooks.rs`）**：`read_json_strict`、`truncate_utf8_chars` 为未使用副本，可与 `hook_common` 或单一 truncate 工具合并删除。

### 2.3 P1 / P2

- **P1**：`closeout_enforcement` 上 struct 级 `allow(dead_code)` —— 可评估改为 `#[allow(dead_code)]` 仅标字段或 `#[serde(default)]` 策略，减少「全结构豁免」面积。
- **P2**：`eval_route` fixture 字段保持现状即可。

---

## 3. 配置与控制面：双真源与派生报告

### 3.1 `SKILL_LOADOUTS` / `SKILL_TIERS` / `SHADOW`

| 工件 | 机器可读角色 | 消费者（摘录） |
|------|--------------|----------------|
| `configs/framework/FRAMEWORK_SURFACE_POLICY.json` | `source_of_truth: true`，声明 `derived_reports` / `deprecated_or_foldable_reports` | `tests/policy_contracts.rs::framework_surface_policy_is_the_activation_source_of_truth` |
| `skills/SKILL_TIERS.json` | `source_of_truth: false`，`report_status: generated_debug_report` | 同上测试；`scripts/skill-compiler-rs` 生成；`host_integration` / `claude_hooks` 列安装清单 |
| `skills/SKILL_LOADOUTS.json` | `foldable_generated_report` | 同上；compiler 生成 |
| `skills/SKILL_SHADOW_MAP.json` | 编译诊断 / shadow | `host_integration`、`skill-compiler-rs`、`evolution-audit.yml` |

**结论**：运行时**热路由**仍以 `SKILL_ROUTING_RUNTIME.json` 为主；tiers/loadouts 已由策略文件明确为**派生/可折叠报告**（`policy_contracts` 596–631 行断言），与历史清单「旁路文档」一致但**已有正式契约**，**不宜无迁移直接删文件**。

**Verify**：`rg -n "SKILL_LOADOUTS|SKILL_TIERS|SHADOW" skills configs tests`；`cargo test --test policy_contracts --quiet` → **72 passed**（2026-05-11）。

### 3.2 P1 冗余设计（非死文件）

- **三份叙事**：`FRAMEWORK_SURFACE_POLICY` + 两份生成 JSON 仍重复表达激活策略；长期减法方向是「compiler 只产出一份合并报告或仅 CI 消费」——需改 `policy_contracts` 与 `skill-compiler-rs` 同步，属设计债非 P0 死代码。

---

## 4. 宿主 Hook 与 Browser MCP 胶水

### 4.1 当前架构

- **Live stdio**：`router-rs` 内 `browser_mcp.rs` + `cli/dispatch_body.txt` 的 `BrowserCommand::McpStdio`；README 声明 **`start_browser_mcp.sh` 已移除**。
- **Verify**：`Glob tools/browser-mcp/**/start_browser*.sh` → **0 files**；`rg -n "browser-mcp|stdio" tools/browser-mcp scripts/router-rs/src` 见 Rust 主路径与 TS `index.ts`/`runtime.ts`。

### 4.2 仍开放冗余（对照历史 §1 / §6）

- **`tools/browser-mcp/src/runtime.ts`** 仍维护 RouterRs stdio client pool（历史 checklist 已指出）；与 Rust-first 路径**并行**，属 **P1「双进程控制叙事」** —— 是否保留仅作 dev/replay 应在 README 与 CI 矩阵中写死，避免默认双轨。
- **`host_integration.rs`** 仍引用 `tools/browser-mcp/node_modules` 路径（3569 行附近）用于安装/校验场景，与上条耦合。

### 4.3 `docs/rust_contracts.md`

正文 **无** `browser-mcp` 字面匹配；浏览器 MCP 契约主要落在 Rust 源码与 `host_adapter_contract.md`。文档索引可补充交叉链接（**P2 文档**）。

---

## 5. 与 `docs/history/` 清单的 Diff（各 ≥1 条）

### 5.1 已关闭（有当前树证据）

| 历史项 | 证据 |
|--------|------|
| `codex_adapter` / `host_adapter_payload` 命名债 | `framework_profile.rs` 现用 `codex_profile` / `codex_host_payload`；`rg codex_adapter framework_profile.rs` → 无命中 |
| `python_may_continue_to_author` | `rg python_may_continue framework_profile.rs` → 无命中 |
| `start_browser_mcp.sh` 三层 wrapper | `tools/browser-mcp` 下无 `start_browser_mcp.sh`；README 指向 `router-rs … mcp-stdio` |
| `main.rs` 巨型 `Cli` 单文件调度 | `main.rs` 仅 `cli::Cli::parse` + `cli::run`（69–72 行）；子命令已下沉 `cli` 模块 |

### 5.2 仍开放

| 历史项 | 证据 |
|--------|------|
| `runtime_storage` 多 backend（fs/sqlite/memory） | 未在本次细读 `runtime_storage.rs` 全路径；历史项保持 **待专盘审计** |
| TS `runtime.ts` RouterRs 进程池 | `tools/browser-mcp/src/runtime.ts` 仍存在 client pool 逻辑 |
| surface + tiers + loadouts 三份表达 | `FRAMEWORK_SURFACE_POLICY.json` + 生成物仍在；测试强制对齐 |

### 5.3 新发现（相对历史清单未强调）

- **`framework_runtime/mod.rs` 与子模块的大块重复实现**（§2）：比「CLI 膨胀」更直接的 **可复制删除** 死区。

---

## 6. 分级汇总

| 等级 | 项 | 建议动作 |
|------|-----|----------|
| **P0** | `framework_runtime/mod.rs` 死区函数岛 | 单 PR 删除重复私有函数，委托唯一 `runtime_view` / `session_artifacts` |
| **P0** | `cursor_hooks/` 未用 `read_json_strict` / `truncate_utf8_chars` | 删除或合并到公共 util |
| **P1** | TS browser-mcp `runtime.ts` 与 Rust stdio 双轨 | 文档声明默认路径 + 可选降级 CI；长期收敛 replay 只保留一侧 |
| **P1** | tiers/loadouts 与 surface policy 三份结构 | compiler/测试驱动的合并设计，非静默删文件 |
| **P2** | `closeout_enforcement` struct 级 dead_code allow | 收紧到字段级或测试模块 |
| **P2** | `rust_contracts.md` 与 browser MCP 交叉索引 | 补链接段落 |

---

## 7. 建议下一实现波次（≤5 条）

1. **P0（done）**：已删除 `framework_runtime/mod.rs` 中与 `runtime_view.rs` / `session_artifacts.rs` 重复的 `#[allow(dead_code)]` 私有函数岛及仅被其使用的 `write_text_if_changed` / `write_json_if_changed` 包装层；保留 `normalize_task_registry_rows`、`read_json_strict`、`is_terminal`、`normalize_evidence_index` 等仍被子模块或本模块热路径使用的符号。
2. **P0（done）**：已删除 `cursor_hooks/` 目录内（原单体文件迁移后）未引用的 `read_json_strict` 与 `truncate_utf8_chars`。
3. **P1**：为 `tools/browser-mcp` 增加「默认宿主仅 Rust stdio」的 README/CI 声明，并列出 TS pool 的允许场景（dev-only 或 replay-only）。
4. **P1**：若要做 loadouts/tiers 减法，先改 `skill-compiler-rs` + `tests/policy_contracts.rs` 的契约，再删生成物。
5. **P2**：`runtime_storage.rs` 专盘审计（legacy key / sqlite 默认策略）与历史 §3 对齐。

---

## 8. 验证命令速查

```bash
rg "#\\[allow\\(dead_code\\)\\]" scripts/router-rs/src
cargo test --manifest-path scripts/router-rs/Cargo.toml --quiet
cargo test --test policy_contracts --quiet
rg -n "SKILL_LOADOUTS|SKILL_TIERS|SHADOW" skills configs tests
rg -n "browser-mcp|browser_mcp" scripts/router-rs/src tools/browser-mcp
```

**Clippy**：减法 PR 已跑 `cargo clippy --manifest-path scripts/router-rs/Cargo.toml -- -D warnings`（通过）。

---

## 9. 计划执行对照（仓库 todos）

| Plan todo id | 状态 | 说明 |
|--------------|------|------|
| anchor-steadystate | done | §1 |
| rust-dead-inventory | done | §2 + `cargo test` router-rs |
| config-dual-source | done | §3 + `policy_contracts` |
| host-glue-browser | done | §4 |
| history-diff | done | §5 |
| findings-doc | done | 本文件 |
| plan-gitx-closeout | **需宿主** | 见 §10 |

---

## 10. Git 与 `/gitx plan` 收口

- **本 agent 已执行**：`git status`（提交前请本地查看）；测试见 §2 / §3。
- **宿主仍需执行**：按 [skills/gitx/SKILL.md](../../skills/gitx/SKILL.md)，在 Cursor/Codex 会话对本轮变更执行 **`/gitx plan`**（与 **`/gitx`** 同契约），完成「计划 vs 实际」逐项签字。

---

## 11. 已执行减法（实现记录）

| 项 | 动作 | 验证 |
|----|------|------|
| `framework_runtime/mod.rs` | 移除与子模块重复的 dead 私有函数、`chrono` 未用 import、冗余 `write_*` 包装；收紧 `constants` / `types` import | `cargo test --manifest-path scripts/router-rs/Cargo.toml`；`cargo clippy … -D warnings` |
| `cursor_hooks/` | 移除未调用（历史记录） `read_json_strict`、`truncate_utf8_chars` | 同上 |
| §7 第 3–5 条 | **defer** 至后续 PR（browser-mcp 文档、tiers/loadouts 契约、`runtime_storage` 审计） | 未改对应路径 |
