# Adversarial Rewriting Strategies

15 strategies that naturally lower AI detection scores by improving prose quality.
These are **not tricks** — they are good writing practices that happen to disrupt the statistical signals detectors rely on, including specialized academic detectors like Turnitin.

For the detector mechanics behind these strategies, see [detection-mechanics.md](detection-mechanics.md).

---

## Part 1: Core Perplexity & Burstiness Strategies

### Strategy 1: Perplexity injection — use the accurate-but-unexpected word
AI picks the statistically safest word. Humans pick the word that fits their voice.
- Replace "utilize" → "lean on", "employ"
- Replace "significant" → "nontrivial", "sizable", "measurable"
- Replace "facilitate" → "make possible", "open the door to"
- **Rule**: the replacement must be equally accurate, just less predictable.

### Strategy 2: Three-item pattern disruption
AI loves groups of three. Humans don't count.
- Three adjectives → drop to two or expand to four with different weights.
- "X, Y, and Z" → "X and Y (Z matters less here)".

### Strategy 3: Transition word erasure
AI opens every paragraph with a transition. Humans let logic carry the flow.
- Delete paragraph openers: 此外 / Additionally / Furthermore / Moreover / Notably.
- **Better approach**: just start the next idea.

### Strategy 4: Sentence-opening diversification
Scan for: 3+ consecutive sentences starting with "The...", "This...", or Subject-Verb.
Fix by varying the first element: prepositional phrase, dependent clause, direct object, or a short fragment.

### Strategy 5: Structural disruption
- Start with the conclusion, then backfill evidence.
- Drop subheadings when the text is short enough to flow as prose.
- End abruptly when the point is already made — no wrap-up paragraph needed.

---

## Part 2: Turnitin & Academic-Specific Strategies (New)

Turnitin's AI detector specifically targets academic formulas, predictability in methodology sections, and over-polished literature reviews.

### Strategy 6: Citation integration variation (Turnitin-killer)
AI mechanically summarizes papers: "Smith (2020) stated X. Jones (2021) argued Y."
- **Humanize by clustering and reacting**: "While multiple studies agree on X (Smith, 2020; Jones, 2021), they overlook..."
- **Humanize by active critique**: "Smith's (2020) model struggles with the edge case of Z, a limitation this paper addresses by..."
- Never use list-like citation sequences. Synthesize the findings.

### Strategy 7: De-sterilize the Methodology section (2026 Update)
AI writes methodology like a textbook. Humans write it like a lab report of what actually happened.
- **Narrative of Decisions**: Instead of "Data was collected...", use "We opted to collect data via X because preliminary tests with Y showed unacceptable variance."
- **Mention Exclusions & Failures**: AI never fails. Humans do. "We excluded 4 samples where the sensor drifted >2%."
- **Arbitrary Thresholds**: Explain why a number was picked. "We set the cutoff at 0.05, a standard but conservative choice for this pilot study."
- Use passive voice occasionally (it's expected in science), but break the rhythm with active statements about decisions: "We chose to..." or "To account for X, we..."

### Strategy 8: Hedge specifically, not generically
AI hedging: "It could potentially be possible that X might indicate Y."
Human hedging (Science): "X indicates Y, assuming Z remains constant."
- Remove "can be considered", "it is widely believed", "has been observed".
- Replace with the exact statistical or logical constraint.

### Strategy 9: Break the "Importance Statement"
AI abstracts and intros always start by inflating the field's importance. Turnitin flags this heavily.
- ❌ "In recent years, machine learning has played an increasingly vital role in medical imaging, drawing widespread attention from researchers worldwide."
- ✅ "Diagnosing rare lesions from MRI scans remains bottlenecked by high false-positive rates, even with recent CNN architectures."
- Start directly with the specific problem, not the history of the universe.

### Strategy 10: Academic sentence-length burstiness
Turnitin flags academic text where every sentence is exactly clause-comma-clause (20-25 words).
- Follow a dense, citation-heavy sentence with a short, declarative one.
- *Example*: (Long sentence with formulas and citations). "This assumption is problematic." (Short punch). (Back to longer explanation).

---

## Part 3: Voice & Imperfection Strategies

### Strategy 11: Trust the reader
AI over-explains. Good writing assumes the reader is competent.
- Delete "As we all know..." / "众所周知".
- Delete step-by-step hand-holding for concepts the audience already knows.

### Strategy 12: Subjective judgment injection
AI is pathologically neutral. Humans have opinions.
- "This approach has advantages and disadvantages" → "This approach works well for X but falls apart when Y."

### Strategy 13: Controlled imperfection (Adaptive Imperfection)
Natural human markers (use sparingly and authentically):
- **Technical Friction**: Use a specific, slightly obscure technical term instead of a smooth synonym.
- **Parenthetical asides**: "(though I'm not sure this holds for all cases)" — shows real-time cognitive processing.
- **Register breaks**: A casual phrase in otherwise formal prose (e.g., "In short, it works.").
- **Hedges from genuine uncertainty**: "I suspect..." / "My guess is..." instead of "It is likely that...".

---

## Part 4: 中文专项降 AIGC 策略 (Chinese-Specific)

### Strategy 14: 动词化与去名词化 (De-nominalization)
中文 AI 极度依赖复合名词串和抽象名词。真人写作多用动词。
- ❌ "实现了交互效率的显著提升" → ✅ "让交互更快了"
- ❌ "缺乏有效的问题解决机制" → ✅ "遇到问题不知道怎么解决"
- **核心操作**：把静态的"名词+名词"拆解成动态的"主谓宾"结构。

### Strategy 15: 口语颗粒度注入 (Oral granularity)
检测器通过统计"高雅书面语"的出现频率来打分。
- 将 "致力于" 换成 "想做 / 打算"
- 将 "在此基础上" 换成 "顺着这个思路"
- 将 "总而言之" 换成 "说白了 / 核心就一句"
- **适用边界**：科研论文慎用，但在商业文案、邮件、汇报中立竿见影，极其有效。

---

## Part 5: 2026 Advanced Detection Countermeasures

### Strategy 16: Citation Synthesis & Clustering
AI mechanically summarizes: "A said X. B said Y."
- **Humanize**: "While A (2023) and B (2024) converge on X, they differ fundamentally on the role of Y, a tension we resolve by..."
- This breaks the "Listicle" pattern and shows cross-document reasoning, which current LLMs struggle to do naturally without prompting.

### Strategy 17: Asymmetric Heading-to-Content Ratio
AI produces perfectly balanced sections (e.g., 3 subheadings, each 200 words).
- **Humanize**: Let one section be a massive deep dive (500 words) and the next a quick 50-word bridge. Disrupt the structural "perfection."

### Strategy 18: Recent Data/Event Injection
Inject specific events or data points that occurred *after* the most common LLM training cutoffs (e.g., specific 2025/2026 conference results). AI often defaults to generic "In recent years..." whereas humans cite "As seen in last month's [Specific Event] report..."

---

### Strategy 19: Asymmetric Narrative Friction (Pragmatic Grounding)
AI writes methodology and results as a linear success story.
- **Humanize**: Narrate the path taken. "We initially attempted X, but found Y; thus, we shifted to Z."
- **Focus on the 'Why'**: Explain the human reason for parameter choices (e.g. "We chose 20 epochs to strike a balance between training time and convergence stability").
- **Audit the outliers**: Mentioning specific data points that were odd or excluded is a strong human signal.

---

## Anti-pattern: what NOT to do

These "tricks" are brittle and produce worse prose:

| Bad trick | Why it fails |
|---|---|
| Random synonym injection | Produces awkward, unnatural phrasing |
| Deliberate typos / misspellings | Detectors ignore surface errors; looks sloppy |
| Inserting invisible characters | Detected by plagiarism tools like Turnitin; useless against statistical detectors |
| English to Chinese to English | Translating back and forth destroys academic nuance |
| Adding fake citations | Fabrication; severe academic integrity risk |
| Mechanically alternating length | Creates a new detectable pattern |

The best anti-detection strategy is genuine, specific, well-voiced writing.
