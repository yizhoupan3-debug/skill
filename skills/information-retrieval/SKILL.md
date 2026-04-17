---
name: information-retrieval
description: |
  Run multi-round pre-action research across docs, code, GitHub, and the web before acting.
  Use for 调研 / 仔细调研 / 深度调研 / 多轮调研 / 搜一下 / 查一下 / 对标分析 / 方案调研,
  option comparison, or evidence gathering when freshness and source quality matter.
  Do not use for stable facts, debugging, academic literature review, or scraping.
routing_layer: L1
routing_owner: owner
routing_gate: none
session_start: n/a
short_description: Run multi-round research before acting or recommending
metadata:
  version: "1.3.0"
  platforms: [codex, antigravity, claude-code]
  tags:
    - information-retrieval
    - research
    - deep-research
    - benchmarking
    - option-comparison
    - investigation
---

# Information Retrieval

This skill owns **pre-action research**: gather external evidence, compare
options, and reduce uncertainty before implementation, recommendation, or
planning.

## When to use

- The user asks to **调研 / 仔细调研 / 深度调研 / 多轮调研 / 对标 / 查最新做法 / 先调研再给方案**
- The task needs non-trivial research before implementation or recommendation
- GitHub examples, official docs, or recent web evidence would materially improve the answer
- The output should synthesize sources into findings, tradeoffs, and a recommendation
- The assistant is uncertain enough that acting without retrieval would be risky

## Do not use

- Academic paper search → use `$academic-search`
- Literature review / novelty check → use `$literature-synthesis`
- Structured page extraction / crawling → use `$web-scraping`
- OpenAI official product guidance → use `$openai-docs`
- Root-cause debugging of a failure → use `$systematic-debugging`
- Repository, issue, PR, timeline, or code-history deep dives are central → use `$github-investigator`
- A simple stable fact answerable directly without research

## Primary operating principle

This owner should behave like a **research orchestrator**, not a long-form browsing transcript:

1. narrow first, then deep-read
2. keep the main thread to scope, findings, conflicts, and recommendation
3. sink source detail into notes, tables, and cited outputs
4. if runtime policy permits, sidecar high-value independent research slices
5. if spawning is blocked, preserve the same retrieval matrix in local-supervisor mode

## Main-thread compression contract

The main thread should contain only:

- retrieval goal
- comparison axes
- key findings
- conflicts / unknowns
- recommendation and confidence

## Runtime-policy adaptation

If multiple independent retrieval slices materially help and runtime policy permits:

- route them through [`$subagent-delegation`](/Users/joe/Documents/skill/skills/subagent-delegation/SKILL.md)

If runtime policy does **not** permit spawning:

- keep the same retrieval slices as a local-supervisor queue
- return only synthesized findings to the main thread

## Core workflow

1. Define the retrieval brief:
   - what must be learned
   - what decision it supports
   - how fresh the evidence must be
   - what deliverable shape is needed
2. Run **broad scan** first:
   - identify dimensions, stakeholders, or comparison axes
   - collect likely primary sources before deep reading
3. Run **deep dives** on the highest-value dimensions:
   - search in ascending cost
   - fetch only the sources that matter
   - benchmark GitHub repositories explicitly when implementation patterns matter
4. Verify before recommending:
   - cross-check important claims with at least two sources when feasible
   - record conflicts, unknowns, or stale evidence explicitly
5. Return a concise synthesis:
   - key findings
   - risks and gaps
   - recommendation tied to evidence

## Source preference

Prefer in this order when applicable:

1. official docs / primary sources
2. source code / reference repos
3. issue threads / discussions
4. third-party blogs or summaries

## Output defaults

```markdown
## Retrieval Summary
- Goal: ...
- Scope: ...
- Freshness: ...
- Confidence: High / Medium / Low

### Key Findings
- ...

### Conflicts / Unknowns
- ...

### Recommendation
- ...

### Sources
- ...
```

## Hard constraints

- Do not fabricate missing evidence.
- Do not deep-read everything; narrow first, then read.
- Do not treat one unverified blog post as source of truth.
- If repository structure, issues, PRs, commits, or timeline reconstruction become central, reroute to `$github-investigator`.
- If research reveals local skill-library gaps, hand off to `$skill-scout`.

## References

- [references/deep-research-playbook.md](references/deep-research-playbook.md)
- [references/source-scoring.md](references/source-scoring.md)
- [references/git-research-hygiene.md](references/git-research-hygiene.md)
