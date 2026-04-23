---
name: github-investigator
description: |
  GitHub 仓库深度调研：仓库拆解、repo对标、issue/PR 时间线、code history 与可复用模式分析。
  Use when the answer depends on repository structure, discussions, commits, releases, or architecture evolution.
short_description: Deep GitHub repo research with issue/PR timeline and code-history evidence
trigger_hints:
  - github深度调研
  - repo对标
  - 仓库拆解
  - issue PR 时间线
  - code history
  - 调研开源项目
  - 看看这个仓库怎么做的
  - GitHub 仓库分析
  - 开源项目架构
  - repo investigation
  - open source analysis
metadata:
  version: "1.2.0"
  platforms: [codex, antigravity]
  tags:
    - github
    - mcp
    - repo-research
    - timeline
    - architecture
risk: low
source: local
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
allowed_tools:
  - github
  - web
approval_required_tools: []
filesystem_scope:
  - repo
  - artifacts
network_access: required
artifact_outputs:
  - runtime_evidence.md
  - EVIDENCE_INDEX.json
bridge_behavior: mobile_complete_once
---
# github-investigator

This skill owns **deep GitHub research** when repository structure, issue and PR
evolution, or code-history evidence are central to the answer.

## When to use

- The user asks for **github深度调研 / repo对标 / 仓库拆解 / 看这个开源项目怎么做的**
- The task requires architectural insight from an external repository
- The task requires reading issues, PRs, commits, releases, or contributor activity to understand decisions
- The answer depends on timeline reconstruction, reusable implementation patterns, or project evolution
- Searching for specific code implementations across GitHub repositories is part of the investigation

## Do not use

- General web research with no repository-history center of gravity → use `$information-retrieval`
- External skill-ecosystem benchmarking for local skill improvement → use `$skill-scout`
- Triaging local git status or local branch safety → use `$gitx`
- Directly resolving CI failures without research → use `$gh-fix-ci`
- Only responding to PR review comments → use `$gh-address-comments`

## Core workflow

1. Clarify the investigation brief:
   - target repo or search scope
   - architecture, timeline, issues, PRs, or patterns
   - desired deliverable
2. Build a **repo baseline**:
   - repo summary
   - README and top-level tree
   - default branch, language mix, and major modules
3. Run an **architecture pass**:
   - inspect key directories and entrypoints
   - identify main subsystems and tool boundaries
   - note reusable patterns and local applicability
4. Run an **evolution pass**:
   - inspect commits, issues, PRs, releases, or tags as needed
   - reconstruct the relevant timeline with concrete dates
   - separate active direction from stale artifacts
5. Synthesize a research memo:
   - executive summary
   - architecture findings
   - timeline or evolution notes
   - reusable patterns, risks, and recommendation
   - citations and confidence aligned to evidence quality

## Output defaults

```markdown
## GitHub Investigation Summary
- Target: ...
- Scope: ...
- Confidence: High / Medium / Low

### Executive Summary
- ...

### Architecture
- ...

### Timeline / Evolution
- ...

### Reusable Patterns
- ...

### Risks / Unknowns
- ...

### Sources
- ...

### Confidence Assessment
- High: ...
- Medium: ...
- Low: ...
```

## Hard constraints

- Prefer first-party repository evidence over third-party summaries.
- Use explicit dates when describing repository evolution.
- Distinguish current mainline behavior from historical experiments.
- Do not stop at README when issues, PRs, commits, or releases are central.
- Attach citations immediately after externally derived claims when feasible.
- Confidence must reflect source quality, recency, and corroboration rather than presentation polish.

## References

- [references/repo-research-template.md](references/repo-research-template.md)
- [references/citation-confidence.md](references/citation-confidence.md)
