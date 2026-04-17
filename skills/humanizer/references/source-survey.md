# GitHub Humanizer Survey Notes

This note records the GitHub scan that motivated the local `humanizer` skill.

## Repositories reviewed (v1.0–v2.0)

1. `blader/humanizer`
   - Link: https://github.com/blader/humanizer
   - Why it mattered: strong skill-native packaging and a useful catalog of overly generic writing patterns.
   - Why not imported as-is: it is explicitly framed around removing signs of AI-generated writing and includes a detector-evasion style audit loop.

2. `Text2Go/ai-humanizer-mcp-server`
   - Link: https://github.com/Text2Go/ai-humanizer-mcp-server
   - Why it mattered: MCP packaging is relevant to agent tooling.
   - Why not imported as-is: it is mainly a wrapper around an external detection API, which is a poor fit for this local skills repo and ties the workflow to detector-oriented SaaS behavior.

3. `CBIhalsen/text-rewriter`
   - Link: https://github.com/CBIhalsen/text-rewriter
   - Why it mattered: simple open-source rewrite implementation.
   - Why not imported as-is: the approach is too shallow and synonym-driven to be a good writing skill.

## v3.0 deep survey (2026-03)

4. `blader/humanizer` v2.3.0 (re-reviewed)
   - Updates: pattern #25 (hyphenated word pair overuse); two-pass self-audit loop
   - **Adopted**: pattern #25, self-audit concept expanded to three-pass

5. `op7418/Humanizer-zh`
   - Link: https://github.com/op7418/Humanizer-zh
   - Chinese localization with 5 core rules, 5-dimension quality scoring (50pt), "delete quotable lines", "trust the reader"
   - **Adopted**: core rules, scoring system, new concepts

6. `khadinakbaronline/humanizer-pro-mcp` — MCP server; Stealth/Academic/SEO modes. **Not adopted**: SaaS dependency.

7. `DadaNanjesha/AI-Text-Humanizer-App` — Streamlit app. **Not adopted**: too shallow.

### Academic papers

8. **ICLR'24 Spotlight**: Multiscale PU Detection (https://github.com/YuchuanTian/AIGC_text_detector)
9. **ACL 2024**: RAID benchmark (https://github.com/liamdugan/raid)
10. **Turnitin AI Detection Whitepaper (2025 Updates)**: Focus on phrase-level predictability and sentence-entropy gradients.
11. **NeurIPS 2023**: Paraphrasing evades detectors (https://github.com/martiansideofthemoon/ai-detection-paraphrases)

## v3.1 深度强化 (2026-03)
- **目标**：强化科研/论文 Humanization，打通下游精修链路。
- **集成**：Turnitin 专项对抗策略（引用融合、方法论去同质化）、100分制评分系统、Orchestrator 架构（向下游 handoff 至 $paper-writing/$copywriting）。

## Integration principles

Integrated: pattern-level cleanup, perplexity/burstiness awareness, detection-mechanics reference, adversarial strategies, quality scoring, three-pass audit.

Not integrated: SaaS APIs, detector-bypass promises, mechanically forced patterns, fake anecdotes.
