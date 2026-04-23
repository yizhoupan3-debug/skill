# Humanizer Before/After Examples

Use these examples to keep the skill grounded in **style naturalization**, not detector evasion.

---

## English Examples

### Example 1: generic professional prose

**Before:**
> Our company is committed to delivering innovative solutions that empower teams, streamline collaboration, and drive meaningful business outcomes in a rapidly evolving landscape.

**After:**
> Our company builds tools that help teams work together with less friction. The focus is practical: fewer handoff delays, clearer coordination, and faster delivery.

**Patterns fixed:** promotional language (#4), rule of three (#10), AI vocabulary (#7 — "innovative", "empower", "streamline", "evolving landscape")

---

### Example 2: chatbot residue

**Before:**
> Great question. Here is a polished version of your paragraph. I hope this helps. It is worth noting that the policy plays a crucial role in improving long-term efficiency.

**After:**
> The policy matters because it reduces repeated manual work and makes long-term maintenance easier.

**Patterns fixed:** chatbot artifacts (#19), filler phrases (#22 — "It is worth noting"), AI vocabulary (#7 — "crucial role")

---

### Example 3: academic-adjacent significance inflation

**Before:**
> This study provides a comprehensive and insightful exploration of the issue, highlighting its significance in modern research and underscoring its broad implications.

**After:**
> This study examines the issue in a defined research setting and explains why the result matters for that setting.

**Patterns fixed:** significance inflation (#1), -ing analysis (#3 — "highlighting", "underscoring"), AI vocabulary (#7 — "comprehensive")

---

### Example 4: founder/personal voice

**Before:**
> Our platform leverages cutting-edge artificial intelligence to deliver seamless experiences. We are committed to innovation and believe in empowering users through technology. The future is bright.

**After:**
> We built the search feature because our support team was drowning — 200 tickets a week, most asking the same five questions. Six months later, ticket volume dropped 40%. The AI part? It's a keyword matcher with some ranking. Not glamorous, but it works.

**Patterns fixed:** promotional language (#4), AI vocabulary (#7 — "leverages", "seamless", "cutting-edge"), generic positive conclusion (#24), sycophantic tone (#21). Applied: first person, specific numbers, honest assessment.

---

### Example 5: technical documentation

**Before:**
> This comprehensive API provides a robust and scalable solution for seamless integration with third-party services, offering a wide range of endpoints that facilitate efficient data exchange and enhance overall system performance.

**After:**
> The API has 12 endpoints for auth, payments, and webhooks. It handles 5K req/s on a single instance. Rate limiting is built in. See the endpoint reference for specifics.

**Patterns fixed:** promotional language (#4 — "comprehensive", "robust", "scalable", "seamless"), AI vocabulary (#7 — "facilitate", "enhance"), vague claims → specific numbers.

---

### Example 6: email with excessive hedging

**Before:**
> I hope this email finds you well. I wanted to reach out regarding the upcoming project deadline. It is worth noting that there are several important considerations that we should potentially discuss at your earliest convenience. I would be more than happy to schedule a meeting.

**After:**
> The project deadline is Friday. Two things need decisions before then: the API scope and the testing timeline. Can we meet tomorrow at 2pm?

**Patterns fixed:** chatbot artifacts (#19 — "I hope this email finds you well"), filler (#22 — "It is worth noting"), excessive hedging (#23 — "potentially", "at your earliest convenience").

---

## 中文示例

### 示例 7: 中文职场表达

**Before:**
> 本项目致力于打造创新性的智能协作方案，全面赋能团队高效协同，进一步推动组织在快速变化的环境中实现高质量发展。

**After:**
> 这个项目主要解决团队协作里的几个具体问题：信息传递慢、交接成本高、责任边界不清。目标不是"全面赋能"，而是把协作过程里最容易卡住的环节先做顺。

**修复的模式:** 推销语言"打造"、"赋能"、"全面"；意义膨胀"高质量发展"；空泛目标 → 具体问题

---

### 示例 8: 中文聊天味残留

**Before:**
> 这是一个非常好的问题。下面我将为你详细介绍这个方案，希望对你有所帮助。这个方案在当前背景下具有非常重要的意义。

**After:**
> 这个方案的重点有两点：一是降低重复操作，二是让责任分工更清楚。真正重要的不是"意义重大"，而是它能不能解决当前流程里的具体卡点。

**修复的模式:** 助手残留"这是一个非常好的问题"、"下面我将为你详细介绍"、"希望对你有所帮助"；意义膨胀"具有非常重要的意义"

---

### 示例 9: 中文学术论文

**Before:**
> 本研究具有重要的理论意义和实践价值。研究结果表明该方法在多个维度上取得了显著成效，填补了该领域的研究空白，为未来研究提供了新的思路，展现了广阔的应用前景。

**After:**
> 本研究在三个公开数据集上验证了方法的有效性：准确率比基线高 3–5%，推理速度持平。此前仅有 Zhang et al. (2021) 在单一数据集上做过类似尝试，本文将其推广到了跨域场景。
>
> 局限性也很明确：在小样本条件下（<500 条），性能下降约 8%，需要进一步优化采样策略。

**修复的模式:** 意义膨胀"重要的理论意义和实践价值"；空泛判断"显著成效"、"填补空白"、"广阔前景" → 具体数据。增加了局限性承认（自然口吻：承认复杂性）

---

### 示例 10: 中文个人陈述

**Before:**
> 我对计算机科学有着浓厚的兴趣和坚定的热情。通过四年的学习，我培养了扎实的编程能力、良好的团队合作精神和出色的解决问题的能力。我坚信人工智能将深刻改变世界，我渴望在这一领域贡献自己的力量。

**After:**
> 大一时我不喜欢编程课。转折点是大二那门操作系统课——我花了两个周末写 shell，debug 到凌晨三点，但第一次觉得这件事有意思了。
>
> 后来做毕设选了 NLP 方向，原因很直接：导师手上有标注好的医疗数据，我可以直接上手跑实验。做了半年之后发现，最难的不是模型调参，而是理解医生到底想从文本里提取什么信息。这个经历让我意识到，技术问题的核心往往不在技术本身。

**修复的模式:** 八股结构"浓厚的兴趣和坚定的热情"、"培养了…的能力"三连、"坚信…将改变世界" → 具体经历。应用自然口吻：第一人称叙事、允许否定（"不喜欢"）、具体场景和转折

---

### 示例 11: 中文社交帖子

**Before:**
> 近日，我有幸参加了一场关于人工智能前沿技术的高端论坛。与多位行业领袖和技术专家进行了深入交流和思想碰撞，收获颇丰。此次论坛对于推动行业发展具有重要意义。

**After:**
> 昨天去了个 AI 论坛。大部分演讲跟去年差不多，但有一个做端侧推理的团队讲得好——他们把 7B 模型量化到手机上跑，延迟只有 200ms。会后跟他们聊了聊，打算下周试试他们的 SDK。

**修复的模式:** 推销语言"高端论坛"、"行业领袖"、"技术专家"；模糊归因"深入交流和思想碰撞"；意义膨胀"重要意义" → 具体见闻和行动计划
