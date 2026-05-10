# 科研 / 手稿技能栈索引（真源地图）

面向维护者与模型实现方：**先读本节再下钻**，避免在同一任务里平行打开过多 reference 导致「合规压过行动」。

## 第一性原理（减法一句话）

**用户要**：稿子能不能投、差什么证据、下一步改哪 —— **不是**技能拓扑考试。
**默认入口一个**：`$paper-workbench`；其余专科为**内部分片**（`disable-model-invocation`），除非用户点名。

## 技能层（谁拥有什么）

| 角色 | Skill | 何时加载 |
|------|--------|----------|
| 前门 / 编排 | `$paper-workbench` | 任何手稿级模糊请求；ref-first；先审再改；workflow 抱怨 |
| 判断 | `$paper-reviewer` | 审稿、严审、能不能投、单维度审；**不改稿**除非用户切换 |
| 执行 | `$paper-reviser` | findings/审稿意见已就绪、要动稿；遵守 `edit_scope` |
| 表达 | `$paper-writing` | claim 边界已冻结下的局部/授权范围内润色与叙事 |
| 引用真源 | `$citation-management` | `.bib`、参考文献表、DOI/格式、文后一致性 |

**邻接技能**（按artifact 并入，不替代前门）：`statistical-analysis`（统计深度）、`experiment-reproducibility`（可复现）、`scientific-figure-plotting` / `tikz-paper-figure`（作图代码）、`visual-review` / `pdf`（成稿视觉与版式）、`math-derivation`（推导，窄）。

## Reference 下钻顺序（渐进披露）

| 层级 | 何时需要 | 读什么 |
|------|----------|--------|
| **L0** | 每一轮手稿任务 | [`../SKILL.md`](../SKILL.md) 正文 + **Progressive disclosure** |
| **L1** | 即将改稿 | [`edit-scope-gate.md`](edit-scope-gate.md)（`surgical` 默认） |
| **L1** | 证据与主张不匹配 | [`claim-evidence-ladder.md`](claim-evidence-ladder.md)（先补证据再缩口径） |
| **L1** | 落笔 / 语言问题 | [`research-language-norms.md`](research-language-norms.md) |
| **L2** | 顶刊/顶会栏 | [`top-tier-paper-standard.md`](top-tier-paper-standard.md) |
| **L2** | 先学 ref 再写 | [`ref-first-writing-workflow.md`](ref-first-writing-workflow.md) |
| **L2** | Lane 语义速查 | [`paper-lanes.md`](paper-lanes.md) |
| **L2** | 用户话术 → lane 最小对照 | [`user-phrases-to-lanes.md`](user-phrases-to-lanes.md) |
| **L3** | 多轮冻结、并行 sidecar、磁盘门控 | [`../../PAPER_GATE_PROTOCOL.md`](../../PAPER_GATE_PROTOCOL.md) |

**纪律**：单轮交互**不要**默认展开 L3；用户要持久化状态或并行 batch 时再打开协议。

## 横切规则（避免重复真源）

- **编辑范围**：只信 [`edit-scope-gate.md`](edit-scope-gate.md)。
- **主张 vs 证据**：只信 [`claim-evidence-ladder.md`](claim-evidence-ladder.md)。
- **用语 / 内部口径 / 防御式堆叠**：只信 [`research-language-norms.md`](research-language-norms.md)。
- **RFV（代码）vs PAPER_GATE（手稿）**：协议首段 —— 勿混 PASS 语义。

## 已收敛的断裂

- **不存在** `skills/literature-synthesis/`：目标期刊语料与 `ref_learning_brief` 在 **`$paper-workbench` 内**完成，见 [`ref-first-writing-workflow.md`](ref-first-writing-workflow.md)。
- `literature-synthesis` slug 在仓库政策中视为 **retired**（勿复活目录）；编译器侧勿再当作 runtime workflow slug。

## 可选工具

- 并行 lane：本仓库不提供必需的 scaffold 脚本；需要时按 `PAPER_GATE_PROTOCOL` 的
  Lane Manifest Contract 手工创建 `lane_manifest.md` 与各 lane 子目录。
