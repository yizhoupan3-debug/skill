# Deep Research Playbook

Use this playbook when the user asks for **仔细调研 / 深度调研 / 多轮调研 / 对标分析 / 方案调研** and the default one-pass search is too shallow.

## Phase 1 — Define the brief

Capture four fields before searching:

- **Goal**: what question must be answered
- **Decision**: what downstream action the research supports
- **Freshness**: how current the evidence must be
- **Deliverable**: memo, comparison table, ranked options, or concrete plan

## Phase 2 — Broad scan

Start broad to map the territory.

Checklist:
- identify 3–6 dimensions
- identify primary-source candidates
- identify terms, aliases, and competing labels
- identify where recency matters

Typical outputs:
- topic map
- comparison axes
- candidate repos / products / docs

## Phase 3 — Deep dive

Deep-read only the highest-value sources.

Preferred order:
1. official docs
2. source code / reference repos
3. issue threads / discussions
4. third-party analysis

For each dimension, try to collect:
- one primary source
- one corroborating source
- one counter-signal or limitation if it exists

## Phase 4 — Verification

Before concluding, test for false confidence.

Checklist:
- Are key claims double-sourced?
- Are dates explicit when recency matters?
- Are conflicting signals surfaced?
- Are unknowns clearly separated from conclusions?
- Is the recommendation tied to evidence instead of preference?

## Phase 5 — Synthesis

Minimum structure:

```markdown
## Retrieval Summary
- Goal:
- Scope:
- Freshness:
- Confidence:

### Key Findings
### Conflicts / Unknowns
### Recommendation
### Sources
```

## Escalate or reroute

Escalate to `$github-investigator` when any of these become central:
- repo architecture
- issue / PR history
- commit timeline
- contributor or release evolution

Escalate to `$skill-scout` when the real output is a local skill-library gap analysis.
