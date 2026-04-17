---
name: skill-scout
description: |
  Research external skill ecosystems and produce gap-analysis proposals for the local skill library.
  Use when the task is 调研外部 skill, 对标 skill 生态, 从 GitHub 学 skill best practices,
  or scanning outside repos for ideas to strengthen existing skills. This skill scouts and evaluates;
  it does not directly rewrite local skills or replace generic research / repo-deep-dive owners.
routing_layer: L1
routing_owner: owner
routing_gate: none
routing_priority: P2
session_start: n/a
metadata:
  version: "1.2.0"
  platforms: [codex, antigravity]
  tags:
    - skill-scouting
    - external-research
    - gap-analysis
    - skill-enhancement
    - benchmarking
risk: low
source: local
---

# skill-scout

This skill owns **external ecosystem scouting for skill improvement**.
It researches outside skill libraries, evaluates what is actually better,
and proposes absorbable improvements for the local library.

## When to use

- The user wants to benchmark local skills against external skill ecosystems
- The task is to scan GitHub or awesome lists for skill best practices
- General research has already surfaced a likely skill-library improvement opportunity
- The deliverable is a **local skill-library gap analysis or enhancement proposal**, not direct file edits
- The question is not merely “how does this repo work”, but “what should our local skill library absorb from it”

## Do not use

- Generic research with no skill-library angle → use `$information-retrieval`
- Repo architecture, issue/PR history, or code evolution deep dives → use `$github-investigator`
- Direct Codex framework redesign or routing governance → use `$skill-developer-codex`
- One-skill wording or boundary rewriting → use `$skill-writer`
- Actual skill-file editing or packaging → use `$skill-creator`
- Post-task routing miss repair → use `$skill-routing-repair-codex`

## Core workflow

1. Define the local gap or target skill area.
2. Scout a bounded set of external sources.
3. Evaluate candidates on:
   - trigger precision
   - workflow completeness
   - boundary clarity
   - token efficiency
   - genuinely new ideas
4. Recommend one of three actions:
   - patch incumbent
   - extend references
   - create new skill
5. Include explicit boundary impact notes.
6. State why the result belongs to `skill-scout` instead of `$information-retrieval` or `$github-investigator`.

## Output defaults

```markdown
## Scout Report
- Topic: ...
- Sources: ...

### Gap Analysis
- ...

### Action Plan
- Patch / Extend / Create

### Boundary Impact
- ...
```

## Hard constraints

- Do not edit local skills directly.
- Do not propose a new skill when an incumbent-first patch is enough.
- Do not assume external content is better just because it is different.
- Keep scouting bounded; avoid runaway context consumption.

## Reference

- [references/detailed-guide.md](references/detailed-guide.md)
