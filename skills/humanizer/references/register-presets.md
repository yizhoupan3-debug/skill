# Register Presets

Use these presets after the base naturalization pass so the output matches the user's intended context.
Each preset includes English and Chinese guidance where applicable.

## 1. Academic-adjacent formal prose

Use when the text should stay formal but not sound inflated.

Prefer:
- precise nouns
- restrained verbs
- explicit scope
- evidence-linked claims

Reduce:
- broad significance language
- generic "this study highlights"
- filler transitions

### 中文：学术中文（论文 / 学位报告 / 基金申请 / 应对查重）

Prefer:
- 精确术语，保持一致性
- 证据驱动的陈述（围绕具体数据、P值、对比结果）
- 克制的方法论描述（写出"为什么这么做"的决策，而不是教科书式的堆砌）
- 长短句结合，破坏 AI 段落的结构匀称度（对抗 Turnitin 等学术查重工具）

Reduce:
- "具有重要意义" / "填补了空白" / "展现了广阔前景" 等空泛判断
- "近年来，随着…" 的宏大叙事开头
- 频繁的 "此外" / "同时" / "值得注意的是"
- 连续超过3句以上的结构相似长句

**Pipeline 注意事项：**
- 如果用户目的是**深度精修论文**，使用 Humanizer 去除 AI 结构后，建议引导并移交（Handoff）给 `$paper-writing` 进行学术逻辑层面的最终润色。本级重点只负责打破八股结构。

Example:

> ❌ "本研究具有重要的理论意义和实践价值，填补了该领域的研究空白，为未来研究提供了新的思路。"
> ✅ "本研究解决了 X 方法在 Y 条件下的适用性问题（此前仅 Zhang et al. 2021 在 Z 场景做过类似尝试），实验结果在三个数据集上均优于基线 3–5%。"

## 2. Professional email / statement

Use when the output is for outreach, update notes, internal communication, or application-style prose.

Prefer:
- direct openings
- short to medium sentence lengths
- concrete asks or outcomes
- polite but not over-servile tone

Reduce:
- throat-clearing
- corporate slogans
- over-formal apology padding

### 中文：职场中文（工作邮件 / 周报 / 汇报）

Prefer:
- 直接开场，结论先行
- 短到中等句子
- 具体的行动项和时间节点
- 礼貌但不过度客气

Reduce:
- 开头寒暄过长
- 企业黑话（"赋能"、"抓手"、"闭环"、"颗粒度"）
- 过度铺垫

Example:

> ❌ "感谢各位领导的大力支持和团队成员的辛勤付出。经过本周全面深入的工作推进，各项任务取得了显著成效。"
> ✅ "本周完成了三件事：❶ 用户反馈系统上线（周三）❷ 修复了支付流程的两个 bug ❸ 完成了 Q2 OKR 初版。下周重点是性能优化。"

## 3. Technical explanation / docs

Use when the user wants cleaner product, API, or engineering prose.

Prefer:
- stable terminology
- causal order
- compact definitions
- examples only when grounded

Reduce:
- marketing adjectives
- abstract promises
- noisy emphasis formatting

### 中文：技术中文（技术文档 / 设计文档 / Code Review）

Prefer:
- 术语稳定（同一概念用同一个词）
- 因果顺序说明
- 紧凑定义
- 示例基于真实场景

Reduce:
- 营销式形容词（"强大的"、"灵活的"）
- 抽象承诺（"提供无缝体验"）
- 格式噪音（过度粗体、emoji）

Example:

> ❌ "该系统采用先进的微服务架构，提供无缝衔接的服务体验，全面赋能开发团队高效协作。"
> ✅ "系统拆成 5 个服务：auth、gateway、order、payment、notification。服务间用 gRPC 通信，用 Redis 做缓存。"

## 4. Social / creator / personal post

Use when the user wants a more personal, internet-native voice.

Prefer:
- stronger rhythm contrast
- lighter first person when appropriate
- selective attitude
- concrete scenes or reactions only if already grounded in the draft

Reduce:
- corporate polish
- robotic neutrality
- repetitive list cadence

### 中文：社交中文（朋友圈 / 微博 / 公众号 / 小红书）

Prefer:
- 更强的节奏对比（长短句交替）
- 适当的第一人称
- 有态度的表达（不是中性报告）
- 如果草稿中已有具体场景/反应就保留

Reduce:
- 企业公关腔
- 机器人式的中性
- 重复的列表节奏

Example:

> ❌ "本产品凭借卓越的用户体验和创新性的设计理念，为广大用户带来了全新的使用感受。"
> ✅ "用了一周，说真的挺好用。特别是那个自动整理功能，帮我省了每天 20 分钟的分类时间。唯一的槽点是导出格式少了点。"

## 4.5 Commercial / Advertorial (种草营销文体)

Use when preparing product reviews or conversion-focused social posts.

Prefer:
- 强代入感的具象痛点开场
- 单一核心体验的故事化包装
- 真实的试用反馈（带有一点合理的吐槽）

Reduce:
- "你是不是也经常遇到X的问题？"（太生硬）
- 说明书式的罗列功能
- 陈词滥调的感叹词（绝绝子、氛围感拉满、一秒沦陷等高危词）

**Pipeline 注意事项：**
- 如果需要输出专业的商业文案架构（如 AIDA、PAS 等），可以将打碎 AI 味后的素材交由 `$copywriting` 执行最终商业文案封装。

Example:

> ❌ "你是不是也总觉得桌面很乱？这款收纳盒采用多层设计，帮你轻松解决整理难题，氛围感拉满，赶紧入手吧！"
> ✅ "我的书桌常年处于'找根笔要翻三分钟'的状态。换了这个收纳盒后，最直观的改变是不用再翻箱倒柜了。不是什么神器，但确实顺手很多。"

## 5. Founder / personal authority voice

Use when the user wants writing that sounds like a founder, executive, or subject-matter expert sharing hard-won experience.

Prefer:
- strong first-person voice
- specific dates, numbers, failures, and lessons
- stories before abstractions
- opinionated assertions backed by experience
- short paragraphs and punchy rhythm

Reduce:
- corporate distance
- neutral hedging
- abstract claims without personal context
- "we believe in" language

## 6. Storytelling / narrative voice

Use when the user wants scene-driven, immersive prose with deliberate pacing.

Prefer:
- show over tell
- deliberate sentence length variation (very short followed by longer flowing ones)
- sensory details when grounded in the draft
- delayed reveals and buildup
- natural dialogue rhythms

Reduce:
- summary-first structure
- uniform paragraph lengths
- abstract analysis where concrete scenes exist
- listicle formatting

## 7. Personal statement / 个人陈述 / 申请材料

Use when the user is writing a personal statement, cover letter, or application essay.

Prefer:
- 具体经历 > 抽象品质
- 时间线清晰
- 有反思的叙事
- 适当的不确定性（比"坚信"更真实的是"当时我并不确定"）

Reduce:
- "我是一个热爱…的人"
- "培养了我…的能力"
- "坚信…" / "深深地被…吸引"

Example:

> ❌ "我对计算机科学有着浓厚的兴趣和坚定的热情。通过四年的学习，我培养了扎实的编程能力和良好的团队合作精神。"
> ✅ "大一时我不喜欢编程课。转折点是大二那门操作系统课——我花了两个周末写 shell，debug 到凌晨三点，但第一次觉得这件事有意思了。"

## Guardrail

Do not force personality into contexts that need restraint.
The right output should sound **written by a real person in that context**, not forcibly casual or forcibly polished.
