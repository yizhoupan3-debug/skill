> **调研范围与可执行性示例**。本文件从 `skills/plan-mode/SKILL.md` 提取的调研范围定义、能力联动表与 todo 弱/强示例。主文件仅保留指针；完整规范以本文件为准。

# 调研范围与能力联动

---

## 1. 调研范围声明

与 **`plan_profile`**、零实现面声明**并列**：在 `overview` 中用**一句**标明调研是否触网，避免默认静默拉取外部资料或反过来只做网页摘要却未读仓库。

### Overview 可加贴的调研范围句（复制用）

**默认（仅仓库内只读）** — 不默认发起 WebSearch / WebFetch；本地证据以 `rg`、Read、在**不修改** tracked 源码 / 配置 / 测试的前提下运行的 `cargo test`/`clippy`（只读验收读结果）、以及按需 **`router-rs framework snapshot`** / `contract-summary` 等为准（`target/` 等构建产物**不**计入 **research**「零实现面」对 **tracked** 资产的承诺范围）：

```text
调研范围：仅仓库内只读（rg/读文件/仓库内命令与连续性工件）；不默认发起对外网络检索。
不改 tracked 源码/配置/测试；`cargo test`/`clippy` 仅作只读验收读结果；`target/` 等构建产物不纳入对 **tracked** 的零实现面承诺。
```

**用户明确要求「外部 / 网络 / 官方 cross-check」等（内部 + 外部并行）** — 仍须保留至少一条**仓库内**检索/读文件类 todo；外部仅限 **只读** 拉取（WebSearch、WebFetch、只读 MCP）；**§证据与范围** 须写清 URL、抓取日期或文档版本：

```text
调研范围：仓库内只读 + 外部只读（与仓库内检索并行）；外部来源须在 §证据与范围 列 URL 与日期/版本；禁止未经批准的网络写操作。
```

---

## 2. 能力与工件联动表

| 能力 | 适用 profile | 最小证据 | 指针 |
|------|----------------|----------|------|
| 本地代码与配置调研 | `research` / `execution`（起草前） | 路径级 `rg` 命中或 Read 锚点 | 见 **Workflow** 第 1 步 |
| 连续性 / 框架只读视图 | 按需 | `router-rs framework snapshot` 或文档约定命令输出摘要 | `docs/harness_architecture/`（见 [`index.md`](../../../docs/harness_architecture/index.md)）；勿在 plan 正文发明第二套账本 |
| **可选审 plan** | 仅当用户明确要求 review plan / 审计划 | review-only findings（问题、风险、缺失验证），不改代码 | 可落盘 `docs/plans/<topic>_findings*.md` |
| 对抗式 / 全切片 **深度代码审** | 用户要 hostile / security / 整 PR 级 review 时 | review-only：默认 **severity 排序的 findings**（P0–P2 符号锚点），verdict 至多一行可选；只找问题，不改代码 | [`skills/code-review-deep/SKILL.md`](../../../skills/code-review-deep/SKILL.md) |
| Cursor **review** 硬路径（宿主） | 深度 review 类任务 | 以仓库根 **`AGENTS.md`** → **Execution Ladder** 与宿主 hook 状态为准；清门只用宿主注入的单行短码 | 不在 plan 正文自拟长段机读块；宿主差异见主文件 **宿主差异** |
| 调研收口 | `research` | `git status --porcelain` + 正文矩阵对照 | 主文件 **Plan profile** 末条 |
| Git 计划收口 | `execution`（或下游计划） | 计划 vs 实际 + Git 状态证据；宿主支持时用 **`/gitx plan`**（与 **`/gitx`** 同契约） | [`skills/gitx/SKILL.md`](../../../skills/gitx/SKILL.md) |

### 宿主侧计划落盘（与协作）

宿主侧的 Save to workspace 机制与计划落盘路径差异：见主文件 **宿主差异** 与 [`docs/plans/README.md`](../../../docs/plans/README.md)。

---

## 3. Todo 弱例与强例

### 弱例

```text
优化 registry 叐轨
```

问题：无范围、无 Done、无 Verify —— 不满足四元组中任何一项。

### 强例（完整四元组）

```text
从 RUNTIME_REGISTRY 移除 host_targets.entrypoint_files 并同步 fixture @ configs/framework/RUNTIME_REGISTRY.json, tests/common/mod.rs
| Done: rg 在 configs/framework 与 tests 下无该键（例外在 § 单列）
| Verify: cargo test --manifest-path core/router-rs/Cargo.toml
```

仓库以实际约定命令为准。

### 强例（execution 收口与 gitx 习惯对齐）

末条或关联 closeout 文档中写明 **`git diff --stat`**（或一句「本次无代码 diff」）和 `git status --short --branch`；宿主支持时 `Verify` 可附带 **`/gitx plan`**，与 [`skills/gitx/SKILL.md`](../../../skills/gitx/SKILL.md) 中实质性 diff 记录习惯一致。

### 强例（可选审 plan 修订可复核）

仅当用户明确要求审 plan / review plan 时，对**本计划文件**执行例如 `git diff plans/<本计划>.plan.md | head -n 40`（路径按实际替换），或将等价 diff 摘要写入 closeout；避免仅用 `rg Finding` 而看不到计划正文是否已合并修订。

### 强例（深度 review 防空壳）

若 todo 指向深度代码审，`Done` 须要求 P0–P2 中**至少一条**含具体**符号锚点**（函数名/常量名等）；`Verify` 用 `rg` 命中该符号之一。

---

## Related

- `skills/plan-mode/SKILL.md` — 跨宿主通用 plan-mode 规范（主文件）。
- [references/research-profile-guide.md](research-profile-guide.md) — 调研与执行 profile 详细指南。
- [references/cursor-createplan-contract.md](cursor-createplan-contract.md) — Cursor 宿主专属契约。
