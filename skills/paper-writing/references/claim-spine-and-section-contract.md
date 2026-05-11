# Claim spine & section contract（叙事骨架与章节契约）

用于 **`$paper-writing`** / **`$paper-reviser`** / **`$paper-workbench`** 改稿前对齐：**主张一张卡** + **各主章节读者任务**，避免「段落顺了但贡献线断了」或静默抬口径。

## Claim card（改前 Step 0，一轮一张）

在动正文前先写出可见 Claim card（可与 `claim_ledger` 对齐；多轮时同步 delta）：

```text
headline_contribution: <一句话：本篇唯一可被记住的贡献动作（动词 + 对象 + 边界）>
decisive_evidence_unit: <最强关停证据单元：Fig./Table./Theorem/主结果句 + 编号或锚点>
closest_work_gap: <最近邻工作是谁、差在哪、为何仍值得占版面>
venue_slot: <目标期刊/会议轨道或体裁：读者默认契约 + 页数/披露约束一句话>
```

**纪律**：任意改写不得让正文表述超过 `headline_contribution` 与 `decisive_evidence_unit` 所允许的力度；若用户要求「写得更响」而证据未升级，停并回到 reviewer/reviser 决策链（见 [`../../paper-workbench/references/claim-evidence-ladder.md`](../../paper-workbench/references/claim-evidence-ladder.md)）。

## Section contract（IMRaD-ish 主章节）

下列契约描述**每节必须完成的读者任务**与**交给下一节的 handoff**（不必覆盖附录、补充材料全文）。

| Section | Reader job | Must deliver | Handoff to next |
|--------|------------|--------------|-----------------|
| **Title + Abstract** | 快速判定「做什么、为何重要、证据在哪」 | 贡献动词清晰；结果边界与范围可读；无超出证据的最高级 | Intro：展开 bottleneck 与 gap |
| **Introduction** | 建立 field tension → 具体瓶颈 → 本文 move | `core_problem → bottleneck → paper_move` 可读；closest-work 缝隙指向 Related / Methods | Related：对照坐标系 |
| **Related work** | 聚类对照，钉死「最近邻 + 为何仍不够」 | 簇末有一句落到 `closest_work_gap`；禁止文献清单无 contrast | Methods：读者接受「我们怎么做」的前提 |
| **Methods / Setup** | 可复核：谁能复现、测什么、对照公平性 | 与正文图表编号、`decisive_evidence_unit` 对齐；符号与指标首次可操作定义 | Results：证据顺序已预设 |
| **Results** | 按贡献链呈现证据，而非按实验顺序流水账 | 图表Self-contained；每条主结论能指回 claim card | Discussion：解释机制/局限/含义 |
| **Discussion / Limitations** | 诚实边界 + 为何仍成立 | limitation **保护** claim 而非削弱可信度堆砌；无防御式叠句顶替证据缺口 | Conclusion（若有）：收敛一句贡献 |
| **Conclusion**（若分立） | 重复贡献与边界，不引入新主张 | 与 abstract / intro **同一 move**，仅压缩 | Rebuttal / cover letter：逐条映射（若适用） |

多节 **`refactor`** 时：先交 **section-level outline**（每节 3–7 条 bullet，含上表 Handoff），再落 prose。

## 与其它真源的衔接

- **故事线与诊断**：[`storytelling-patterns.md`](storytelling-patterns.md)（spine、常见病、ref-guided 流程）。
- **顶刊六层栏与自检问题**：[`../../paper-workbench/references/top-tier-paper-standard.md`](../../paper-workbench/references/top-tier-paper-standard.md)（贡献 / closest-work / 证据关停先于辞藻）。
- **主张—证据阶梯与审稿关停**：[`../../paper-workbench/references/claim-evidence-ladder.md`](../../paper-workbench/references/claim-evidence-ladder.md)（含 R&R、代码/数学质疑）。
- **交给审稿维度的用语与防御腔红线**：[`../../paper-workbench/references/research-language-norms.md`](../../paper-workbench/references/research-language-norms.md) §3；审稿侧全文核对亦可对照同文件 §6。

Reviewer-facing handoff：若本节产出将进入 **`$paper-reviewer`** 或 response 映射，Claim card 中的 `decisive_evidence_unit` 与 `closest_work_gap` 应能 **逐条**对应 Major/Minor 中的证据与对照质疑，避免「故事改了、关停件丢了」。
