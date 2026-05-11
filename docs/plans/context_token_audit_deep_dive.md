# Token / 上下文占用 — 代码路径深度调研

**类型**：在 [context_token_audit_build_report.md](context_token_audit_build_report.md) 静态基线之上的 **router-rs 注入路径** 补全。  
**时间**：2026-05-11。  
**范围**：`scripts/router-rs/src/` 连续性 / hook / digest；`docs/harness_architecture.md` §8 交叉核对。  
**Non-goals**：未改 hook 默认行为；未用宿主内置 token 计数器。

---

## 1. 相对基线报告的「代码路径缺口」表（≥5）

| 缺口主题 | 基线报告覆盖 | 本报告对应证据 |
|----------|--------------|----------------|
| **谁在什么相位写入模型侧上下文** | 仅习惯叙述 | `cursor_hooks.rs` / `codex_hooks.rs` 各事件分支；Codex `SessionStart` 调 `build_framework_continuity_digest_prompt(repo_root, 4)`（`codex_hooks.rs`） |
| **合并后是否有字符上限** | 未区分宿主 | Codex：`codex_compact_contexts` + `truncate_codex_additional_context`，默认 **640** 字符（可调 `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX`，clamp **256–8192**）。Cursor：`merge_additional_context` **仅追加字符串**，**无**合并后总长度 cap（见 §3 风险） |
| **SessionStart digest 行数上界** | 未写 | `continuity_digest.rs`：`max_lines` 经 `clamp(2, 4)` 限制；与传入参数（如 Codex 用 `4`）共同约束基线正文长度 |
| **PostTool → 磁盘证据膨胀** | 提到 EVIDENCE 条数现象 | `framework_runtime/mod.rs`：`MAX_POST_TOOL_EVIDENCE_ARTIFACTS = 120`，超出 **drain 头部**；`command_preview` 截断 **2000** UTF-8 字符；关断：`ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=0` |
| **大段 JSON 调试刮取** | 未提 | `cursor_hooks.rs`：`HOOK_JSON_STRING_SCRAPE_CAP = 2 * 1024 * 1024`（2MiB 级字符串拼接预算，属极端排障路径） |
| **续跑文案 verbose** | 仅 env 名 | `router_env_flags::router_rs_goal_prompt_verbose()` 同时影响 digest 内 Goal 段落、`build_autopilot_drive_followup_*`、`build_rfv_loop_followup_*`（及 pre-goal 路径，见各文件注释） |
| **operator 文案与 digest 硬编码行** | harness §8 有表 | `continuity_digest.rs`：`ROUTER_RS_HARNESS_OPERATOR_NUDGES=0` **不**去掉深度自检行；`depth_compliance_refresh_hint` 仍追加到 digest |
| **账本 → 门控读盘（非整段注入）** | 注册表/Goal 不一致已记 | `cursor_hooks.rs` `hydrate_goal_gate_from_disk`：读 `GOAL_STATE` + `EVIDENCE_INDEX` **布尔/摘要**补全门控，与「把整份 EVIDENCE 贴进 prompt」不同；但若 **agent 或用户** `read_file` 全文件仍属 Tier0 误用 |

---

## 2. Cursor / Codex 宿主对照（字段、cap、去重）

| 维度 | Codex | Cursor |
|------|-------|--------|
| **主要注入字段** | `hookSpecificOutput.additionalContext`（JSON 字符串） | `additional_context` 与/或 `followup_message`（由 `ROUTER_RS_CURSOR_HOOK_CHAT_FOLLOWUP` 决定续跑类段落写哪字段） |
| **合并策略** | `codex_compact_contexts`：段落 trim 后 **按全文小写 key 去重**，再 `join("\n")` | `merge_additional_context`：**直接 `\n\n` 追加**，整段再经 `scrub_spoof_host_followup_lines`；**无**总字符 cap |
| **硬 cap** | 默认 **640** 字符；`ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX` 覆盖（256–8192）；超长在 **换行边界**截断 + `...` | 无统一 cap；错误输出路径 `MAX_ERROR_LINES = 20`（`truncate_lines`）；另有 `truncate_utf8_chars_local` 用于部分片段（如 goal 预览） |
| **续跑段落刷新** | N/A（表结构不同） | `merge_hook_nudge_paragraph`：按段落首行前缀 **strip 再 append**（`AUTOPILOT_DRIVE` / `RFV_LOOP_CONTINUE` 等），减少重复堆叠 |
| **SessionStart digest** | `build_framework_continuity_digest_prompt(..., 4)` + 后续与 `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX` 的整体截断 | Cursor 侧 digest 若合并进输出，同样受各相位 `merge_*` 行为约束（以实际 hook 分支为准） |

**结论**：Codex 对 `additionalContext` 有 **明确上界**；Cursor 对 `additional_context` **依赖**「少合并 + SILENT 剥离 + 段落替换」而非总量 cap，长会话下 **理论上可线性增长**（若多路径反复 `merge_additional_context` 且未命中 SILENT 全剥）。

### 2.1 Claude Code（`claude_hooks.rs`）

与 Cursor/Codex 的「大块 continuity digest / 续跑」路径不同：Claude 侧 `add_context` 仅把 **短常量串** 写入 `hookSpecificOutput.additionalContext`（见 `add_context` 与 `SETTINGS_CHANGED_CONTEXT` / `FRAMEWORK_CHANGED_CONTEXT` / `AUTOMATION_CONTEXT`，约 [scripts/router-rs/src/claude_hooks.rs](../../scripts/router-rs/src/claude_hooks.rs) L10–L15、L100–L107、L159–L196）。**无** `codex_compact_contexts` 式去重、**无**与 `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX` 对等的本文件内截断；因文案长度固定为几句英文提示，**默认不构成**与 Cursor `merge_additional_context` 无界追加同级的 token 风险。PostTool 仅在「触达 settings 与/或 framework 守护路径」时合并上述短串。

---

## 3. Continuity digest / Goal / RFV 与开关交叉

| 文案块 | `ROUTER_RS_GOAL_PROMPT_VERBOSE` | `ROUTER_RS_OPERATOR_INJECT` | `ROUTER_RS_HARNESS_OPERATOR_NUDGES` |
|--------|--------------------------------|-----------------------------|--------------------------------------|
| digest 基线 + `depth_compliance_refresh_hint` | 否（hint 独立） | 不拦截 digest 主线 | 不拦截 depth rollup 行（harness §8 已写明） |
| digest 内 **Goal 段落**（`format_goal_state_digest_section_*`） | **是**（compact vs verbose 模板） | 不直接关 digest | **是**（去掉 JSON 内 nudge 行；**深度自检行仍保留**） |
| **AUTOPILOT_DRIVE** followup | **是** | **是**（总闸 off → 整块不注入） | **是**（compact/verbose 内嵌 nudge 句） |
| **RFV_LOOP_CONTINUE** followup | **是** | **是** | **是** |
| `ROUTER_RS_RFV_EXTERNAL_STRUCT_HINT` | 否 | 受 `OPERATOR_INJECT` 与 RFV 续跑总开关链式约束（见 `router_env_flags` 注释） | 否 |

实现参考：`framework_runtime/continuity_digest.rs`，`autopilot_goal.rs`（`build_autopilot_drive_followup_*`），`rfv_loop.rs`（`build_rfv_loop_followup_*`，`rfv_followup_compact_line` 对 goal 文本 **120** 字符级紧凑化）。

---

## 4. 账本与 PostTool 数据流（EVIDENCE_INDEX）

**数据流（一句）**：宿主 PostTool（及 `framework hook-evidence-append`）在满足连续性就绪 + 启发式「验证类命令」时，向 `artifacts/current/<task>/EVIDENCE_INDEX.json` 的 `artifacts[]` **追加**行；`task_state` / closeout / RFV **按路径读 JSON** 做深度与合规交叉检查；digest **不默认嵌入**整份 EVIDENCE 正文，但若模型 **`read_file` 全文件** 仍会进入对话上下文。

**增长与上界**：`MAX_POST_TOOL_EVIDENCE_ARTIFACTS = 120`（`framework_runtime/mod.rs`），与基线报告中 **120 条、约 50KB** 一致；满后 **丢弃最旧** 行。

**采样策略（建议）**：

- 日常：`router-rs framework snapshot` / `contract-summary`（若有）优先于整文件 `cat`。
- 排障：只读 `artifacts` 数组 **尾部 N 条**（`jq '.artifacts[-5:]'`）。
- 降噪：`ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=0` 停止自动追加（仍保留手动 `hook-evidence-append` 等路径时需再核对）。

---

## 5. `router_env_flags` 与散落 `std::env::var("ROUTER_*")`**

**集中登记**：[`scripts/router-rs/src/router_env_flags.rs`](../../scripts/router-rs/src/router_env_flags.rs) — 续跑、beforeSubmit opt-in、`GOAL_PROMPT_VERBOSE`、`OPERATOR_INJECT`、depth strict、RFV struct hint 等。

**未纳入 `router_env_flags`、在其它模块直读的环境变量（调研登记）**：

| 变量 | 模块 | 与上下文关系 |
|------|------|----------------|
| `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX` | `codex_hooks.rs` | Codex `additionalContext` **硬 cap**（文档已列为窄域例外） |
| `ROUTER_RS_CURSOR_SESSION_NAMESPACE` / `WORKSPACE_ROOT` / `TERMINAL_KILL_MODE` / `KILL_STALE_TERMINALS` | `cursor_hooks.rs` | 多为路径/终端生命周期，非 prompt 体积主因 |
| `ROUTER_RS_CURSOR_AUTOPILOT_PRE_GOAL_MAX_NUDGES` 等 | `cursor_hooks.rs` | pre-goal 次数 cap，影响 beforeSubmit **条数** |
| `ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE` / `HOOK_SILENT` / `MAX_OPEN_SUBAGENTS` / `OPEN_SUBAGENT_STALE_AFTER_SECS` | `cursor_hooks.rs` | 门控与静默剥离；SILENT 对含 `REVIEW_GATE` 等关键字 **保留**（见 harness §8） |
| `ROUTER_RS_CLOSEOUT_ENFORCEMENT` | `framework_runtime/mod.rs` | CI/本地 closeout 硬软路径 |
| `ROUTER_RS_GENERATOR_TIMEOUT_SECONDS` / `ROUTER_RS_BIN` | `host_integration.rs` | 生成/路径，非 prompt |
| `ROUTER_RS_STORAGE_ROOT` | `runtime_storage.rs` | 存储根 |
| `ROUTER_RS_SHARED_TARGET` | `router_self.rs` | 共享 target |
| `ROUTER_RS_UPDATE_*` | `framework_maint.rs` | `/update` 维护流 |
| `ROUTER_RS_OPERATOR_INJECT`（测试锁） | `harness_operator_nudges.rs` / `paper_adversarial_hook.rs` | 测试与论文 hook 读取 |
| `ROUTER_RS_DEPTH_SCORE_MODE` | `task_state.rs` | digest / statusline 深度 rollup |
| `ROUTER_RS_CURSOR_HOOK_CHAT_FOLLOWUP` | `autopilot_goal.rs`（测试） | 与 `router_env_flags` 定义一致，测试里保存/恢复 |

**建议**：新增 `ROUTER_RS_*` 时优先进 `router_env_flags` + `docs/harness_architecture.md` §8（与仓库扩展规则一致）。

---

## 6. 热点表（P0–P2）与「异常」可观测判据

### 6.1 热点表（≥8 行）

| ID | 来源 | 位置 / 配置 | 默认 | 体积或放大机制 | 旋钮 / 备注 |
|----|------|-------------|------|----------------|-------------|
| H1 | 静态 | `AGENTS.md` + `.cursor/rules/*.mdc` | alwaysApply | ~19KB + ~5KB 级（见基线 wc） | 策略收敛、避免重复真源 |
| H2 | 静态误用 | `skills/SKILL_ROUTING_RUNTIME.json` | 路由热文件 | ~63KB | **勿整文件进对话**；`jq` 取单条 `records[]` |
| H3 | L5 注入 | `configs/framework/HARNESS_OPERATOR_NUDGES.json` | 开 | 每续跑段 +N 行 | `ROUTER_RS_HARNESS_OPERATOR_NUDGES=0`；总闸 `ROUTER_RS_OPERATOR_INJECT=0` |
| H4 | L5 可选 | `configs/framework/PAPER_ADVERSARIAL_HOOK.txt` | opt-in | beforeSubmit 短文 | `ROUTER_RS_CURSOR_PAPER_ADVERSARIAL_HOOK` + 总闸 |
| H5 | digest | `continuity_digest` + SessionStart | 有连续性即有 | 基线 clamp **2–4** 行 + Goal 段 + depth hint | `ROUTER_RS_GOAL_PROMPT_VERBOSE`；`HARNESS_OPERATOR_NUDGES` |
| H6 | 续跑 | `autopilot_goal` / `rfv_loop` followup | drive/active 时 | compact 已较短；verbose 明显变长 | `ROUTER_RS_GOAL_PROMPT_VERBOSE`；`ROUTER_RS_AUTOPILOT_DRIVE_HOOK` / `ROUTER_RS_RFV_LOOP_HOOK`；beforeSubmit opt-in 两变量 |
| H7 | 磁盘 | `EVIDENCE_INDEX.json` | PostTool 默认追加 | **最多 120** 行 × preview≤2000 字符/行量级 | `ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=0`；归档任务目录 |
| H8 | Cursor 合并 | `merge_additional_context` | 各相位多次调用 | **无总 cap**（与 H6 叠加） | 依赖 SILENT、段落 strip；长期需观察是否需产品级 cap |
| H9 | 状态不一致 | `task_registry` vs `GOAL_STATE`（基线已记） | 视工作流 | 心智噪声 / 误续跑 **非**直接 token，但增加 followup 触发概率 | `complete` / `clear` + 对齐 registry |

### 6.2 P0 / P1 / P2 建议（仅调研结论）

- **P0（优先核实）**：Cursor `additional_context` **无界追加**假设是否在长任务中成立；若成立，属于宿主适配层产品债。
- **P1**：`EVIDENCE_INDEX` 满 120 后仍被 **整文件** 读入模型上下文的路径（agent 习惯 / 工具误用）。
- **P2**：`ROUTER_RS_GOAL_PROMPT_VERBOSE=1` 与 `HARNESS_OPERATOR_NUDGES` 全开时的 **文案乘法**（digest + Stop + beforeSubmit opt-in）。

### 6.3 「异常」可观测判据（自查用）

1. **单轮 hook 输出**中 `additional_context` 字节数异常大（需在宿主侧或日志中采样，本仓库未内置 token 计）。
2. **`EVIDENCE_INDEX` artifacts.length == 120** 且单次对话多次 `read_file` 该 JSON。
3. **`GOAL` completed** 但 `task_registry` 仍为 `in_progress`（基线 §2.3）→ 续跑启发式可能仍偏真。
4. **verbose + beforeSubmit 双续跑 opt-in 全开** → 同一轮多次 AUTOPILOT/RFV 文案叠加。

---

## 7. 复现命令（只读）

```bash
cd /Users/joe/Documents/skill

# 基线环境（与 build_report 一致）
printenv | grep -E '^ROUTER_' || true

# 符号定位（本报告 §2–§5）
rg -n "merge_additional_context|codex_additional_context_max_chars|MAX_POST_TOOL_EVIDENCE" scripts/router-rs/src/cursor_hooks.rs scripts/router-rs/src/codex_hooks.rs scripts/router-rs/src/framework_runtime/mod.rs
rg -n "router_rs_goal_prompt_verbose|build_autopilot_drive_followup|build_rfv_loop_followup" scripts/router-rs/src/router_env_flags.rs scripts/router-rs/src/autopilot_goal.rs scripts/router-rs/src/rfv_loop.rs
rg -n "std::env::var\(.*ROUTER" scripts/router-rs/src --glob "*.rs"

# 账本体积（若 shell 对 find 有别名，请用 /usr/bin/find）
/usr/bin/find artifacts/current -maxdepth 4 -name 'EVIDENCE_INDEX.json' -print -exec wc -c {} \;
```

---

## 8. 计划收口（gitx）

- **本调研交付**：本文件已落盘 `docs/plans/context_token_audit_deep_dive.md`。
- **`/gitx plan`**：须在 **Cursor 本会话** 由用户或宿主执行（子 shell 无法等价替代）；用于对照计划 todos 与 Git 工作区（与 `skills/gitx/SKILL.md` 契约一致）。

---

## 9. 验证记录（本代理已执行）

| 命令 | 结果 |
|------|------|
| `test -f docs/plans/context_token_audit_build_report.md && rg -n "ROUTER_RS_" docs/harness_architecture.md \| head -n 40` | 通过（harness §8 命中） |
| `rg merge_additional_context\|codex_additional_context… cursor_hooks codex_hooks` | 通过 |
| `rg GOAL_PROMPT_VERBOSE\|build_.*followup\|continuity digest/autopilot/rfv` | 通过 |
| `rg EVIDENCE_INDEX\|POSTTOOL\|posttool scripts/router-rs/src` | 通过（节选 50 行内） |
| `rg std::env::var\(.*ROUTER scripts/router-rs/src` | 通过（散落清单已汇总 §5） |

---

## 10. 执行核对（「Token调研执行核对」计划，2026-05-11）

本节约束：**未修改** `.cursor/plans/token调研执行核对_*.plan.md` 磁盘计划文件；仅更新本调研正文。

### 10.1 阶段 A — `printenv` 与符号 `rg`（子 shell）

| 步骤 | 结果 |
|------|------|
| `printenv \| grep -E '^ROUTER_'` | **无输出**（未设置 `ROUTER_*`；与默认矩阵一致，与 [context_token_audit_build_report.md](context_token_audit_build_report.md) §1 一致） |
| `rg merge_additional_context\|codex_additional_context_max_chars\|MAX_POST_TOOL_EVIDENCE`（三文件） | 命中 `cursor_hooks.rs:1411` 及多处 `merge_additional_context` 调用点；`codex_hooks.rs:101/681`；`framework_runtime/mod.rs:913/1026-1027`（常量 **120**） |

### 10.2 阶段 B — `EVIDENCE_INDEX.json` 体积（`/usr/bin/find`）

**说明**：本机默认 `find` 可能被别名拦截；复现请使用 **`/usr/bin/find`**。

| 路径 | `wc -c` | `artifacts.length`（`jq`） |
|------|---------|------------------------------|
| `artifacts/current/depth-enforcement-2026-05-10/EVIDENCE_INDEX.json` | **48727**（2026-05-11 复跑；此前记录 48629） | **120**（与 `MAX_POST_TOOL_EVIDENCE_ARTIFACTS` 顶格一致，见 [context_token_audit_build_report.md](context_token_audit_build_report.md) §2.3） |

### 10.3 附件原 Token 计划「四条证据链」↔ 本文章节打勾

| 证据链轴 | 原附件计划要求 | 本文落点 | 状态 |
|----------|----------------|----------|------|
| 静态体积与误用 | `AGENTS.md`、`.mdc`、`SKILL_ROUTING_RUNTIME.json`、L5 JSON | §6 热点 H1–H4；基线 [context_token_audit_build_report.md](context_token_audit_build_report.md) §3 | 已覆盖 |
| 宿主注入与截断 | `cursor_hooks` / `codex_hooks` / `claude_hooks` | §2（Cursor/Codex）、**§2.1（Claude）** | 已覆盖 |
| Continuity / Goal / RFV | `continuity_digest`、`autopilot_goal`、`rfv_loop` | §3 | 已覆盖 |
| 磁盘账本与 PostTool | `EVIDENCE_INDEX`、`ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE` | §4、§6 H7；**§10.2** 实测条数 | 已覆盖 |

### 10.4 计划 vs 实际（交付物清单）

| 原 Token 计划交付物 | 实际 |
|---------------------|------|
| 热点表（来源、体积线索、旋钮） | §6.1（H1–H9） |
| 异常可观测判据 | §6.3 |
| 复现命令块 | §7；本节补充 **`/usr/bin/find`** 注意 |
| `claude_hooks` 记入对比表 | **§2.1**（本轮补齐） |

**Defer / 宿主**：`/gitx plan` 仍须用户在 **Cursor 本会话** 执行（见 §8），本子 shell 无法替代。

### 10.5 Git 工作区快照（执行时）

**2026-05-11 复跑**：以下块为首次核对快照；**最新** `git status -sb` 见 **§11.6**。

以下为执行本核对当日 `git status -sb` 摘录（仅作审计；不隐含「应提交」范围）：

```text
## cursor/paper-adversarial-skills
 M .codex/host_entrypoints_sync_manifest.json
A  .cursor/rules/cursor-plan-output.mdc
 M .cursor/rules/execution-subagent-gate.mdc
 M .cursor/rules/review-subagent-gate.mdc
M  .github/workflows/skill-ci.yml
 M AGENTS.md
 M README.md
AM configs/framework/REVIEW_ROUTING_SIGNALS.json
 M configs/framework/RUNTIME_REGISTRY.json
A  configs/framework/cursor-hooks.workspace-template.json
 M docs/harness_architecture.md
 M docs/host_adapter_contract.md
AM docs/plans/cursor_cross_workspace_operator_checklist.md
 M docs/references/rfv-loop/reasoning-depth-contract.md
 M docs/rfv_loop_harness.md
 M docs/rust_contracts.md
A  scripts/cursor-bootstrap-framework.sh
 M scripts/router-rs/src/autopilot_goal.rs
AM scripts/router-rs/src/claude_hooks.rs
 M scripts/router-rs/src/cli/args.inc
 M scripts/router-rs/src/cli/dispatch.rs
 M scripts/router-rs/src/cli/dispatch_body.txt
 M scripts/router-rs/src/closeout_enforcement.rs
 M scripts/router-rs/src/codex_hooks.rs
 M scripts/router-rs/src/framework_host_targets.rs
 M scripts/router-rs/src/framework_runtime/continuity_digest.rs
 M scripts/router-rs/src/hook_common.rs
 M scripts/router-rs/src/host_integration.rs
 M scripts/router-rs/src/main.rs
 M scripts/router-rs/src/main_tests.rs
AM scripts/router-rs/src/review_routing_signals.rs
 M scripts/router-rs/src/rfv_loop.rs
 M scripts/router-rs/src/route/eval.rs
 M scripts/router-rs/src/route/mod.rs
 M scripts/router-rs/src/route/routing.rs
 M scripts/router-rs/src/route/scoring.rs
 M scripts/router-rs/src/route/signals.rs
 M scripts/router-rs/src/route/text.rs
 M scripts/router-rs/src/route/types.rs
 M scripts/router-rs/src/router_env_flags.rs
 M scripts/router-rs/src/task_state.rs
 M scripts/skill-compiler-rs/src/main.rs
 M skills/SKILL_LOADOUTS.json
 M skills/SKILL_MANIFEST.json
 M skills/SKILL_PLUGIN_CATALOG.json
 M skills/SKILL_ROUTING_METADATA.json
 M skills/SKILL_ROUTING_RUNTIME.json
 M skills/autopilot/SKILL.md
 M skills/citation-management/SKILL.md
A  skills/citation-management/references/integrity-redlines.md
 M skills/experiment-reproducibility/SKILL.md
A  skills/experiment-reproducibility/references/research-record-minimum.md
 M skills/paper-reviser/SKILL.md
 M skills/paper-workbench/SKILL.md
 M skills/paper-workbench/references/RESEARCH_PAPER_STACK.md
 M skills/paper-workbench/references/edit-scope-gate.md
 M skills/paper-writing/SKILL.md
A  skills/paper-writing/references/claim-spine-and-section-contract.md
 M skills/paper-writing/references/storytelling-patterns.md
 M skills/plan-mode/SKILL.md
 M skills/statistical-analysis/SKILL.md
A  skills/statistical-analysis/references/causal-prereg.md
 M tests/host_integration.rs
 M tests/routing_eval_cases.json
?? .cursor/plans/
?? docs/plans/RESEARCH_codex_cursor_runtime_architecture.md
?? docs/plans/REVIEW_plan_review_adoption.md
?? docs/plans/benchmark_plan_practices.md
?? docs/plans/context_token_audit_build_report.md
?? docs/plans/context_token_audit_deep_dive.md
?? docs/plans/plan_review_adoption.md
?? docs/plans/plan_review_closeout.md
?? docs/plans/plan_review_findings_round1.md
?? docs/plans/plan_todo_checklist.md
?? docs/plans/research_skills_hooks_survey.md
```

---

## 11. 复跑签收（「Token执行核对复跑」计划）

**日期**：2026-05-11。约束：**未修改** `.cursor/plans/token执行核对复跑_*.plan.md`；仅更新本文件。

### 11.1 阶段 A — `printenv` 与 `rg`（子 shell）

| 检查项 | 结果 |
|--------|------|
| `printenv \| grep -E '^ROUTER_'` | **无输出**（与 §10.1 一致） |
| `rg`：`merge_additional_context` / `codex_additional_context_max_chars` / `MAX_POST_TOOL_EVIDENCE` | 行号与 §10.1 一致（`cursor_hooks.rs:1411`；`codex_hooks.rs:101/681`；`framework_runtime/mod.rs:913/1026-1027`） |
| `rg`：§7 另两条（`router_rs_goal_prompt_verbose` / `std::env::var(.*ROUTER`） | 已执行，命中集与 §5 / §3 叙述一致 |

### 11.2 阶段 B — `EVIDENCE_INDEX.json`

| 路径 | wc -c（字节） | artifacts 条数 |
|------|---------------|----------------|
| `artifacts/current/depth-enforcement-2026-05-10/EVIDENCE_INDEX.json` | **48727** | **120**（`jq` 查询 `length`） |

### 11.3 静态 `wc -c`（对照 [context_token_audit_build_report.md](context_token_audit_build_report.md) §3）

| 路径 | 本次（2026-05-11） | build_report 历史参考 |
|------|-------------------|-------------------------|
| `AGENTS.md` | 20352 | 19345 |
| `skills/SKILL_ROUTING_RUNTIME.json` | 62970 | 62855 |
| `.cursor/rules/chinese-output.mdc` | 539 | 539 |
| `.cursor/rules/cursor-plan-output.mdc` | 1183 | 1183 |
| `.cursor/rules/execution-subagent-gate.mdc` | 1875 | 1335 |
| `.cursor/rules/framework.mdc` | 626 | 626 |
| `.cursor/rules/review-subagent-gate.mdc` | 1676 | 1463 |

**结论**：数量级与 build_report 一致；`AGENTS.md` / runtime JSON / 部分 `.mdc` 相对历史审计有**正常漂移**（仓库持续变更）。

### 11.4 四条证据链签收

对照附件「Token / 上下文占用深度调研计划」四轴与本文 **§10.3**：静态 / 宿主注入 / digest·Goal·RFV / 磁盘账本 — **映射仍成立**，无需改 §2–§6 结构。

### 11.5 `/gitx plan`（宿主）

须在 **Cursor 本会话** 执行 **`/gitx plan`** 完成契约级 Git 收口；本子 shell **无法**替代。

### 11.6 Git 工作区快照（2026-05-11 复跑）

```text
## cursor/paper-adversarial-skills
 M .codex/host_entrypoints_sync_manifest.json
A  .cursor/rules/cursor-plan-output.mdc
 M .cursor/rules/execution-subagent-gate.mdc
 M .cursor/rules/review-subagent-gate.mdc
M  .github/workflows/skill-ci.yml
 M AGENTS.md
 M README.md
AM configs/framework/REVIEW_ROUTING_SIGNALS.json
 M configs/framework/RUNTIME_REGISTRY.json
A  configs/framework/cursor-hooks.workspace-template.json
 M docs/harness_architecture.md
 M docs/host_adapter_contract.md
AM docs/plans/cursor_cross_workspace_operator_checklist.md
 M docs/references/rfv-loop/reasoning-depth-contract.md
 M docs/rfv_loop_harness.md
 M docs/rust_contracts.md
A  scripts/cursor-bootstrap-framework.sh
 M scripts/router-rs/src/autopilot_goal.rs
AM scripts/router-rs/src/claude_hooks.rs
 M scripts/router-rs/src/cli/args.inc
 M scripts/router-rs/src/cli/dispatch.rs
 M scripts/router-rs/src/cli/dispatch_body.txt
 M scripts/router-rs/src/closeout_enforcement.rs
 M scripts/router-rs/src/codex_hooks.rs
 M scripts/router-rs/src/framework_host_targets.rs
 M scripts/router-rs/src/framework_runtime/continuity_digest.rs
 M scripts/router-rs/src/hook_common.rs
 M scripts/router-rs/src/host_integration.rs
 M scripts/router-rs/src/main.rs
 M scripts/router-rs/src/main_tests.rs
AM scripts/router-rs/src/review_routing_signals.rs
 M scripts/router-rs/src/rfv_loop.rs
 M scripts/router-rs/src/route/eval.rs
 M scripts/router-rs/src/route/mod.rs
 M scripts/router-rs/src/route/routing.rs
 M scripts/router-rs/src/route/scoring.rs
 M scripts/router-rs/src/route/signals.rs
 M scripts/router-rs/src/route/text.rs
 M scripts/router-rs/src/route/types.rs
 M scripts/router-rs/src/router_env_flags.rs
 M scripts/router-rs/src/task_state.rs
 M scripts/skill-compiler-rs/src/main.rs
 M skills/SKILL_LOADOUTS.json
 M skills/SKILL_MANIFEST.json
 M skills/SKILL_PLUGIN_CATALOG.json
 M skills/SKILL_ROUTING_METADATA.json
 M skills/SKILL_ROUTING_RUNTIME.json
 M skills/autopilot/SKILL.md
 M skills/citation-management/SKILL.md
A  skills/citation-management/references/integrity-redlines.md
 M skills/experiment-reproducibility/SKILL.md
A  skills/experiment-reproducibility/references/research-record-minimum.md
 M skills/paper-reviser/SKILL.md
 M skills/paper-workbench/SKILL.md
 M skills/paper-workbench/references/RESEARCH_PAPER_STACK.md
 M skills/paper-workbench/references/edit-scope-gate.md
 M skills/paper-writing/SKILL.md
A  skills/paper-writing/references/claim-spine-and-section-contract.md
 M skills/paper-writing/references/storytelling-patterns.md
 M skills/plan-mode/SKILL.md
 M skills/statistical-analysis/SKILL.md
A  skills/statistical-analysis/references/causal-prereg.md
 M tests/host_integration.rs
 M tests/routing_eval_cases.json
?? .cursor/plans/
?? docs/plans/RESEARCH_codex_cursor_runtime_architecture.md
?? docs/plans/REVIEW_plan_review_adoption.md
?? docs/plans/benchmark_plan_practices.md
?? docs/plans/context_token_audit_build_report.md
?? docs/plans/context_token_audit_deep_dive.md
?? docs/plans/cursor_hooks_deep_dive_research.md
?? docs/plans/plan_review_adoption.md
?? docs/plans/plan_review_closeout.md
?? docs/plans/plan_review_findings_round1.md
?? docs/plans/plan_todo_checklist.md
?? docs/plans/research_skills_hooks_survey.md
```
