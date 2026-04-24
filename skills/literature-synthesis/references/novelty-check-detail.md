# Novelty Check Detail

Extracted from the main `literature-synthesis` SKILL.md to keep the main file
focused on routing and workflow overview.

## Idea Novelty Check — Full Protocol

### Phase 1: Claim Extraction

Extract checkable novelty claims from the user's input:
- Decompose the idea into 3-7 atomic claims
- For each claim, identify:
  - the novelty axis (method / task / setting / combination / framing)
  - the specificity level (vague direction vs testable hypothesis)
  - required evidence to validate novelty

Format claims as a numbered list:
- `C1. [method] Using X architecture for Y task — novelty axis: task transfer`
- `C2. [combination] Combining A loss with B regularization — novelty axis: novel combination`
- `C3. [setting] Applying method M to domain D under constraint Z — novelty axis: new setting`

### Phase 2: Systematic Search

For each claim, search for prior work systematically.

**Search strategy** (broadest to narrowest):
1. Core keywords → scan top-20 results for direct overlap
2. Method-component keywords → find papers using the same building blocks
3. Task/domain keywords + time filter (last 3 years) → find recent competitors
4. Combination search → find papers combining similar components

**Recommended search sources** (in priority order):
1. Semantic Scholar API (`api.semanticscholar.org`) — best for citation graphs and abstract search
2. Google Scholar — broadest coverage, useful for cross-disciplinary
3. arXiv search — latest preprints, essential for fast-moving fields
4. Connected Papers / Litmaps — for citation-network discovery

When the agent has web access, prefer API-based search. When it does not,
explicitly ask the user to provide search results or paper lists, and proceed
with what is available.

This workflow does not require API access to be useful. The structured
claim-by-claim approach improves novelty judgment even when searching is done
manually or from user-provided paper lists.

### Phase 3: Claim-by-Claim Comparison

For each claim, compare against the search results:

| Claim | Closest Prior Work | Overlap Level | What's Different | Novelty Risk |
|---|---|---|---|---|
| C1 | [Paper X, Year] | High / Medium / Low | [specifics] | 🔴 / 🟡 / 🟢 |
| C2 | ... | ... | ... | ... |

Overlap levels:
- **High** (🔴): a published paper addresses the same claim in a comparable setting
- **Medium** (🟡): partial overlap — same method but different task, or same task but different approach
- **Low** (🟢): no close match found in the search results

### Phase 4: Novelty Scoring Matrix

Produce a summary matrix:

| Claim | Novelty Axis | Overlap | Confidence | Verdict |
|---|---|---|---|---|
| C1 | method | 🟡 medium | high | Defensible with differentiation |
| C2 | combination | 🟢 low | medium | Likely novel, verify with [search X] |
| C3 | setting | 🔴 high | high | Not novel — [Paper Y] covers this |

Verdicts:
- **Novel**: no close match, high confidence → safe to claim
- **Defensible**: overlap exists but clear differentiation → claim with careful positioning
- **Risky**: high overlap, novelty claim requires strong justification
- **Not novel**: direct prior work found → do not claim, reframe or drop

### Phase 5: Novelty Risk Report

Deliver a structured report:

1. **Overall novelty assessment**: strong / moderate / weak / insufficient
2. **Strongest novel claims**: ranked list
3. **Claims to drop or reframe**: with specific reason
4. **Verification gaps**: searches that still need to be done
5. **Recommended positioning strategy**: how to frame the contribution to maximize defensibility
6. **Baselines that must be compared**: papers that reviewers will expect as baselines
7. **Search directions still open**: queries or databases not yet checked
