---
name: Harness 宿主集成 Round 2
overview: 在 Round 1 之后解决 registry 双轨与隐式约定；P0 必做，P1–P3 按条件开启；附可勾选 to-do、评审收紧项（rg 例外、B 规则模板、P2 落盘、协作可见性）。
todos:
  - id: p0-decide-path
    content: "P0 定案：A/B/C 三选一，写入 §P0 决策记录（含 B 时必填对照规则）；多维护者默认倾向 A 删键"
    status: pending
  - id: p0-impl
    content: "P0 执行：仅完成与决策一致的 P0.2-A 或 P0.2-B 或 P0.2-C 一条链，不混做"
    status: pending
  - id: p0-verify
    content: "P0 验证：router-rs 全测 + 根包集成测（见 §验证命令；test 名以 tests/*.rs 文件名为准）"
    status: pending
  - id: p0-visibility
    content: "协作可见：将本 plan 镜像到 docs/（如 docs/plans/harness_host_round2.md）并在 docs/README.md 「多宿主」表加一行链接；或确认 .cursor/plans 已纳入版本控制"
    status: pending
  - id: p1-choose
    content: "P1 二选一或 defer：host_adapter_contract 路径表，或 framework host scaffold --dry-run"
    status: pending
  - id: p2-inventory
    content: "P2 盘点 host_integration 并写入 §P2 盘点笔记；无数据驱动切片则记跳过+日期"
    status: pending
  - id: p3-gate
    content: "P3：第三宿主 PoC 未立项则勾选跳过"
    status: pending
isProject: false
---

# Harness 宿主集成 — 第二轮（Round 2）

## 北极星（一句话）

把 **`entrypoint_files` vs `metadata.host_entrypoints`** 从「易分叉的双描述」变成 **单真源或机读等价**，其余宿主耦合 **盘点后再减法**，不预先堆抽象。

## 减法 + 第一性原理（执行前默读）

| 做 | 不做 |
|----|------|
| 删字段、加断言、文档钉死权威字段 | 无第三宿主证据就大改 `dispatch` / 合并两大 hook 文件 |
| 重复逻辑可计量再抽 | trait 森林、插件 ABI、「万能宿主」 |

**选型提示（评审结论）**：维护者不止一人时，**优先 A（删键）或 B（机读一致）**；**C（仅文档）** 仍可能误导后续编辑 registry 的人。

---

## 背景：`entrypoint_files` 为何危险

| 键 | 现状 |
|----|------|
| `host_targets.entrypoint_files` | 仅 `AGENTS.md` 映射；**router-rs 不消费此键** |
| `host_targets.metadata.<id>.host_entrypoints` | **真源**：Codex 字符串；Cursor 数组（含 `.cursor/rules/*.mdc`） |

权威消费路径：`framework_host_targets::host_entrypoints_value_for_id` → sync manifest 等。`entrypoint_files` 易成第二真源。

---

## 可执行 To-do 清单（主线程按序勾选）

> 复制到 issue / 个人清单时保留 **验收** 列即可。YAML `todos` 与下文勾选应对齐；**P0.2-A/B/C 只执行一条**。

### P0 — 双轨收口（必做，三选一后执行）

- [ ] **P0.1 决策**（阻塞后续）：在 **§P0 决策记录** 写死——选 A / B / C；若选 **B**，必须同时填写 **对照规则**（见记录模板）。  
  - **验收**：本文件 §P0 中有日期 + 选项 +（B 时）规则行。

- [ ] **P0.2-A 若选「删键」**：从 [`configs/framework/RUNTIME_REGISTRY.json`](configs/framework/RUNTIME_REGISTRY.json) 移除 `host_targets.entrypoint_files`；改 [`tests/common/mod.rs`](tests/common/mod.rs) 等内嵌 registry；[`docs/host_adapter_contract.md`](docs/host_adapter_contract.md) 写明「策略入口只认 `metadata.*.host_entrypoints`」。  
  - **验收（rg，已收紧）**：`rg "entrypoint_files" configs/framework tests` 在 **RUNTIME_REGISTRY / fixture 中无键**；`docs/` 侧允许：**(1)** [`docs/history/`](docs/history/) 归档提及；**(2)** `host_adapter_contract.md` **至多一段**「历史字段已删除 / 勿再添加」说明（避免全库零命中与解释力矛盾）。

- [ ] **P0.2-B 若选「机读一致」**：在 `tests/policy_contracts.rs` 或 `router-rs` 单测中解析 registry，按 **§P0 已写死的对照规则** 断言每一 `supported` id 两侧一致。  
  - **验收**：故意只改 `entrypoint_files` 或只改 `metadata` 一侧时测试**红**；全量 `cargo test` **绿**。  
  - **规则示例（须在 P0.1 钉死，勿实现时再拍脑袋）**：例如「`entrypoint_files[id]` 必须等于 `metadata[id].host_entrypoints` 的**首个**字符串元素（`host_entrypoints` 为 string 时即自身）」——若 cursor 需与单字符串不等价，则 **B 不适用，改选 A 或 C**。

- [ ] **P0.2-C 若选「仅文档」**（最低）：`host_adapter_contract.md` 醒目标注：`entrypoint_files` **非 router 消费**；权威 **`metadata.host_entrypoints`**；建议后续删键。  
  - **验收**：评审 30 秒内可定位段落。

- [ ] **P0.3 验证**：见 **§验证命令**。

### P0.5 — 协作可见（与 P0 同轮推荐完成）

- [ ] 将本文件 **镜像**到 `docs/plans/harness_host_round2.md`（或团队约定的 `docs/` 路径），并在 [`docs/README.md`](docs/README.md) 「多宿主」表加一行链接；**或** 确认 `.cursor/plans/` **未被** `.gitignore` 且已提交。  
  - **验收**：协作者在默认 clone 下能打开同一份 Round 2 正文。

### P1 — 新宿主脚手架（可选，可与 P0 并行）

- [ ] **P1.1** [`docs/host_adapter_contract.md`](docs/host_adapter_contract.md) §3 增补 **路径表**：`cursor_hooks/`（`mod.rs`、`dispatch.rs`、`frag_*.rs`）、`codex_hooks.rs`、`host_integration.rs`、`cli/dispatch`、`hooks.json`、[`RUNTIME_REGISTRY.json`](configs/framework/RUNTIME_REGISTRY.json) `metadata`。  
- [ ] **P1.2（可选）** `framework host scaffold --dry-run` —— defer 则写日期于本文件备注。

### P2 — `host_integration` 减法（可选）

- [ ] **P2.1 盘点**：读 [`scripts/router-rs/src/host_integration.rs`](scripts/router-rs/src/host_integration.rs)，列 3～5 条「仅路径/模板差异」分支，写入 **§P2 盘点笔记**（附日期）。  
- [ ] **P2.2 切片**：仅当存在 **可迁入 RUNTIME_REGISTRY 的纯数据差** 才改代码；否则在 §P2 写 **「本轮跳过」+ 日期**。  
  - **验收**：有测试；行为无静默变化。

### P3 — dispatch 共用层（默认关闭）

- [ ] **P3.1**：第三宿主 PoC **未立项** → 勾选「已跳过」；立项后另开 issue。

---

## P0 决策记录（执行 P0.1 时填写）

```
日期：
选项：A 删 entrypoint_files / B 机读校验双轨 / C 仅文档

若选 B，对照规则（必填，实现与单测仅允许引用本节）：
  例如：entrypoint_files[id] == <从 metadata[id].host_entrypoints 推导的标量，规则：……>

备注：
```

---

## P2 盘点笔记（执行 P2.1 时填写）

```
日期：
候选重复分支（3～5 条）：
1.
2.
P2.2 结论：切片 / 本轮跳过（+ 日期）：
```

---

## Non-goals

- N 宿主 trait 体系、运行时插件、合并 Cursor/Codex hook 语义层。
- 扩大 `RUNTIME_REGISTRY` 去承载业务 skill 正文。

---

## 验证命令（每次合并前）

根目录包名为 `skill-rust-test-harness`；集成测文件名 `tests/policy_contracts.rs` 对应 **`cargo test --test policy_contracts`**（若将来拆分/改名，以根 [`Cargo.toml`](Cargo.toml) 与 `tests/` 实际文件名为准）。

```bash
cargo test --manifest-path scripts/router-rs/Cargo.toml
cargo test --test policy_contracts
cargo test --manifest-path scripts/router-rs/Cargo.toml framework_host_targets
```

---

## 可见性与索引

- **主副本**：`.cursor/plans/harness_host_round2.plan.md`（Cursor 任务 YAML 建议保留于此）。  
- **协作副本**：见 **P0.5**（`docs/` 镜像 + `docs/README.md` 链接）。  
- Round 1 真源：[`docs/host_adapter_contract.md`](docs/host_adapter_contract.md)、[`configs/framework/RUNTIME_REGISTRY.json`](configs/framework/RUNTIME_REGISTRY.json)。
