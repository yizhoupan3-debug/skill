# Sentence audit demo

Use this when the user asks for **逐句评估 AIGC** and wants a concrete output shape instead of generic advice.

---

## Input

> In recent years, artificial intelligence has played an increasingly important role in educational innovation. This study aims to provide a comprehensive exploration of personalized learning strategies and their broad implications for future teaching practice. The results highlight the significant potential of the proposed framework in improving learning outcomes, enhancing engagement, and facilitating more effective classroom experiences.

## Recommended output

### 1. Sentence audit table

| # | Sentence | Judgment | Signals | Why | Action |
|---|---|---|---|---|---|
| 1 | In recent years, artificial intelligence has played an increasingly important role in educational innovation. | 需要自然话改写 | boilerplate opener; significance inflation; abstract phrasing | 很像通用学术开场，信息少，谁都能套 | patch |
| 2 | This study aims to provide a comprehensive exploration of personalized learning strategies and their broad implications for future teaching practice. | 需要完全重写 | empty academic wrapper; "comprehensive"; "broad implications"; no concrete mechanism | 典型 AI 学术句，字很多，但基本没说实话 | rebuild |
| 3 | The results highlight the significant potential of the proposed framework in improving learning outcomes, enhancing engagement, and facilitating more effective classroom experiences. | 需要完全重写 | "highlight/significant potential"; 三件套并列; safe verbs; no data | 这是最重的一句，既空又整齐，很像生成式套话 | rebuild |

### 2. Overall judgment

- AIGC 痕迹主要集中在学术八股开头和结果句。
- 这段不适合只改一个词，至少要重写后两句；如果前后文也都这么写，建议整段重做。

### 3. Patched version

> We study personalized learning in one concrete setting: weekly adaptive exercises for first-year algebra classes. The framework adjusts task difficulty from student error patterns and response time. In our pilot, quiz accuracy improved by 4.1%, while completion time stayed roughly unchanged.

### 4. Why this patch works

- 删掉了 `In recent years`、`aims to provide`、`significant potential` 这些高频模板词。
- 把抽象判断改成了场景、机制和数字。
- 打破了原来三句都很“匀”的学术生成腔。

---

## Chinese demo

### Input

> 本研究具有重要的理论意义和实践价值。该方法在多个维度上取得了显著成效，展现出广阔的应用前景。未来，该研究有望为智能教育的发展提供新的思路。

### Recommended output

| # | Sentence | Judgment | Signals | Why | Action |
|---|---|---|---|---|---|
| 1 | 本研究具有重要的理论意义和实践价值。 | 需要完全重写 | 意义膨胀；空判断；无细节 | 典型空心句，几乎没有信息 | rebuild |
| 2 | 该方法在多个维度上取得了显著成效，展现出广阔的应用前景。 | 需要完全重写 | 显著成效；多个维度；广阔前景 | 套话密度很高，像摘要模板 | rebuild |
| 3 | 未来，该研究有望为智能教育的发展提供新的思路。 | 需要自然话改写 | 未来展望套话；抽象名词堆积 | 相比前两句稍轻，但还是空 | patch |

Patched version:

> 在两个公开课堂数据集上，这个方法把推荐准确率提高了 3% 到 5%。效果主要来自难度调整，而不是模型规模本身。它目前只在小班教学场景里验证过，换到大规模在线课程还需要再测。
