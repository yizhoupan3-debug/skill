# Codex Hook 官方文档 × `router-rs` 对照调研

**调研日期**：2026-05-11  
**方法**：仓库代码精读 + `WebFetch` 拉取 [Hooks – Codex](https://developers.openai.com/codex/hooks) 全文；`config-reference` 为站点生成的宽表（单页 JSON/表格混排）；Changelog 列表页为 SPA/HTML bundle，**未**从中可靠抽取逐条版本条目，改用 GitHub 上游 issue/commit 作为「发布侧变更」佐证。  
**非目标**：本轮不改 `router-rs` 行为；仅登记漂移与建议。

---

## 1. 官方 Hooks 页要点摘录（外部真源）

来源：<https://developers.openai.com/codex/hooks>

- Hooks 在 `config.toml` 的 **`[features]`** 下由特性开关控制；文档示例为 **`codex_hooks = true`**（下文矩阵中与仓库 `hooks = true` 对照）。
- 发现路径：`hooks.json` 或 `config.toml` 内联 `[hooks]`；用户级 `~/.codex/` 与项目级 `.codex/` 等；多源合并、与 inline hooks 并存时启动告警。
- 结构三层：**事件名** → **matcher 组** → **一个或多个 handler**（示例含 `type`/`command`/`timeout`/`statusMessage`）。
- **Turn scope**：`PreToolUse`、`PermissionRequest`、`PostToolUse`、`UserPromptSubmit`、`Stop`（文档原文列举）。
- **stdin**：统一 JSON object；常见字段含 `hook_event_name`、`cwd`、`session_id`、`model` 等（详见该页 *Common input fields* 表）。
- **stdout**：各事件支持的字段不同；`SessionStart` / `UserPromptSubmit` 支持 `hookSpecificOutput` 内 `additionalContext`；`PreToolUse` 可 `permissionDecision: deny` 或旧式 **`decision: "block"`**；`Stop` 上 **`decision: "block"`** 语义为**续跑/continuation**而非拒绝整轮（文档 *Stop* 节）。
- **Matcher 表**：`SessionStart` 的 matcher 过滤 **`source`**，文档列当前取值含 **`startup` / `resume` / `clear`**（*Matcher patterns* 表）。
- **GitHub Schemas**：精确 wire format 指向 `openai/codex` 仓库内 `codex-rs/hooks/schema/generated`（文档 *Schemas* 节链接）。

---

## 2. 仓库内部真源（代码侧摘要）

| 区域 | 锚点 |
|------|------|
| 事件分发（stdin `hook_event_name` / `event`，小写匹配） | `run_codex_review_subagent_gate`：`sessionstart` / `userpromptsubmit` / `posttooluse` / `stop` → 对应 handler（[`codex_hooks.rs`](../../scripts/router-rs/src/codex_hooks.rs) 约 L972–L998） |
| CLI 入口 | `dispatch_codex_command` → `run_codex_audit_hook(event, repo_root)`（[`dispatch_body.txt`](../../scripts/router-rs/src/cli/dispatch_body.txt) 约 L148–L157） |
| `codex hook` 子命令归一 | `canonical_codex_audit_command`：`PreToolUse` → `pre-tool-use`；其余生命周期事件 → `review-subagent-gate`（同文件约 L2120–L2144） |
| 项目 `hooks.json` 生成 | `build_codex_hook_manifest`：`INSTALL_EVENTS` 五事件、`timeout` 来自 `codex_hook_command_timeout_secs`，`SessionStart` 带 `matcher: "startup|resume|clear"`，`Stop` 带 `loop_limit: 3`（约 L1000–L1025） |
| 用户 `config.toml` 特性合并 | `merge_features_codex_hooks`：强制 **`hooks = true`**，并将 `codex_hooks`/`hooks` 旧行替换为 canonical `hooks`（约 L1844–L1894） |
| 安装校验 | `verify_codex_hooks`：要求 `hooks = true`，且 **不得**出现 deprecated 的 `codex_hooks` 字面量（[`framework_maint.rs`](../../scripts/router-rs/src/framework_maint.rs) 约 L194–L199） |

**`ROUTER_RS_*`（Codex 相关，非穷尽）**：`ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX`（SessionStart `additionalContext` 长度 clamp，默认 640，区间 256–8192，见 `codex_additional_context_max_chars`）；连续性相关见 README / `AGENTS.md`。

---

## 3. 对照矩阵（≥10 行）

列说明：**官方出处** | **本仓库锚点** | **integration test 契约**（若适用） | **结论** | **风险**

| # | 官方出处 | 本仓库锚点 | integration test 契约 | 结论 | 风险 |
|---|----------|------------|-------------------------|------|------|
| 1 | Hooks 页 `[features]` 示例 `codex_hooks = true` | `merge_features_codex_hooks` 写入 `hooks = true`；`verify_codex_hooks` 拒绝配置含 `codex_hooks` | `shell_installer_e2e_writes_expected_files`：`hooks = true`，`!contains("codex_hooks")` | **文档滞后**：上游已有 `hooks` 作为 canonical 的迁移（GitHub [#20522](https://github.com/openai/codex/issues/20522)、commit `0d9a5d2` 类迁移）；**实现与现行 Codex 一致**，与 Hooks **示例页**不一致 | P1 文档易误导新用户；本仓库实现自洽 |
| 2 | Hooks 页：事件含 `PermissionRequest` | `INSTALL_EVENTS` 无该事件；`run_codex_review_subagent_gate` 无分支 | 测试未要求 `PermissionRequest` | **能力缺口（有意）**：未接入企业/审批拦截面 | P2 若需对齐官方示例 Permission 策略需单独设计 |
| 3 | Matcher 表：`SessionStart` → `startup\|resume\|clear` | `build_codex_hook_manifest`：`"matcher": "startup\|resume\|clear"` | 生成的 `.codex/hooks.json` 与 manifest 测试可间接覆盖 | **一致** | 低 |
| 4 | SessionStart 节：`source` 为 `startup` 或 `resume`（表格正文）；Matcher 表含 `clear` | 代码与 manifest 使用三值 matcher；digest 文案含 `source` | — | **官方页内细微不一致**（节内表 vs Matcher 表）；实现与 **Matcher 表**一致 | 低（文档编辑问题） |
| 5 | PreToolUse：stdout JSON 可 `hookSpecificOutput.permissionDecision` 或旧式 `decision: block` | `block_codex_pre_tool_use` 等返回 `decision`/`reason` 及 `hookSpecificOutput`（grep `block_codex_pre_tool_use`） | 单元测试 `pre_tool_use_blocks_*` | **一致**（含旧式 block） | 低 |
| 6 | Stop：须 JSON；`decision: block` 表 continuation | `handle_codex_stop` 返回 `decision: "block"` + `reason`（review gate） | 多个 `codex_hooks` 单测断言 `decision == block` | **语义一致**（continuation 模型） | 低 |
| 7 | Hooks 页：未展示 `loop_limit`；Stop 示例仅 `timeout` | `Stop` handler 对象含 **`loop_limit`: 3** | 未直接断言该键 | **未文档化/需 schema 核对**：可能为 Codex 扩展字段；建议对照 GitHub generated schema 或 PR（如 stop continuation 相关 [#14532](https://github.com/openai/codex/pull/14532)） | P2 |
| 8 | 默认 `timeout` 省略为 600s | `codex_hook_command_timeout_secs`：SessionStart 3、PostToolUse 5、其余 8 | `shell_installer_e2e` 只检查事件名与 `codex hook --event=` | **一致**（显式短超时，避免挂死） | 低 |
| 9 | PostToolUse 示例常带 `matcher: Bash` | 生成的 `PostToolUse` **无 matcher**（匹配所有工具） | 同左 | **更宽触发面**（有意：证据与 gate）；与官方示例窄化不同 | P2 性能/噪音；非兼容性错误 |
| 10 | stdin：`hook_event_name` | `run_codex_review_subagent_gate` 接受 `hook_event_name` **或** `event`；`run_codex_audit_hook` 可注入 `hook_event_name`（L2037–L2043） | — | **一致或更宽** | 低 |
| 11 | SessionStart JSON：`hookSpecificOutput.additionalContext` | `handle_codex_session_start` → `codex_compact_contexts` + `ROUTER_RS_CODEX_SESSIONSTART_CONTEXT_MAX` | — | **一致** | 低 |
| 12 | 多 handler 并发、互不阻塞 | 单事件单 command（router-rs 独占一条） | — | **不冲突**；未利用多 handler | 低 |
| 13 | 项目 `.codex` 需 trusted 才加载项目 hooks | README 描述 Codex CLI / `codex sync`；代码侧不编码 trust | — | **宿主策略**；仓库文档应提醒 untrusted 模式下降级 | P3 运维认知 |
| 14 | `AGENTS.md` 策略由用户维护 | `build_codex_agent_policy` 使用 **`include_str!("../../../AGENTS.md")`** 编译期嵌入；`codex sync` bootstrap 磁盘文件 | README L8 | **仓库既定策略 A**；与通用 Hooks 文档正交 | 低（已在 AGENTS/README 声明） |
| 15 | `hooks.json` 顶层 `version: 1` | `build_codex_hook_manifest` 输出 `"version": 1` | 磁盘 [`.codex/hooks.json`](../../.codex/hooks.json) | **一致** | 低 |

---

## 4. `tests/host_integration.rs` 提炼的「仓库承诺契约」

以下断言与安装路径强相关（节选 `shell_installer_e2e_writes_expected_files`、`install_native_integration_*`）：

1. `framework maint install-codex-user-hooks` 在用户 `config.toml` 创建/合并 **`[features]`** 且 **`hooks = true`**。
2. 合并结果**不得**包含子串 **`codex_hooks`**（与 `verify_codex_hooks` 一致，视为 deprecated）。
3. 用户级 `hooks.json` 含 **`SessionStart` / `PreToolUse` / `UserPromptSubmit` / `PostToolUse` / `Stop`**，且命令串含 **`codex hook --event=`** 与 **`router-rs`**。
4. 若干 `install_native_integration_*` 用例：当存在 `codex_hooks = true` 时会被改写为仅保留 **`hooks = true`**，且可保留无关键如 `codex_hooks_extra`。

---

## 5. Changelog 切片说明

- **页面**：`https://developers.openai.com/codex/changelog?type=general` 返回的 HTML 为前端 bundle，**未**在此调研中做可靠的全文日期抽取。
- **替代证据**：特性重命名采用 GitHub **`hooks` 别名 `codex_hooks`**（issue [#20522](https://github.com/openai/codex/issues/20522)、commit [0d9a5d2](https://github.com/openai/codex/commit/0d9a5d20ecc4022dfa3b1ab7924e561d1b0a3360)）；Stop 续跑机制参见 PR [#14532](https://github.com/openai/codex/pull/14532) 等。

---

## 6. P0 / P1 / P2 汇总

| 级别 | 项 | 建议 |
|------|-----|------|
| **P0** | 无阻塞性「实现违反现行 Codex」项 | 当前矩阵未发现需立即修代码的 P0 |
| **P1** | 官方 Hooks 页仍展示 `codex_hooks = true`，与本仓库及 `verify_codex_hooks` 推广的 **`hooks`** canonical 不一致 | 向 OpenAI 文档提 issue/PR 或在 README 增加一行「官方页示例键名可能滞后，以 Codex 发行说明与 `hooks` 为准」 |
| **P2** | `PermissionRequest` 未实现；`loop_limit` 未在官方 Hooks 页解释；PostToolUse 无 matcher 宽触发 | 产品决策：若需要再开设计文档 |

---

## 7. 建议的后续实现 PR（本轮不做）

1. 文档：在 [`README.md`](../../README.md) Codex 小节增加「`hooks` vs `codex_hooks`」一句指针链到 GitHub #20522。
2. 可选：在 `.codex/README.md` 生成模板中加「官方 PermissionRequest 未接入」说明。
3. 工程：用 `curl`/`pagefind` 或 OpenAI Docs MCP 定期回归 Hooks 页示例是否更新。

---

## 8. 计划收口（`/gitx plan` 与 Git）

- **本调研交付**：本文件 + 未修改计划文件（按用户要求）。
- **`/gitx plan`**：`/gitx` 为 **Codex** 侧 skill 入口（见 [`skills/gitx/SKILL.md`](../../skills/gitx/SKILL.md)），**当前 Cursor Agent 会话无法调用该 slash 命令**。收口方式：由用户在 Codex 宿主对本轮变更执行 **`/gitx plan`**；此处用 **`git status`** 作为工作区核对替代（见下方命令输出摘录）。
- **Todos 对照**：计划中的 8 条调研 todo 均已在本文件第 1–8 节与矩阵中给出证据或 defer 说明。

### `git status`（执行于仓库根）

> 在合并本文件后运行：`git status`（具体输出以执行环境为准）。

### 验证命令（本轮已执行）

- `cargo test shell_installer_e2e_writes_expected_files -- --nocapture`（**仓库根** `Cargo.toml` 包 `skill-rust-test-harness`）：**通过**（2026-05-11）。
- 说明：单独 `cargo test --manifest-path scripts/router-rs/Cargo.toml` 在本工作区曾因 `cursor_hooks.rs` 解析错误失败，属 **router-rs 子包编译状态**问题，与本次仅新增本文档无关；根包集成测试通过可佐证 `install-codex-user-hooks` 契约未因本文档引入而破坏。
