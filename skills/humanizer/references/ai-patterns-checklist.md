# AI Writing Patterns Checklist

A comprehensive catalog of AI writing patterns to detect and fix during humanization.
Based on [Wikipedia: Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing) and community best practices.

---

## CONTENT PATTERNS

### 1. Undue emphasis on significance / 意义膨胀

**Watch words:** stands/serves as, is a testament/reminder, vital/significant/crucial/pivotal/key role/moment, underscores/highlights importance, reflects broader, symbolizing ongoing/enduring/lasting, setting the stage for, represents/marks a shift, evolving landscape, indelible mark

**中文版高频词:** "具有重要意义"、"标志着…的转折"、"深远影响"、"不可磨灭的印记"、"关键节点"、"里程碑式的"

**Before:**
> The initiative was part of a broader movement, marking a pivotal moment in the evolution of regional governance.

**After:**
> The initiative gave the region its own statistics office, independent from the national one.

---

### 2. Undue emphasis on notability / 知名度注水

**Watch words:** independent coverage, local/regional/national media outlets, written by a leading expert, active social media presence

**中文版:** "引起广泛关注"、"获得业界一致好评"、"被多家权威媒体报道"

**Before:**
> Her views have been cited in The New York Times, BBC, and Financial Times. She maintains an active social media presence with 500K followers.

**After:**
> In a 2024 New York Times interview, she argued that AI regulation should focus on outcomes rather than methods.

---

### 3. Superficial -ing analyses / -ing 尾假分析

**Watch words:** highlighting/underscoring/emphasizing..., ensuring..., reflecting/symbolizing..., contributing to..., fostering..., showcasing...

**中文版:** "体现了…"、"彰显了…"、"促进了…的发展"、"推动了…"

**Before:**
> The color palette resonates with the region's natural beauty, symbolizing local flora, reflecting the community's deep connection to the land.

**After:**
> The architect chose blue, green, and gold to reference local bluebonnets and the Gulf coast.

---

### 4. Promotional language / 推销语言

**Watch words:** boasts, vibrant, rich (figurative), profound, enhancing, showcasing, exemplifies, commitment to, nestled, in the heart of, groundbreaking, renowned, breathtaking, must-visit, stunning

**中文版:** "打造"、"赋能"、"引领"、"卓越"、"尖端"、"前沿"、"颠覆性"、"一站式"、"无缝衔接"

**Before:**
> Nestled within the breathtaking region, it stands as a vibrant town with rich cultural heritage and stunning natural beauty.

**After:**
> It is a town in the Gonder region, known for its weekly market and 18th-century church.

---

### 5. Vague attributions / 模糊归因

**Watch words:** Industry reports, Observers have cited, Experts argue, Some critics argue, several sources/publications

**中文版:** "业内人士指出"、"有关专家表示"、"据了解"、"相关研究表明"（无具体引用）

**Before:**
> Experts believe it plays a crucial role in the regional ecosystem.

**After:**
> The river supports several endemic fish species, according to a 2019 survey by the Chinese Academy of Sciences.

---

### 6. Outline-like "Challenges and Future" / "挑战与前景"八股

**Watch words:** Despite its... faces challenges..., Despite these challenges, Challenges and Legacy, Future Outlook

**中文版:** "尽管面临…挑战…但…"、"未来可期"、"前景广阔"、"任重道远"

**Before:**
> Despite challenges typical of urban areas, the ecosystem continues to thrive. The future looks bright.

**After:**
> Traffic congestion increased after 2015 when three new IT parks opened. A stormwater drainage project began in 2022.

---

## LANGUAGE PATTERNS

### 7. Overused AI vocabulary / AI 高频词

**English 50+ words:** Additionally, align with, crucial, delve, emphasize, enduring, enhance, foster, garner, highlight (verb), interplay, intricate/intricacies, key (adj), landscape (abstract), leverage, moreover, nuanced, pivotal, robust, seamless, showcase, tapestry (abstract), testament, underscore, valuable, vibrant, comprehensive, facilitate, furthermore, illuminate, meticulous, multifaceted, navigate, noteworthy, paradigm, paramount, realm, resonate, synergy, unprecedented

**Fix:** Replace with simpler, more specific alternatives. "Leverage" → "use"; "facilitate" → "help"; "comprehensive" → just be specific about what it covers.

---

### 8. Copula avoidance / 回避 is/are

**Watch words:** serves as, stands as, marks, represents [a], boasts, features, offers [a]

**中文版:** "作为…具有…"、"承载着…的使命"（when "是" works fine）

**Before:**
> Gallery 825 serves as the exhibition space. The gallery features four rooms and boasts 3,000 sq ft.

**After:**
> Gallery 825 is the exhibition space. It has four rooms totaling 3,000 sq ft.

---

### 9. Negative parallelisms / 否定并列

**Pattern:** "Not only...but...", "It's not just about..., it's..."

**中文版:** "不仅仅是…更是…"、"不只是…而是…"

**Before:**
> It's not just about the beat; it's about unlocking creativity at scale. It's not merely a song, it's a statement.

**After:**
> The heavy beat adds to the aggressive tone.

---

### 10. Rule of three / 三件套法则

**Problem:** Forcing ideas into groups of three to appear comprehensive.

**中文版:** "…、…和…"（硬凑三项）

**Before:**
> The event features keynote sessions, panel discussions, and networking opportunities. Expect innovation, inspiration, and industry insights.

**After:**
> The event includes talks and panels, with time for informal networking between sessions.

---

### 11. Elegant variation (synonym cycling) / 同义词轮换

**Problem:** Repetition-penalty causing excessive synonym substitution for the same referent.

**Before:**
> The protagonist faces challenges. The main character must overcome obstacles. The central figure eventually triumphs. The hero returns.

**After:**
> The protagonist faces many challenges but eventually triumphs and returns home.

---

### 12. False ranges / 假范围

**Pattern:** "from X to Y" where X and Y aren't on a meaningful scale.

**Before:**
> Our journey takes us from the singularity of the Big Bang to the cosmic web, from star birth to dark matter.

**After:**
> The book covers the Big Bang, star formation, and current theories about dark matter.

---

## STYLE PATTERNS

### 13. Em dash overuse / 破折号过度

**Problem:** LLMs use em dashes (—) far more than humans, mimicking "punchy" sales writing.

**Fix:** Replace most with commas, parentheses, or periods.

---

### 14. Overuse of boldface / 粗体滥用

**Problem:** Mechanically bolding key phrases. Replace inline bold with plain text.

---

### 15. Inline-header vertical lists / 行内标题列表

**Problem:** Lists where items start with bolded headers + colons. Convert to flowing prose.

---

### 16. Title case in headings / 标题大写

**Problem:** Capitalizing all main words in headings. Use sentence case instead.

---

### 17. Emojis / Emoji 装饰

**Problem:** Decorating headings or bullet points with emojis. Remove unless the context calls for them.

---

### 18. Curly quotes / 弯引号

**Problem:** ChatGPT uses curly quotes ("...") instead of straight quotes ("..."). Normalize to context-appropriate style.

---

## COMMUNICATION PATTERNS

### 19. Chatbot artifacts / 聊天残留

**Watch words:** "I hope this helps", "Of course!", "Certainly!", "You're absolutely right!", "Would you like...", "Here is a..."

**中文版:** "这是一个非常好的问题"、"下面我将为你详细介绍"、"希望对你有所帮助"、"如果你需要更多信息请告诉我"

**Fix:** Delete entirely. Start with content directly.

---

### 20. Knowledge-cutoff disclaimers / 知识截止声明

**Watch words:** "as of [date]", "While specific details are limited...", "based on available information..."

**中文版:** "根据目前公开的信息"、"截至目前"（无必要时）

**Fix:** Replace with specific sourcing or remove.

---

### 21. Sycophantic tone / 谄媚语气

**Watch words:** "Great question!", "Excellent point!", "You're absolutely right!"

**中文版:** "这个问题非常好！"、"您说得非常对！"、"非常感谢您的分享！"

**Fix:** Remove entirely or rephrase as neutral acknowledgment.

---

## FILLER AND HEDGING

### 22. Filler phrases / 填充短语

| Before | After |
|---|---|
| In order to achieve this goal | To achieve this |
| Due to the fact that | Because |
| At this point in time | Now |
| In the event that | If |
| has the ability to | can |
| It is important to note that | *(delete)* |
| It is worth noting that | *(delete)* |
| At its core | *(delete)* |
| In today's rapidly evolving | *(delete or be specific)* |

**中文版填充短语：**

| Before | After |
|---|---|
| 值得注意的是 | *(删除)* |
| 众所周知 | *(删除或给出实际来源)* |
| 不可否认的是 | *(删除)* |
| 在当前背景下 | *(删除或说具体背景)* |
| 从某种程度上来说 | *(删除)* |

---

### 23. Excessive hedging / 过度对冲

**Before:**
> It could potentially possibly be argued that the policy might have some effect on outcomes.

**After:**
> The policy may affect outcomes.

---

### 24. Generic positive conclusions / 泛泛积极结尾

**Watch words:** "The future looks bright", "Exciting times lie ahead", "a major step in the right direction"

**中文版:** "未来可期"、"前景广阔"、"大有可为"、"令人期待"

**Before:**
> The future looks bright. Exciting times lie ahead as they continue their journey toward excellence.

**After:**
> The company plans to open two more locations next year.

---

### 25. Hyphenated word pair overuse / 连字符过度使用

**Words to watch:** third-party, cross-functional, client-facing, data-driven, decision-making, well-known, high-quality, real-time, long-term, end-to-end

**中文版:** 此模式主要影响英文，中文中对应的是过度使用四字成语式修饰

**Problem:** AI hyphenates common word pairs with perfect consistency. Humans are inconsistent with hyphenation for common compounds.

**Before:**
> The cross-functional team delivered a high-quality, data-driven report on our client-facing tools.

**After:**
> The cross functional team delivered a high quality, data driven report on our client facing tools.

---

### 26. Paragraph-opening transition stacking / 段首连接词堆砌

**Watch words:** Additionally, Furthermore, Moreover, Meanwhile, It is worth noting, Notably

**中文版:** 此外、另外、同时、与此同时、值得注意的是、值得一提的是、更重要的是

**Problem:** AI opens every paragraph with a transition word. Human writers let logical sequence carry the flow without explicit signposting.

**Fix:** Delete the transition word and let the paragraph start directly with its content. If the logical connection is unclear without it, the paragraph order itself may need restructuring.

---

### 27. Quotable statements / 金句式总结

**Watch words:** "At the end of the day...", "It's not about X — it's about Y", "The real question is..."

**中文版:** "归根结底…"、"说到底…"、"真正重要的不是…而是…"

**Problem:** AI produces polished, quotable-sounding conclusions that read like keynote slides or motivational posters. Real writing ends with facts, plans, or honest assessments — not sound bites.

**Before:**
> At the end of the day, it's not about the technology — it's about the people behind it.

**After:**
> The team plans to ship the v2 API by March and revisit the auth flow after user testing.

---

## Quick self-check during rewriting

Before delivering any rewrite, run through this:

- [ ] Scan for repeated sentence stems
- [ ] Cut empty intensifiers
- [ ] Replace vague praise with specifics
- [ ] Remove generic opening and closing lines
- [ ] Confirm nothing was fabricated for style
- [ ] Check for AI vocabulary from the list above
- [ ] Ensure sentence lengths vary naturally
- [ ] Verify no chatbot artifacts remain
- [ ] Check for paragraph-opening transition stacking (#26)
- [ ] Delete quotable sound-bite sentences (#27)
- [ ] Verify hyphenation consistency is human-like (#25)

---

## Appendix: Chinese AI phrase replacement tables / 中文 AI 高频词替换表

The inline "中文版" notes above give quick indicators. This appendix provides full replacement tables for Chinese text rewriting.

### A1. 意义膨胀类

| AI 味表达 | 改写方向 |
|---|---|
| 具有重要意义 / 重大意义 | 删除，或说明具体意义 |
| 标志着…的转折点 / 里程碑 | 只在确实如此时保留，否则变为简单陈述 |
| 深远影响 / 不可磨灭的印记 | 说清楚具体影响了什么 |
| 关键节点 / 关键时刻 | 改为具体时间或事件 |
| 推动了…的蓬勃发展 | 用数据或具体结果替代 |

### A2. 推销/赋能类

| AI 味表达 | 改写方向 |
|---|---|
| 打造 | "做"/"建"/"设计" |
| 赋能 | 说清楚具体给谁提供了什么 |
| 引领 | "在…方面先做了…" |
| 全面 / 全方位 | 说清楚具体覆盖哪些方面 |
| 一站式 | 说清楚具体包含什么 |
| 无缝衔接 / 无缝对接 | "衔接"/"兼容"/"互通" |
| 赋予…新的活力 | 删除，或说清楚具体变化 |
| 全面提升 / 显著提升 | 用数据或观察佐证 |
| 高质量发展 | 说清楚"高质量"指什么 |
| 卓越 / 尖端 / 前沿 / 颠覆性 | 多数情况删除 |

### A3. 模糊归因类

| AI 味表达 | 改写方向 |
|---|---|
| 业内人士指出 | 给出具体是谁 |
| 有关专家表示 | 引用具体专家或文献 |
| 据了解 / 据悉 | 给出来源 |
| 相关研究表明 | 引用具体研究 |
| 普遍认为 | 删除或给出依据 |
| 众所周知 | 删除 |

### A4. 八股结构类

| AI 味表达 | 改写方向 |
|---|---|
| 尽管面临…挑战…但… | 分开说挑战和应对，不要套模板 |
| 未来可期 / 前景广阔 | 删除，或换成具体计划 |
| 任重道远 | 删除 |
| 总而言之 / 综上所述 | 多数情况直接删除 |
| 不仅仅是…更是… | 简化为直接陈述 |
| 在…的道路上 | 删除 |

### A5. 填充类

| AI 味表达 | 处理 |
|---|---|
| 值得注意的是 | 删除 |
| 不可否认的是 | 删除 |
| 从某种程度上来说 | 删除或说具体的程度 |
| 在当前背景下 | 删除或写清楚什么背景 |
| 毋庸置疑 | 删除 |
| 需要指出的是 | 删除 |
| 换言之 / 也就是说 | 多数情况删除 |

### A6. 助手残留类

| AI 味表达 | 处理 |
|---|---|
| 这是一个非常好的问题 | 删除 |
| 下面我将为你详细介绍 | 删除，直接写内容 |
| 希望对你有所帮助 | 删除 |
| 如果你需要更多信息请随时告诉我 | 删除 |
| 好的，我来… | 删除，直接执行 |
| 当然可以！ | 删除 |

### A7. 谄媚类

| AI 味表达 | 处理 |
|---|---|
| 您说得非常对！ | 删除或改为中性回应 |
| 这个问题非常好！ | 删除 |
| 非常感谢您的分享！ | 删除 |
| 您的想法很有启发性！ | 删除 |

### A8. 学术特异性与 Turnitin 查重高危词汇

**8.1 学术中文 AI 高频词（动作与评价）：**

| AI 味表达 | 改写方向（降 AIGC 策略） |
|---|---|
| 实现了…的突破 / 填补了空白 | 说清楚具体突破了哪个限制、填补了什么具体场景的空白 |
| 具有创新性 / 表现出优异的性能 | 说清楚创新在哪，性能用量化数据表达 |
| 进一步验证了 / 证明了其有效性 | 说清楚验证了什么假设工作，在什么置信度下有效 |
| 展现了广阔的应用前景 / 具有不可估量的潜力 | 列举具体的工业或商业落地场景，砍掉后面的修饰语 |
| 为…提供了新的思路/视角 | 明确指出"原本是用A思路，现在换成了B思路" |

**8.2 论文引言/摘要公式化表达（Turnitin 极易标红区）：**

这是一组强相关性的句子组合，Turnitin 查重时只要判定逻辑链满足此模板，大概率标红。

| AI 八股模板链 | 破解方法（打碎模板） |
|---|---|
| 近年来，随着X的发展，Y引起了广泛关注。 | 删除宏大叙事。改成："Y在[具体应用阶段]遇到了瓶颈。" |
| 然而，现有方法往往存在…的不足。 | 必须点名具体的方法："Smith(2023)和Wang(2024)的方法在处理长尾数据时准确率会掉30%。" |
| 为了解决上述问题，本文提出了… | 直接说："本文针对小样本场景提出了一个双流网络模型。" |
| 实验结果表明，本方法优于… | 给出具体对比数字："在CIFAR-100上比基线提升4.5%。" |
| 据我们所知，这是首次… | 没有做过极其详尽的文献综述，严禁使用此句。 |

**8.3 学术长句及连接词滥用：**

- 此外 / 另外 / 同时 / 与此同时 / 值得注意的是 / 值得一提的是 / 综上 / 由此可见
- **改写策略：** 学术写作中不需要每个段落都用连接词进行起承转合。通过调整语序，让逻辑关系自身推进，或直接用"实验 1、实验 2"作为引导。

**8.4 方法论部分的"上帝视角"：**

AI 会用教科书的语气写实验步骤，而不会像真实的人类科研者一样表达在实验条件下的权衡与主观选择。
- ❌ "本研究仔细设计了一套筛选流程以确保数据的质量。"
- ✅ "我们将信噪比低于 0.8 的样本丢弃，因为这部分样本在预实验中导致了模型不收敛。"

---

### A9. 商业软文与小红书结构模板

商业写作和社交媒体中的 AI 模式十分明显，容易让转化率断崖式下跌。

| 小红书 AI 模式体 | 破解与重写方向 |
|---|---|
| **空泛痛点引入：** "你是不是也经常遇到X的问题？" | 描述一个具体、极其具象的生活切片。例如："每次开完会桌面就乱成狗" |
| **生硬的列点（Emoji 堆砌）：** <br>👉优势一：...<br>👉优势二：... | 把产品功能融入到单一的一日体验流水账中；或者删掉过多的 Emoji。 |
| **强行拔高的总结：** "让生活变得更好！大家都去试试吧！" | 直接放购买/注册的利益点，或者留下一个真实的购买体验："虽然有点贵，但确实省时间。" |
| **过度礼貌的互动：** "欢迎在评论区留言讨论！/ 留下你的看法！" | 真人博主通常会抛出一个极端的选择题或找认同："有懂的人吗？" |

---

### A10. 2026 High-Polished & "Bypasser" Signatures

Current AI models (especially those used in "Humanizers") often leave a distinctive "over-polished" trail.

**Watch for:**
- **Asymptotically perfect grammar**: Zero missed commas, zero sentence-level ambiguities even in complex technical contexts.
- **Rhythmic parallelism in segments**: 3 sentences of ~15 words followed by 1 sentence of ~40 words, repeated every 2 paragraphs.
- **Connector substitution**: Using "Nontrivial" instead of "Significant" *every single time* (a sign of a mechanical humanizer tool).

**Fix**: Introduce "Adaptive Imperfection" (Strategy 13) and "Segment-level Breaks".

---

### A11. Chinese-Academic AI Phrase List (2026 Update)

**High-Frequency AI Constructs:**
- "具有极其重要的意义" (inflated significance)
- "旨在解决...的问题" (formulaic objective)
- "基于上述分析，我们不难得出..." (forced convergence)
- "不仅提升了...，更强化了..." (not only... but also... pattern)

**Replacement Strategy**: Use direct causality. "我们要解决X，是因为Y导致了Z。" (Direct, active, specific).
