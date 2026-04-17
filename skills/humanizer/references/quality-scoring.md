# Humanization Quality Scoring System

Use this 100-point rubric to explicitly grade long rewrites before delivery.
For short snippets, use this mentally. For documents >300 words, show the score calculation explicitly.

---

## 质量评估维度 (Quality Dimensions)

| Dimension评分维度 | 依据说明 | 分值 /20 |
|---|---|---|
| **信息密度 (Information Density)** | 没有因为删减套话而丢失核心事实信息。具体名词、数据、引用的留存率高。 | |
| **检测脆弱度 (Detection Vulnerability)** | 句首是否存在长串连接词堆砌？句子长度是否呈现教科书般的均匀分布（低 burstiness）？是否存在被穷举的高危词汇？ | |
| **学术/语境适配 (Register Adequacy)** | 学术场景下是否做到了克制的方法论描述（过 Turnitin）？商业场景下痛点是否足够具象且具代入感？ | |
| **可读性与留白 (Reader Trust)** | 结论和过渡自然后文逻辑得出，移除了过度解释和"众所周知"式的自作主张。尊重读者的智商。 | |
| **生动性与质感 (Authenticity & Soul)** | 阅读起来有明显的人类观点倾向甚至一点合理的主观态度/自我纠偏（视具体语境而定），而非平铺直叙的维基百科。 | |

---

## 阶段门槛 (Gate Criteria)

- **≥ 85 分**：Deliver immediately.
- **70–84 分**：Targeted Revision. Identify the lowest-scoring dimension and do one focused pass to fix it.
- **< 70 分**：Reject and Restart. The approach failed. Re-read the source and try a completely different structural angle.

---

## 防假性收敛 (Anti-Pseudo-Convergence)

When grading, be careful of "pseudo-convergence" (假性收敛) — a state where the text *looks* clean because obvious filler words were removed, but the underlying AI *structure* remains intact.

**If the text scores high but still feels robotic, check for:**
1. **The "Listicle in Disguise"**: Bullet points were converted to paragraphs, but they still read like a disconnected list.
2. **Predictable Topic Sentences**: Every paragraph accurately states its point in the first sentence, provides exactly two sentences of support, and ends.
3. **Absence of Negative Space**: Humans leave things unsaid. AI fills every logical gap. If the text explains absolutely everything, re-score **Information Density** and **Reader Trust** lower.
