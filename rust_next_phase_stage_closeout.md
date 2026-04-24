# Rust 化下一阶段完成总结

> 本文件用于记录一个**假设前提已经成立**的阶段收口版本：
> `rust_next_phase_checklist.md` 中定义的本轮 next phase 已全部完成，
> 且相关定点验证、文档刷新、生成物刷新都已经收口。
>
> 它不是下一轮 checklist，也不是回顾式流水账，而是这一阶段结束后可对外复用的
> 阶段状态说明。

## 1. 本阶段结论

本阶段结束后，仓库已经从“Rust-first 迁移中”推进到“Rust contract-first 主线基本收口”：

- 默认 live/runtime/control-plane authority 稳定保持 Rust-first
- Python 明确退回 thin projection / compatibility host 角色
- route / execute / runtime control-plane / attach-replay 这些高价值主链路已不再依赖
  Python 作为日常语义真源
- compatibility / fallback surface 被压回显式 lane，不再影响默认 steady-state truth
- docs、generated artifacts、targeted verification 的口径已与当前实现对齐

这意味着后续工作不再是“继续把主链路从 Python 搬到 Rust”，而是：

- 继续做更深的 backend-family / transport / persistence 能力增强
- 继续删减兼容残留
- 或者在不破坏当前边界的前提下做下一轮 runtime 能力扩展

---

## 2. 本阶段真正完成了什么

### 2.1 Route consumer 已完成 typed-first 收口

- route CLI 与相邻 helper 默认围绕 Rust-owned typed contract 工作
- raw JSON 已退回 transport 边界职责，不再由 Python 自行补语义
- route decision / match result / diff report 的共享 schema 已稳定

### 2.2 Execution kernel metadata 已完成 canonicalization

- live execution 的主通路稳定以 Rust 为 canonical producer
- Python 不再补写 steady-state kernel metadata
- live primary / dry-run / retired compatibility 三条 response shape 的 contract
  已统一冻结

### 2.3 Workspace bootstrap / shared contract parity 已锁稳

- `workspace_bootstrap` 的 Python / Rust 双实现已对齐到同一份 shared truth
- framework truth 与 host projection 的边界已更清楚
- host adapter 不再借口“方便集成”把 host-private 字段倒灌回 framework truth

### 2.4 Process-external attach surface 已完成硬化

- attach descriptor / replay / cleanup / resume 已共享同一条 descriptor contract
- runtime 与 consumer 不再各自补半套协议
- process-external recovery 路径的 fail-closed 行为已补齐

### 2.5 Integrator / regenerate / verify 已完成本轮收口

- 本轮真正变更到的 docs / generated outputs / evidence 都已刷新
- 没变的 surface 没有被无意义重刷
- 最终 closeout 口径已经固定，可作为下一阶段的起点

---

## 3. 本阶段结束后的系统状态

可以把当前状态理解成三句话：

1. **主链路已经 Rust-owned。**
2. **Python 还在，但主要承担 projection / compatibility / host glue。**
3. **后续再推进，应该是做能力深化，不是重开主权之争。**

更具体一点：

- route authority：Rust
- execution authority：Rust
- runtime control-plane authority：Rust
- steady-state fallback：已退休，不再是默认 runtime 能力
- memory extraction / prompt compression policy：Rust
- memory policy persistence：Rust writes SQLite rows and the stable `decisions.md`
  journal; Python must not add a second writer
- Python host role：thin projection / compatibility host
- host adapters：继续是 shared contract 的消费方，不再是 framework truth 的私造源头

---

## 4. 这一阶段冻结下来的边界

以下边界在本阶段结束后应视为**冻结约束**，后续不得随手打破：

- 不回退到 Python-first route / execute / runtime control plane
- 不重新开放默认 live Python fallback
- 不把 compatibility surface 再抬回 default peer set
- 不把 host-private semantics 回写成 framework truth
- 不把“测试方便”当成恢复双真源的理由
- 不把 runtime kernel rewrite 和普通 contract 收口混成一个大爆炸工程

这些边界的意义是：保证未来所有增量工作都站在已收口的 Rust-first 地基上，而不是
反复横跳。

---

## 5. 现在达到的效果

如果用最朴素的人话来讲，这一阶段做完后的效果是：

- 日常真正重要的链路，已经不是 Python 在底下偷偷兜底
- Rust 不只是“多了一份实现”，而是已经成了主通路和主口径
- Python 这边更多是在接宿主、做兼容、做薄投影
- 以后再查问题、补 contract、看 health、接 consumer，优先看的都是 Rust 那条线

也就是说，这个仓库已经从“Rust 化进行中”切到了“Rust 主线已站稳，后面进入深化期”。

---

## 6. 本阶段不声称完成的事

为了避免口径过满，本阶段虽然完成，但**不代表下面这些事情已经全部做完**：

- 远程 / 分布式 event transport 已 fully finished
- checkpoint / compaction / snapshot-delta 的 backend family 已 fully expanded
- sandbox lifecycle 已达到最终形态
- 所有 Python compatibility code 都已经物理删除
- live in-process Rust kernel 已演化到最终架构终点
- 所有 host-private 展示文案都已经并入 shared Rust truth

本阶段完成，代表的是：

- 高价值主链路已经 Rust-first 收口
- 关键 contract 已冻结
- 继续演进时不需要再回头争夺默认 authority

---

## 7. 验收完成后的推荐口径

对内可以这样描述这一阶段：

> Rust next phase 已经完成。主链路 authority 已稳定收口到 Rust，
> Python 已退回 thin projection / compatibility host；后续工作进入能力深化与
> 兼容清理阶段，不再属于主链路 Rust ownership 争夺。

对外或对未来 handoff 可以这样描述：

> 当前系统已经完成高价值 Rust-first 收口。下一阶段若继续推进，应围绕
> persistence backend、remote transport、sandbox lifecycle、compatibility 删除、
> 以及更深的 runtime 能力增强展开，而不是回头重做 route/execute/control-plane
> 主权切换。

---

## 8. 下一阶段建议

如果把这一阶段视为已经结束，下一阶段建议不再按“再找一批 Python 残影继续 Rust 化”
来组织，而改成以下几类：

### A. Runtime 能力深化

- remote / host-bound transport boundary
- stronger persistence backend family
- compaction / snapshot-delta / replay efficiency
- sandbox lifecycle 与调度控制面继续落地

### B. Compatibility 清理

- 删除已经完全退休且只剩历史噪音价值的 compatibility path
- 压缩 legacy docs / tests / inventory 中仍然保留的过渡叙事
- 让 compatibility lane 更显式、更边缘，而不是继续占据认知空间

### C. 消费侧扩展

- 让更多 host / consumer 直接吃 Rust-owned contract
- 减少 Python-only helper surface
- 强化 typed contract 在外部消费端的稳定性

---

## 9. 一句话收口

这一阶段完成之后，Rust 在这个仓库里已经不是“迁移目标”，而是“默认主线”。
