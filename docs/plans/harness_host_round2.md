# Harness 宿主集成 — 第二轮（Round 2）

**主副本**：`.cursor/plans/harness_host_round2.plan.md`（含 YAML todos）。本文件为 **协作镜像**：正文与清单与主副本对齐；执行状态以主副本或 PR 描述为准。

**状态校准（2026-05-12）**：本文件保留 Round 2 历史计划语境；Claude Code 后续已进入 `host_targets.supported` 闭集并具备 `claude_hooks.rs` / project projection。下文“第三宿主未立项 / P3 跳过”只描述 Round 2 当时边界，不再代表当前宿主集合。

**主副本 YAML**：`.cursor/plans/harness_host_round2.plan.md` 已于 2026-05-12 与上述 P0 状态及本镜像叙事对齐（含 frontmatter todos）。

---

## 北极星（一句话）

把 **`metadata.host_entrypoints`** 与注册表里曾并存的 **第二套入口描述** 从「易分叉的双描述」收口为 **单真源**；其余宿主耦合 **盘点后再减法**，不预先堆抽象。

---

## 减法 + 第一性原理（执行前默读）

| 做 | 不做 |
|----|------|
| 删字段、加断言、文档钉死权威字段 | 在无证据时大改 `dispatch` / 合并两大 hook 文件 |
| 重复逻辑可计量再抽 | trait 森林、插件 ABI、「万能宿主」 |

**选型提示（评审结论）**：维护者不止一人时，**优先删键（本仓库 P0 已选 A）或机读一致**；仅文档标注仍可能误导后续编辑 registry 的人。

---

## 背景：为何曾存在双轨

| 位置 | 现状（Round 2 后） |
|------|-------------------|
| `host_targets` 下已删除的仅 AGENTS 映射键 | **已移除**；`router-rs` 从未消费该键 |
| `host_targets.metadata.<id>.host_entrypoints` | **真源**：Codex 字符串；Cursor 数组（含 `.cursor/rules/*.mdc`） |

权威消费路径：`framework_host_targets::host_entrypoints_value_for_id` → sync manifest 等。

---

## 可执行 To-do 清单（主线程按序勾选）

### P0 — 双轨收口（必做）

- [x] **P0.1 决策**：见下文 **§P0 决策记录（已执行）**。
- [x] **P0.2-A 删键**：从 `configs/framework/RUNTIME_REGISTRY.json` 移除 `host_targets` 下第二套入口映射；更新 `tests/common/mod.rs` 等内嵌 registry；`docs/host_adapter_contract.md` 写明权威字段为 `metadata.*.host_entrypoints`。
- [x] **P0.3 验证**：`cargo test --manifest-path scripts/router-rs/Cargo.toml`；`cargo test --test policy_contracts`（根包）。

### P0.5 — 协作可见

- [x] 本文件 + `docs/README.md` 「按主题」表链接。

### P1 — 新宿主脚手架

- [x] **P1.1** `docs/host_adapter_contract.md` §3 增补路径表。
- [ ] **P1.2** `framework host scaffold --dry-run` — defer（未立项）。

### P2 — `host_integration` 减法（可选）

- [x] **P2.1 盘点**：见下文 **§P2 盘点笔记**。
- [x] **P2.2 切片**：本轮无仅数据差的安全切片 → **跳过**（见 §P2）。

### P3 — dispatch 共用层（默认关闭）

- [x] **P3.1**：第三宿主 PoC **未立项** → **已跳过**。

---

## P0 决策记录（已执行）

```
日期：2026-05-11
选项：A — 从 `RUNTIME_REGISTRY.json` 的 `host_targets` 移除「仅 AGENTS 路径映射、运行时从不消费」的旧并列键；权威入口列表仅保留 `metadata.<host>.host_entrypoints`（键名历史见主策划 `.cursor/plans/harness_host_round2.plan.md`）。

若选 B，对照规则（必填）：N/A

备注：多维护者默认倾向 A；权威字段 host_targets.metadata.<host>.host_entrypoints。
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

```bash
cargo test --manifest-path scripts/router-rs/Cargo.toml
cargo test --test policy_contracts
cargo test --manifest-path scripts/router-rs/Cargo.toml framework_host_targets
```

---

## 可见性与索引

- **主副本**：`.cursor/plans/harness_host_round2.plan.md`。
- **协作副本**：本文件 + `docs/README.md`。
- Round 1 真源：`docs/host_adapter_contract.md`、`configs/framework/RUNTIME_REGISTRY.json`。
