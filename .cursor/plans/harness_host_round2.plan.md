---
name: Harness 宿主集成 Round 2
plan_profile: execution
overview: |
  本文件为执行计划（plan_profile: execution）。P0（`host_targets.entrypoint_files` 与 `metadata.host_entrypoints` 双轨）已在仓库真源收口（选项 A，见协作镜像与 `docs/host_adapter_contract.md`）。允许按下方 todos 继续修改本 plan、可选 `docs/plans/harness_host_round2.md` 同步说明、以及未完成项 `docs/host_adapter_contract.md` / `scripts/router-rs`（P1.2 等）；不扩大 trait/合并 hook 语义。末条 `round2-closeout-gitx` 以计划 vs 实际 + Git 状态证据收口，宿主支持时可使用 /gitx plan。
todos:
  - id: p0-archive
    content: |
      动作：对照 `RUNTIME_REGISTRY.json`、`rg entrypoint_files`、协作镜像，确认 P0 无回退；主副本 YAML/正文与镜像一致。
      范围：`configs/framework/RUNTIME_REGISTRY.json`；`.cursor/plans/harness_host_round2.plan.md`；`docs/plans/harness_host_round2.md`；`docs/host_adapter_contract.md`
      Done when：`configs/framework` 与 `tests/` fixture 无 `host_targets.entrypoint_files` 键；主 plan 无「待选 A/B/C 删键」误导句。
      Verify：`rg entrypoint_files configs/framework tests`（`docs/history/` 若存在历史提及可忽略）；`rg -n '待选 A/B/C|P0\\.2-A 删键' .cursor/plans/harness_host_round2.plan.md` 应无待执行删键类表述。
    status: completed
  - id: p1-choose
    content: |
      动作：`host_adapter_contract` §3 路径表已扩充则维持；`framework host scaffold --dry-run` 未立项则 defer 并写日期。
      范围：`docs/host_adapter_contract.md`；`scripts/router-rs` CLI（若 scaffold）
      Blocked by: p0-archive
      Done when：P1.1 路径表存在或 defer 含日期；若启动 scaffold 则有 dry-run 记录。
      Verify：`rg -n 'cursor_hooks|dispatch\\.rs|host_integration' docs/host_adapter_contract.md`
    status: pending
  - id: p2-inventory
    content: |
      动作：§P2 盘点与 P2.2 结论已与 `host_integration.rs` 对照落盘（见正文）；无新切片则保持「本轮跳过」。
      范围：`scripts/router-rs/src/host_integration.rs`；本文件 §P2
      Blocked by: p0-archive
      Done when：§P2 含日期与分支列表及 P2.2 结论。
      Verify：`rg -n 'P2 盘点|候选重复分支|P2\\.2 结论' .cursor/plans/harness_host_round2.plan.md`
    status: completed
  - id: p3-gate
    content: |
      动作：dispatch 共用层 PoC 未立项则勾选跳过并写日期；当前以 Claude Code 闭集与 registry 为准。
      范围：本文件 §P3；`configs/framework/RUNTIME_REGISTRY.json`
      Blocked by: p0-archive
      Done when：§P3 有跳过或立项指针。
      Verify：`rg -n 'p3-gate|P3|跳过|PoC' .cursor/plans/harness_host_round2.plan.md`
    status: completed
  - id: round2-closeout-gitx
    content: |
      动作：对照 YAML `todos` 与正文 §P0/§P2/§P3；记录 Git 证据。
      范围：仓库根；`.cursor/plans/harness_host_round2.plan.md`
      Done when：各 todo 状态与正文勾选一致；未完成项（如 P1.2）有 defer 或 pending 原因。
      Verify：`git status --short --branch`；`git diff --stat`；宿主支持时可 **`/gitx plan`**（`skills/gitx/SKILL.md`）。
    status: completed
isProject: false
---

# Harness 宿主集成 — 第二轮（Round 2）

## 执行计划继承面

| 字段 | 内容 |
|------|------|
| **继承指针** | [`docs/plans/harness_host_round2.md`](../../docs/plans/harness_host_round2.md)（状态校准 2026-05-12）；[`docs/host_adapter_contract.md`](../../docs/host_adapter_contract.md)（历史字段：`entrypoint_files` 已从注册表移除） |
| **Goal** | Round 2 主副本与仓库真源一致：P0 已归档；P1–P3 按镜像进度延续或显式 defer。 |
| **Non-goals** | N 宿主 trait、合并 Cursor/Codex hook 语义层；扩大 `RUNTIME_REGISTRY` 承载业务 skill 正文。 |
| **P0 状态** | `host_targets.entrypoint_files` **已移除**；权威为 `host_targets.metadata.<host>.host_entrypoints`（`framework_host_targets::host_entrypoints_value_for_id`）。不再打开 A/B/C 删键决策。 |

## 北极星（一句话）

把曾并存的 **`metadata.host_entrypoints`** 与第二套入口描述收口为 **单真源**（已完成）；其余宿主耦合 **盘点后再减法**，不预先堆抽象。

## 减法 + 第一性原理（执行前默读）

| 做 | 不做 |
|----|------|
| 删字段、加断言、文档钉死权威字段 | 在无证据时大改 `dispatch` / 合并两大 hook 文件 |
| 重复逻辑可计量再抽 | trait 森林、插件 ABI、「万能宿主」 |

**选型提示（评审结论）**：维护者不止一人时，**优先删键（本仓库 P0 已选 A）或机读一致**；仅文档标注仍可能误导后续编辑 registry 的人。

---

## 背景：为何曾存在双轨（Round 2 后）

| 位置 | 现状 |
|------|------|
| `host_targets` 下已删除的仅 AGENTS 映射键 | **已移除**；`router-rs` 从未消费该键 |
| `host_targets.metadata.<id>.host_entrypoints` | **真源**：Codex 字符串；Cursor 数组（含 `.cursor/rules/*.mdc`） |

权威消费路径：`framework_host_targets::host_entrypoints_value_for_id` → sync manifest 等。

---

## 可执行 To-do 清单（YAML `id` 与勾选一一对应）

### P0 — 双轨收口（已归档）

- [x] **p0-archive**：P0 决策 **A** 已在仓库执行；`RUNTIME_REGISTRY` / fixture 无 `entrypoint_files`；`host_adapter_contract.md` 钉死权威字段；协作镜像 [`docs/plans/harness_host_round2.md`](../../docs/plans/harness_host_round2.md) 已状态校准。归档日期以 §P0 决策记录为准。

### P1 — 新宿主脚手架（可选）

- [ ] **p1-choose**：[`docs/host_adapter_contract.md`](../../docs/host_adapter_contract.md) §3 路径表 **已完成**；`framework host scaffold --dry-run` — **defer（未立项，2026-05-12）**。

### P2 — `host_integration` 减法（可选）

- [x] **p2-inventory**：见 **§P2 盘点笔记**；P2.2 本轮无纯数据切片 → 跳过。

### P3 — dispatch 共用层（默认关闭）

- [x] **p3-gate**：第三宿主 PoC 未立项 → **已跳过**；当前 Claude Code 支持以后续 `RUNTIME_REGISTRY` 与实现为准。

---

## P0 决策记录（已执行，只读）

```
日期：2026-05-11（归档）；主副本 YAML 对齐：2026-05-12
选项：A — 从 `RUNTIME_REGISTRY.json` 的 `host_targets` 移除「仅 AGENTS 路径映射、运行时从不消费」的旧并列键；权威入口列表仅保留 `metadata.<host>.host_entrypoints`。

若选 B，对照规则（必填）：N/A

备注：多维护者默认倾向 A；勿再执行删键。键名历史见 `docs/host_adapter_contract.md` 历史字段说明。
```

---

## P2 盘点笔记（`host_integration.rs`）

**日期**：2026-05-11

**候选重复分支（路径 / 模板差异为主）**：

1. **入口文件落盘**：`codex_entrypoint_target`（`.codex/prompts/framework.md`）与 `cursor_entrypoint_target`（`.cursor/rules/framework.mdc`），含 `user` / `project` scope 分支。
2. **投影 manifest 路径**：`projection_manifest_path` 对 `codex-cli` / `cursor` × `user` / 默认 的矩阵；默认回落到 `project_root/.framework-projection.json`。
3. **入口正文模板**：`render_codex_framework_entrypoint` vs `render_cursor_framework_entrypoint`（YAML 头、`host_projection`、`globs` / `alwaysApply` 等宿主形状差异）。
4. **manifest 写入**：`write_codex_projection_manifest` 与 `write_cursor_projection_manifest`（`managed_key_paths`、托管文件列表形状不同）。
5. **Cursor 专有**：`cursor_mcp_config_path` / `install_cursor_mcp_server` 等与 Codex 无对称分支。

**P2.2 结论**：上述差异混合了 HOME 解析、模板字符串与宿主专有策略；**不存在**可单独迁入 `RUNTIME_REGISTRY` 的纯数据切片而不改行为契约。**本轮跳过**（2026-05-11）。

---

## P3 说明

**Round 2 当时第三宿主 PoC 未立项 — P3 跳过。当前 Claude Code 支持以后续实现与 `RUNTIME_REGISTRY` 为准。**

---

## Non-goals

- N 宿主 trait 体系、运行时插件、合并 Cursor/Codex hook 语义层。
- 扩大 `RUNTIME_REGISTRY` 去承载业务 skill 正文。

---

## 验证命令（每次合并前）

根目录包名为 `skill-rust-test-harness`；集成测文件名 `tests/policy_contracts.rs` 对应 **`cargo test --test policy_contracts`**（若将来拆分/改名，以根 [`Cargo.toml`](../../Cargo.toml) 与 `tests/` 实际文件名为准）。

```bash
cargo test --manifest-path scripts/router-rs/Cargo.toml
cargo test --test policy_contracts
cargo test --manifest-path scripts/router-rs/Cargo.toml framework_host_targets
```

---

## 可见性与索引

- **主副本**：`.cursor/plans/harness_host_round2.plan.md`（本文件，含 YAML todos）。
- **协作副本（p0-visibility）**：[`docs/plans/harness_host_round2.md`](../../docs/plans/harness_host_round2.md) + [`docs/README.md`](../../docs/README.md)「多宿主」表链。
- Round 1 真源：[`docs/host_adapter_contract.md`](../../docs/host_adapter_contract.md)、[`configs/framework/RUNTIME_REGISTRY.json`](../../configs/framework/RUNTIME_REGISTRY.json)。
