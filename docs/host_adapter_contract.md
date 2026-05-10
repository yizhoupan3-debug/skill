# 宿主适配契约（Host adapter contract）

本文件描述 **新宿主如何接入** 本仓库的连续性 harness 与 `router-rs` 控制面：哪些能力可移植复用、宿主侧事件如何映射到 CLI、以及 L4/L5 边界。实现细节仍以代码与 [`harness_architecture.md`](harness_architecture.md) 为准。

**权威列表**：`configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported` 为 **当前闭集宿主 id**；安装/状态/卸载与 Codex **`host_entrypoints_sync_manifest`** 的 `supported_hosts` / `host_entrypoints` 由同一注册表推导（实现见 `router-rs` 中 `framework_host_targets`）。

**历史字段**：`host_targets.entrypoint_files` 已从注册表移除，请勿再添加；宿主策略入口与同步 manifest 的入口集合以 `host_targets.metadata.<host>.host_entrypoints` 为唯一权威（消费路径：`framework_host_targets::host_entrypoints_value_for_id`）。

**相关契约**（英文实现侧叙事）：[`rust_contracts.md`](rust_contracts.md)。

## 快速路径（我要接新宿主）

- **先读**：[`harness_architecture.md`](harness_architecture.md) **§5**（扩展规则）与 **§6**（文件映射），再读本文件 **[§3.1](#31-可复制执行清单工程顺序)** 工程清单与下文 **§0**（维护地图）。
- **再改**：按 §3.1 勾选推进；合并顺序见该节 **「PR 顺序」** 一行（`RUNTIME_REGISTRY` → hooks 模块 → `dispatch` → `host_integration` → L4 → 测试）。
- **跑测**：`cargo test --manifest-path scripts/router-rs/Cargo.toml`；若改动仓库根 [`tests/`](../tests/) 下用例，再于仓库根执行 `cargo test`。
- **`codex sync`**：仅当变更落在 **Codex** 投影链（含需进入 **编译期嵌入** 的 `AGENTS.md` 策略快照）时，在重建 `router-rs` 后执行 `router-rs codex sync --repo-root "$PWD"`（见仓库根 [`AGENTS.md`](../AGENTS.md)「Codex：`AGENTS.md` 构建快照」）。

---

## 0. North-star contract 与维护地图

**解耦**：宿主差异只允许停留在 L4 适配壳与 `RUNTIME_REGISTRY.host_targets` 元数据；L2 工件 schema、L3 CLI 行为、门控/证据追加逻辑必须复用 Rust owner。**无感**：用户在 Codex / Cursor 中得到同一套连续性、证据与 closeout 语义；差异只体现在安装工具名、宿主入口文件形状、hook 事件触发名称与宿主 HOME 解析。

**非目标**：不把每个宿主做成独立 runtime；不在 hook shell、`.mdc` 或 skill prose 中复制 L3 决策；不承诺所有宿主具备同等外部能力（如 tmux supervisor / GUI 自动化），只要求降级路径清晰。

| 维护面 | 不变量（共享） | 变量（宿主元数据 / 薄适配） | grep anchor |
|--------|----------------|-----------------------------|-------------|
| L2 连续性 | `artifacts/current`、`EVIDENCE_INDEX`、`GOAL_STATE`、`SESSION_SUMMARY` schema | 当前任务指针路径由 repo root 解析 | `CURRENT_ARTIFACT_DIR` / `EVIDENCE_INDEX_FILENAME` |
| 事件 × CLI × L2 | 宿主事件最终调用 `router-rs`，验证类命令追加同一 evidence row 形状 | Codex `PostToolUse` / Cursor `postToolUse` 的事件字段归一化 | `try_append_post_tool_shell_evidence` |
| 宿主安装/入口 | 闭集宿主 id 来自 `host_targets.supported`，缺元数据 fail-closed | `host_targets.metadata.<host>.install_tool` 与 `host_entrypoints` | `host_id_and_skills_install_tool_pairs_from_registry` |
| L4 / L5 边界 | L4 只做 argv/stdin/超时/路径转发；L5 只承载 skill 契约与可读叙事 | `.cursor/rules/*.mdc`、Codex `AGENTS.md` 投影形状不同 | `host_entrypoints_sync_manifest` |
| `${CODEX_HOME}/skills` | 表示 Codex 用户级 skill 投影根；仓库开发态优先 `skills/` | 仅 Codex install/sync 使用该 HOME 语义，Cursor 不复用 | `workspace_bootstrap_defaults.skills.user_dir` |

单行指针：五层模型见 [`harness_architecture.md`](harness_architecture.md)；Rust API / CLI 契约见 [`rust_contracts.md`](rust_contracts.md)；跨宿主语言、路由与执行协议见仓库根 [`../AGENTS.md`](../AGENTS.md)。

---

## 1. Portable core（宿主无关可复用面）

以下条件满足时，新宿主只需提供 **薄适配层**（转发 stdin/JSON、超时、路径解析），无需复制门控算法或业务 prose：

| 区域 | 内容 | 真源 / 约定 |
|------|------|----------------|
| L2 | 连续性工件、`EVIDENCE_INDEX`、`GOAL_STATE`、`SESSION_SUMMARY` 等 | `artifacts/current/`、`docs/harness_architecture.md` §1–§3 |
| L3 CLI | `router-rs framework snapshot|contract-summary|hook-evidence-append|…`、`closeout`、`task-state-*` | `docs/rust_contracts.md`、`RUNTIME_REGISTRY.json` → `framework_commands` |
| 共用门控启发式 | review / delegation / reject_reason / normalize_tool 等纯函数 | `scripts/router-rs/src/hook_common.rs` |
| Cursor review/subagent stdin 流水线 | stdin JSON → `dispatch_cursor_hook_event` → stdout JSON | `scripts/router-rs/src/review_gate.rs` + `cursor_hooks.rs` |

**反模式**：在 L4 shell/bash 里复制 L3 正则门控、`EVIDENCE_INDEX` 拼写规则或 RFV/G goal 拼接逻辑——应调用已有子命令或由 hook 二进制统一处理。

---

## 2. 事件 → `router-rs` CLI 对照（摘要）

以下内容只列 **入口与磁盘副作用字段名级别** 指针；细则见 [`harness_architecture.md`](harness_architecture.md) §3、「主数据流」与各宿主 `hooks.json`。

### Codex（`hooks.json`）

| 关注点 | 典型触发 | router-rs 路径 | 主要写盘 / 产出 |
|--------|----------|----------------|-----------------|
| 会话连续性 / digest / PostTool | 配置项指向 `router-rs codex hook …` | `codex hook`（`codex_hooks.rs`） | `EVIDENCE_INDEX`、`FRAMEWORK_DIGEST` / session 工件等（以 hook 分支为准） |
| 宿主入口对齐 | `router-rs codex sync` | 生成 `.codex/hooks.json`、`AGENTS.md` 等及 **`host_entrypoints_sync_manifest`** | 受 **`RUNTIME_REGISTRY.host_targets.supported`** 约束 |

### Cursor（`.cursor/hooks.json`）

| 关注点 | 典型触发 | router-rs 路径 | 主要写盘 / 产出 |
|--------|----------|----------------|-----------------|
| Review / subagent 门控、beforeSubmit/Stop | `router-rs cursor hook <event>` | `review_gate::run_review_gate` → `dispatch_cursor_hook_event` | `.cursor/hook-state/review-subagent-*.json`（及策略合并字段，见运行时） |
| 续跑类合并 | Same | `cursor_hooks.rs` + `autopilot_goal` / `rfv_loop` | `additional_context` / `followup_message`（宿主 JSON 出站字段） |

**统一原则**：宿主配置中的命令应保持 **短命 + 超时**；语义在 Rust，不在宿主脚本里分支业务规则。

---

## 3. 新宿主接入 Checklist

**路径表**（按职责；新宿主通常需各改一处或并列扩展）：

| 职责 | 仓库路径 |
|------|----------|
| Cursor hook 语义与出站 JSON | `scripts/router-rs/src/cursor_hooks.rs` |
| Codex hook 语义、`sync_host_entrypoints` 等 | `scripts/router-rs/src/codex_hooks.rs` |
| `framework host-integration install`、投影 manifest、入口模板 | `scripts/router-rs/src/host_integration.rs` |
| CLI 子命令注册与 `framework`/`cursor`/`codex` 分发 | `scripts/router-rs/src/cli/dispatch.rs`（及生成片段 `cli/dispatch_body.txt` 若适用） |
| 宿主侧事件绑定 | 仓库根 `.cursor/hooks.json`；Codex 侧 `.codex/hooks.json`（由 sync/install 写入） |
| 闭集宿主 id 与 `install_tool` / `host_entrypoints` | `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported` 与 `host_targets.metadata` |

1. **在 `RUNTIME_REGISTRY.json`** 扩展 `host_targets.supported`、`host_targets.metadata.<host>.install_tool` 与 `host_targets.metadata.<host>.host_entrypoints`（及若需要，`host_projections.*`）；`framework_host_targets.rs` 必须只从注册表读取这些值，并补齐 fail-closed 单测。
2. **薄 hook**：仅解析 workspace root / repo root → 组装 `router-rs …` argv；stdin 透传钩子 JSON。
3. **验证 L2**：在真实任务指针下跑一次验证类命令，确认 `EVIDENCE_INDEX` / `NEXT_ACTIONS` 等可按现有 schema 写入。
4. **再接入安装/投影**：`framework host-integration install --to …` 路径应注册到宿主专用安装函数（与其它宿主并列，`match`/`factory` 收敛在宿主集成模块内）。
5. **文档**：先更新本节或 `rust_contracts.md` Host 小节，再合入大范围行为改动（与 [`harness_architecture.md`](harness_architecture.md) **§7** 文末维护说明一致）。

**本轮边界**：[计划镜像 `plans/harness_host_round2.md`](plans/harness_host_round2.md) 将第三宿主 PoC **defer**；接入清单仍按闭集扩展编写，但 **不要** 向 [`RUNTIME_REGISTRY.json`](../configs/framework/RUNTIME_REGISTRY.json) 添加占位宿主 id。

### 3.1 可复制执行清单（工程顺序）

下列路径均为 **仓库根相对**；按顺序勾选可减少漏改。新宿主 CLI 约定仍为：**stdin JSON → Rust 结构化处理 → stdout JSON**（与现有 `cursor_hook` / `codex hook` 入口一致）。

**PR 顺序（建议单行记忆）**：`RUNTIME_REGISTRY`（及测试夹具中的最小 registry）→ `framework_host_targets` → `<host>_hooks` + `main` mod → `dispatch`（含 `dispatch_body.txt`）→ `host_integration` → L4 `hooks.json` → `tests/host_integration` / `tests/policy_contracts`。

| 阶段 | 主要落点 |
|------|----------|
| 注册表 / 契约 | `RUNTIME_REGISTRY.json`，[`tests/common/mod.rs`](../tests/common/mod.rs)，[`framework_host_targets.rs`](../scripts/router-rs/src/framework_host_targets.rs) |
| L3 入口 | 新增 `scripts/router-rs/src/<host>_hooks.rs`（命名对齐 [`codex_hooks.rs`](../scripts/router-rs/src/codex_hooks.rs) / [`cursor_hooks.rs`](../scripts/router-rs/src/cursor_hooks.rs)），[`main.rs`](../scripts/router-rs/src/main.rs) |
| CLI 分发 | [`dispatch_body.txt`](../scripts/router-rs/src/cli/dispatch_body.txt)，[`dispatch.rs`](../scripts/router-rs/src/cli/dispatch.rs) |
| 安装 / 投影 | [`host_integration.rs`](../scripts/router-rs/src/host_integration.rs)，[`GENERATED_ARTIFACTS.json`](../configs/framework/GENERATED_ARTIFACTS.json)（若新增生成物） |
| L4 + 验证 | 宿主 `hooks.json`，[`tests/host_integration.rs`](../tests/host_integration.rs)，[`tests/policy_contracts.rs`](../tests/policy_contracts.rs) |

- [ ] **[`configs/framework/RUNTIME_REGISTRY.json`](../configs/framework/RUNTIME_REGISTRY.json)**：在 `host_targets.supported` 追加宿主 id；补齐 `host_targets.metadata.<id>.install_tool` 与 `host_entrypoints`（字符串或 JSON 数组形状需与现网 `codex-cli` / `cursor` **对称**，避免半套映射）；若改动 `framework_commands` 等按宿主分列的表，逐键对照现有列。
- [ ] **[`tests/common/mod.rs`](../tests/common/mod.rs)**（及任何内嵌最小 registry 的测试夹具）：与真实 `RUNTIME_REGISTRY.json` 的 `host_targets` 块对齐，避免 CI 用「缩水 registry」与真源分叉。
- [ ] **[`scripts/router-rs/src/framework_host_targets.rs`](../scripts/router-rs/src/framework_host_targets.rs)**：确保只从注册表读取上述字段，fail-closed；必要时补充单元测试。
- [ ] **新增** `scripts/router-rs/src/<host>_hooks.rs`（命名对齐现有 [`codex_hooks.rs`](../scripts/router-rs/src/codex_hooks.rs) / [`cursor_hooks.rs`](../scripts/router-rs/src/cursor_hooks.rs)）：实现各生命周期分支；在 [`main.rs`](../scripts/router-rs/src/main.rs) 注册 `mod` 并导出入口。
- [ ] **[`scripts/router-rs/src/cli/dispatch_body.txt`](../scripts/router-rs/src/cli/dispatch_body.txt)** 与 [`scripts/router-rs/src/cli/dispatch.rs`](../scripts/router-rs/src/cli/dispatch.rs)：挂上 `router-rs <host> hook <event> …` 分发（与现有 `codex` / `cursor` 子命令并列）。
- [ ] **[`scripts/router-rs/src/host_integration.rs`](../scripts/router-rs/src/host_integration.rs)**：`framework host-integration install --to <tool>` 能解析注册表中的 `install_tool`；为该宿主增加投影写入（对标 `render_cursor_framework_entrypoint` / `render_codex_framework_entrypoint`）；若产生新的生成物路径，同步 [`configs/framework/GENERATED_ARTIFACTS.json`](../configs/framework/GENERATED_ARTIFACTS.json)（及代码中 `REQUIRED_GENERATED_ARTIFACTS` 等常量，若有）。
- [ ] **L4 样例**：检出中的 [`.cursor/hooks.json`](../.cursor/hooks.json)（Cursor）与由 sync 写入的 `.codex/hooks.json`（Codex）应保持 **argv + 超时 + stdin 透传**，不在 shell 内复制 L3 业务分支；新宿主应对照新增同级配置。
- [ ] **[`tests/host_integration.rs`](../tests/host_integration.rs)**：增加 dry-run 或临时目录安装断言（沿用现有 `host_targets.metadata` / manifest 断言模式）。
- [ ] **[`tests/policy_contracts.rs`](../tests/policy_contracts.rs)**（根包）：registry / 契约回归。
- [ ] **验证**：`cargo test --manifest-path scripts/router-rs/Cargo.toml`；仓库根 `cargo test`。
- [ ] **[`AGENTS.md`](../AGENTS.md)**：若新宿主属于 Codex 投影链且涉及策略快照与 **`codex sync`** 生命周期，在「权威分层」表补一句说明（避免第二叙事真源；与现有 Codex 编译期嵌入段落对齐）。

### 3.2 Maint / supervisor / profile 硬编码宿主耦合盘点

以下为 **盘点结论**（非要求本轮改代码）：第三宿主出现时优先扩展对应 `match` / 维护序列，或在 [`RUNTIME_REGISTRY.json`](../configs/framework/RUNTIME_REGISTRY.json) / 文档中写明能力降级，避免 silent drift。

| 位置 | 硬编码内容 | 建议 |
|------|------------|------|
| [`scripts/router-rs/src/framework_maint.rs`](../scripts/router-rs/src/framework_maint.rs) | `refresh_host_projections` 固定顺序：`codex sync` → `framework host-integration install --to cursor` → `verify_cursor_hooks`；另含 `VerifyCursorHooks` / `VerifyCodexHooks`、`install_codex_user_hooks`、打印/探测 `CODEX_HOME`·`CURSOR_HOME` 等路径 | **扩展**：新宿主纳入 refresh 与 verify；或 **文档**：声明 maint 默认仅刷新 codex+cursor |
| [`scripts/router-rs/src/session_supervisor.rs`](../scripts/router-rs/src/session_supervisor.rs) | `classify_rate_limit_block` 仅接受 `codex` / `codex-cli`；`build_driver_command` 仅组装 Codex CLI；`driver_id_for_host` 非 codex 映射为 `unknown_driver` | **扩展**：为新 CLI 宿主补 driver / 限速模式；或 **文档**：标明 supervisor 仅保障 Codex 驱动 |
| [`scripts/router-rs/src/framework_profile.rs`](../scripts/router-rs/src/framework_profile.rs) | `build_profile_bundle` 仅注入 `codex-cli` 的 `host_payloads`；`HostProfileKind::Codex`；产物字段 `codex_profile` / `full_codex_profile` 为 Codex 一等投影（无 `cursor` 字面量，但模块语义上 **非通用多宿主**） | **扩展**：并列构建第三宿主 profile bundle；或 **文档**：Cursor 投影由 `host_integration` 等路径承担，与本模块 Codex 工件分离 |

### 3.3 PostTool / 终端证据归一化（本轮未抽取）

Codex 侧 [`codex_hooks.rs`](../scripts/router-rs/src/codex_hooks.rs) 中 `PostToolUse` 直接将原生事件传入 [`try_append_post_tool_shell_evidence`](../scripts/router-rs/src/framework_runtime/mod.rs)；Cursor 侧在 [`cursor_hooks.rs`](../scripts/router-rs/src/cursor_hooks.rs) 中先用 `synthetic_codex_shape_for_post_tool_evidence` 将异构 `postToolUse` 合成 **同一 evidence 解析形状** 再调用同一 API，并额外承担终端归属、`rust-lint` 等 Cursor 专用分支。**共享归一化路径已在 `framework_runtime`**，再抽独立 `hook_posttool_normalize` 模块会把「仅 Cursor 需要的字段拆解」与通用层揉在一起，收益有限，故 **本轮不抽取**。

---

## 4. 与五层模型的对齐（L4 / L5）

与 [`harness_architecture.md`](harness_architecture.md) §1–§2 一致：

| 层 | 允许 | 禁止 |
|----|------|------|
| **L4** | 调用 `router-rs` 子命令、固定超时、环境透传 | 长段策略 prose、复制 L3 门控、手写 `EVIDENCE_INDEX` 规则 |
| **L5** | SKILL 契约、`verify_commands`、拒因枚举、编排叙事 | 第二套连续性目录、或与 L2 schema 冲突的并行真源 |

L3（`cursor_hooks` / `codex_hooks` / `framework_runtime` 等）负责合并续跑提示、采样 PostTool、持久化 gate 状态；**不得**承载领域产品长篇文案（应进配置文件或 L5 文档）。

---

## 5. 可选演进（非本轮承诺）

若未来需要将「仅 portable core」独立发布为多 crate workspace，可把 `hook_common` + 无宿主 IO 的路径进一步拆仓；当前单 crate `router-rs` 仍为默认形态。
