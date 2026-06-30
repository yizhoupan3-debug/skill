# Discovery Lane — 文献发现、理论调研、背景分析

本 lane 是 `$research` 统一前门的发现子模式。处理文献调研、理论背景、数学背景询问、知识图谱和选题定界。

## 使用场景

- 用户有具体的调研问题：文献综述、相关定理、未知性质
- 需要 candidate theorems 识别（`applies_when` / `fails_when`，**不许**写推导路线图）
- 需要 dataset 识别与编目（有哪些、各自属性）
- 需要 external research 检索（arXiv / OpenAlex / CrossRef / PubMed / DOAJ）
- 用户从 `$research` 统一前门接收到此 lane

## 不要使用

- 用户要做实验设计/ablation/benchmark → route to `$research` execution lane
- 用户要做数学建模（控制方程/量纲分析）→ route to `$research` execution lane
- 用户有手稿对象 → route to `$research` paper-workbench lane
- 用户有模糊兴趣但无具体问题 → 先走 `$good-question`
- 用户需求不清 → 先走 `$deepinterview`

## Lane routing（对应原 research-discovery 的 lanes）

| Lane | 说明 | 输出 |
|------|------|------|
| `research_question` | 研究目标/决策定界 | 问题卡片 |
| `external_research` | 学术文献检索 | retrieval_trace + 证据地图 |
| `math_background_inquiry` | 候选定理识别（定理级别，无推导路线图） | theorem_list × applies_when/fails_when |
| `paper_handoff` | 发现工作完成后递交给手稿 lane | handoff payload |

## 边界（与 execution lane 的数学分界线）

**P1 级别（最容易误用）**：

| 阶段 | Owner | 产出 | 限制 |
|------|-------|------|------|
| 候选定理识别 | discovery lane | candidate theorem list × applies_when/fails_when | **不许写推导路线图**、不许声称可证 |
| 推导策略 | execution lane | proof_strategy_hints + 定理依赖链 | 在 discovery 给出 candidates 后产出 |
| 推导执行 | `$math-derivation` (L4) | formal proof / 符号推导 | — |
| 验证 | `$formal-verification` (退出门) | Z3/SymPy/Lean 验证结果 | — |

## Lane handoffs（来自统一前门协议）

- discovery → execution：传递 `claims_to_verify` + `retrieval_trace` + `evidence_gaps`
- discovery → paper-workbench：传递 `language_register` + `theory_list`
- Loop-back 输入：execution lane 可在发现未知未知时 loop-back 到此 lane

## 验证合约

- 没有外部检索（当需要且允许时）不得将"深度研究"变成无来源猜测
- 数学背景工作必须产出 witness list（定理级）

## 相关资源

- External research 来源：详见 [`../references/academic-sources.md`](../references/academic-sources.md)
- 链路协议：详见 [`../references/research-lane-routing.md`](../references/research-lane-routing.md) §Math 分界线
- Quality gates（退出门）：
  - `external_research` / `literature_survey` → `$literature-verification`
  - `math_background_inquiry` → `$formal-verification`
- 下游工具：`$citation-management`（L3，引用格式校验）/ `$deep-search`（L3，通用搜索）
