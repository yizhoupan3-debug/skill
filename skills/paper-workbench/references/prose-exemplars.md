# Prose exemplars（坏→好对照，分语言场景）

**用法**：改稿时对照结构，**禁止抄句式**；用用户稿件的事实替换占位符。  
**门控**：配合 [`prose-quality-gate.md`](prose-quality-gate.md) 的 `language_register`。

---

## English (`en_submission`)

### Abstract — empty opener

**Weak**

```text
In recent years, machine learning has attracted widespread attention. However, existing methods still have limitations. Therefore, we propose a novel framework that leverages deep learning to achieve significant improvements.
```

**Stronger**（具体 gap + move + scoped result）

```text
Training [model class] on [setting] fails when [specific condition] because [mechanism]. We introduce [method] to [concrete move] and evaluate it on [datasets/tasks]. On [metric], [method] improves over [strong baseline] by [value] under [scope]. These results suggest [bounded implication].
```

### Introduction — literature dump

**Weak**

```text
Smith et al. (2020) proposed A. Jones et al. (2021) proposed B. Lee et al. (2022) proposed C. However, none of them solves our problem.
```

**Stronger**（聚类 + 边界 + 后果）

```text
Prior work on [subproblem] clusters into [theme 1] and [theme 2]. Methods in [theme 1] assume [assumption], which breaks when [your setting]. Approaches in [theme 2] improve [nearby metric] but leave [specific gap] because [reason]. This gap matters for [consequence]. We address it by [paper move].
```

### Results — table narration

**Weak**

```text
Table 2 shows the results. We can see that our method is better. Table 3 shows another result. Our method is also better here.
```

**Stronger**（claim-led + 一句 takeaway）

```text
On [task], [method] reduces [metric] by [Δ] relative to [baseline] across [N] seeds (Table 2). The gain concentrates in [regime], consistent with [mechanism]. On [second task], [method] matches [baseline] on [metric A] while improving [metric B] (Table 3), indicating [bounded interpretation].
```

### Sentence — nominalization / emphasis

**Weak**

```text
We performed an analysis of the failure modes and observed that there was an improvement of 15% in accuracy when the attention module was utilized.
```

**Stronger**

```text
We analyzed failure modes and found that adding attention improves accuracy by **15%** on [datasets] under [protocol].
```

---

## 中文 (`zh_manuscript`)

### 摘要 — 套话开场

**弱**

```text
近年来，随着人工智能的快速发展，深度学习在诸多领域得到了广泛应用。然而，现有方法仍存在一定不足。本文提出了一种新颖的方法，取得了显著效果。
```

**较强**（问题具体、结果可核对）

```text
在[具体场景]下，现有[方法类]因[机制原因]难以满足[可观测需求]。本文提出[方法名/核心做法]，在[数据/任务]上将[指标]从[基线水平]提升至[结果水平]（[不确定性/范围]）。该结果说明在[明确范围]内[有边界的结论]。
```

### 引言 — 文献罗列

**弱**

```text
张三等(2020)提出了方法A。李四等(2021)提出了方法B。王五等(2022)提出了方法C。然而，上述工作均存在不足。
```

**较强**

```text
针对[子问题]，已有工作大致分为[路线一]与[路线二]。[路线一]依赖[关键假设]，在[你的设定]下失效；[路线二]虽在[相近指标]上有效，但未解决[具体缺口]，原因在于[机制]。该缺口直接影响[后果]。本文通过[核心做法]填补这一空白。
```

### 讨论 — 防御堆叠

**弱**

```text
需要指出的是，本文并非声称解决了所有问题。我们并不认为该方法在所有情况下都优于现有方法。然而，在某种意义上，结果仍具有一定参考价值。
```

**较强**（正面主句 + 一句边界）

```text
在[实验范围]内，[方法]相对[最强基线]稳定提升[指标]，支持「[核心主张]」。[局限]：未在[未覆盖设定]上验证；[下一步]需在[条件]下补充[对照/分析]。
```

### 段落 — 缺主题句

**弱**

```text
实验采用 Adam 优化器，学习率 1e-4，batch size 32。训练 100 epoch。验证集上准确率为 92.3%。
```

**较强**

```text
在[任务]上，[方法]达到 92.3% 准确率，较[基线]高 [Δ]（表 1）。实现上采用 Adam（lr=1e-4，batch=32），训练 100 epoch；完整协议见§3。
```

---

## Mixed (`mixed`)

- **中文正文**：只用 §中文 范例的段落节奏与套话禁令。  
- **英文 Abstract / Figure caption**：只用 §English 范例；图注须自洽（变量、队列、一句 takeaway），见 [`section-by-section.md`](section-by-section.md)。

---

## 维护说明

新增范例时：**一对 weak/stronger + 一句「改了什么」**（gap 具体化 / topic 句 / 句末 emphasis / 删套话），勿堆更多抽象规则。
