---
description: 高级 AIGC 文本人性化通道 — LLM 驱动的深度改写，支持声音校准和两阶段自审闭环。调用现有 Rust humanizer（research_aigc_humanize）做预处理，再用 LLM 做语义级精细改写。
metadata:
  platforms:
  - supported
  tags:
  - aigc
  - humanize
  - rewriting
  - voice-calibration
  - audit-loop
  version: '1.0.0'
name: humanizer-v2
scene: research
risk: low
routing_gate: none
routing_layer: L4
routing_owner: owner
routing_priority: P3
session_start: n/a
short_description: >-
  LLM 增强的文本人性化：先 Rust 规则引擎预处理，再 LLM 语义级精细改写。
  支持声音校准（从用户写作样本提取风格）、两阶段自审闭环。
trigger_hints:
- 消除AI痕迹
- 改写润色
- 去AI味
- humanize
- 脱AIGC
- AIGC降重
- 人性化改写
- 消除机器感
- voice calibration
- 写作风格匹配
when_to_use: >-
  用户需要高质量消除文本 AI 痕迹，特别是投稿前的终稿润色、非确定性场景。
do_not_use: >-
  批量处理场景（用 research_aigc_humanize MCP 工具）。
  纯英文简单替换（用默认 humanizer 即可）。
---

# humanizer-v2 — LLM 增强的文本人性化

> 本技能是 `core/research-harness/src/aigc/humanizer.rs` V1 Rust 规则引擎的 LLM 增强层。
> 先调用 `research_aigc_humanize`（确定性规则）做基线处理，再用 LLM 进行语义级精细改写。

## 工作流程

```
用户输入文本
    │
    ▼
1. Rust 基线处理 ── 调用 research_aigc_humanize（词汇替换 + 句法改写 + 句子变化 + 从句重构 + 填充词删除 + 清晰度降级）
    │
    ▼
2. 声音校准 ── 如果用户提供了 2-3 段写作样本，分析其句式节奏、词汇选择、标点习惯
    │
    ▼
3. LLM 精细改写 ── 对 Rust 输出做语义级调整，针对性解决残留的 AI 痕迹
    │
    ▼
4. 自检 ── 调用 research_aigc_check 评估改写后的 AIGC 分数
    │
    ▼
5. 如需反复 ── 如果分数未达标（score > 30），回到步骤 2-3 迭代，最多 3 轮
    │
    ▼
输出最终文本 + AIGC 分数对比
```

## 使用方式

```bash
# 1. 先读参考文档了解声音校准
skill_read("humanizer-v2/voice-calibration")

# 2. 如果用户有写作样本，先提取风格
# (用户提供 2-3 段自己的写作)

# 3. 执行人性化处理
# 本 skill 指导 LLM 完成剩余的改写工作
```

## 改写强度（可选参数）

| 强度 | 适用场景 | 说明 |
|------|---------|------|
| light | 学术论文/投稿 | 仅微调最明显的 AI 模式，保留学术语气 |
| medium | 一般写作 | 平衡改动力度和自然度 |
| strong | 需要大幅改写的场景 | 改变句式和结构以最大化人性化 |

## 注意事项

- 始终**先调用 research_aigc_humanize** 做基线处理，再让 LLM 精修（而非 LLM 从头改写）
- 学术模式应保留专业术语、参考文献格式、数据陈述的精确性
- 如果中英文混排，对中文部分应用中文规则，英文部分应用英文规则
