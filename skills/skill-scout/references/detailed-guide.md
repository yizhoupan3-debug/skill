# skill-scout Detailed Guide

## Curated Source List

### Tier 1 — High-Signal Skill/Prompt Repositories

| Repository | Focus | URL |
|---|---|---|
| `VoltAgent/awesome-agent-skills` | 500+ agent skills, multi-platform | `https://github.com/VoltAgent/awesome-agent-skills` |
| `anthropics/skills` | Official Claude Code skills | `https://github.com/anthropics/skills` |
| `PatrickJS/awesome-cursorrules` | Cursor AI rules collection | `https://github.com/PatrickJS/awesome-cursorrules` |
| `CommandCodeAI/agent-skills` | Skills for coding agent workflows | `https://github.com/CommandCodeAI/agent-skills` |
| `heilcheng/awesome-agent-skills` | Curated AI agent skills + tutorials | `https://github.com/heilcheng/awesome-agent-skills` |
| `learnaiforlife/ai-agent-skills` | Production-ready Claude/Cursor skills | `https://github.com/learnaiforlife/ai-agent-skills` |

### Tier 2 — Prompt Engineering Collections

| Repository | Focus | URL |
|---|---|---|
| `dair-ai/Prompt-Engineering-Guide` | Comprehensive prompt engineering | `https://github.com/dair-ai/Prompt-Engineering-Guide` |
| `promptslab/Awesome-Prompt-Engineering` | Papers, tools, benchmarks | `https://github.com/promptslab/Awesome-Prompt-Engineering` |
| `f/awesome-chatgpt-prompts` | Community prompt collection | `https://github.com/f/awesome-chatgpt-prompts` |

### Tier 3 — Agent Architecture References

| Repository | Focus | URL |
|---|---|---|
| `e2b-dev/awesome-ai-agents` | Open-source AI agent projects | `https://github.com/e2b-dev/awesome-ai-agents` |
| OpenAI Codex docs | Official Codex skill specification | `https://openai.com/codex` |
| `agents.md` standard | AGENTS.md open standard | `https://agents.md` |

### Discovery Strategies

When the curated list is insufficient, use these search patterns:

```
# GitHub repository search
query: "agent skills" OR "cursor rules" OR "AGENTS.md" OR "SKILL.md"
query: "awesome" + [domain keyword] + "AI agent"

# GitHub code search
query: "routing_layer" OR "routing_owner" language:markdown
query: "## When to use" "## Do not use" language:markdown

# Web search
query: "best AI coding agent skills [year]"
query: "[specific domain] agent skill GitHub"
```

## Gap-Analysis Matrix Template

Use this template when evaluating external findings:

```markdown
| # | External Item | Source | Local Counterpart | Trigger | Workflow | Boundary | Token | Novel | Score | Verdict |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | [name] | [repo/URL] | [skill-name] | [++/+/=/−] | [++/+/=/−] | [++/+/=/−] | [++/+/=/−] | [++/+/=/−] | [sum] | [Patch/Extend/Create/Skip] |
```

**Scoring guide**:
- `++` = Significantly better, clear actionable improvement (2 points)
- `+`  = Marginally better, worth considering (1 point)
- `=`  = Equivalent, no action needed (0 points)
- `−`  = Worse than local version (−1 point)

**Verdict thresholds**:
- Score ≥ 5: Strong candidate for absorption
- Score 3-4: Worth considering, evaluate effort vs. impact
- Score ≤ 2: Skip unless a single dimension is `++` with high impact

## Boundary Impact Checklist

For each proposed change, verify:

- [ ] Does this change affect the `description` trigger surface of any adjacent skill?
- [ ] Does this change require updating any skill's `## Do not use` section?
- [ ] Does this change require a new entry or modification in `SKILL_ROUTING_INDEX.md`?
- [ ] Does this change affect `SKILL_ROUTING_LAYERS.md` (layer, overlap, or confusion entries)?
- [ ] Could this change cause routing ambiguity between two skills?
- [ ] Is the incumbent-first principle respected (patch before create)?

## Action Plan Template

```markdown
## Action Plan: [topic] Scout

### Priority Legend
- P1: High impact, low effort — do immediately
- P2: High impact, moderate effort — plan next
- P3: Low impact, any effort — backlog

### Patch Actions (strengthen existing skills)

| # | Target Skill | Change | Priority | Boundary Impact |
|---|---|---|---|---|
| 1 | [skill-name] | [description] | P1/P2/P3 | [none / affects X] |

### Extend Actions (add references or sub-resources)

| # | Target Skill | New Resource | Priority | Boundary Impact |
|---|---|---|---|---|
| 1 | [skill-name] | [file path] | P1/P2/P3 | [none / affects X] |

### Create Actions (propose new skills — handoff to skill-framework-developer)

| # | Proposed Name | Brief | Priority | Replaces/Extends |
|---|---|---|---|---|
| 1 | [name] | [one-line] | P1/P2/P3 | [none / extends X] |
```
