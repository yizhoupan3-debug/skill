# Citation and Confidence for GitHub Investigation

Use this reference when the investigation result depends on repository evidence, issue threads, pull requests, or release history.

## Citation rule

For materially important claims, attach the source immediately after the claim when feasible.

Preferred order:
1. repository files or README
2. issues / PRs / commits / releases
3. external supporting commentary

## Confidence rubric

### High
- backed by first-party repository evidence
- corroborated by more than one repository source when needed
- recent enough for the question being answered

### Medium
- grounded in one strong repository source
- or supported by slightly stale evidence where direction still seems stable

### Low
- inferred from partial signals
- or dependent on outdated, sparse, or contradictory evidence

## Conflict handling

When repository signals disagree:
- state the disagreement directly
- prefer merged code and recent mainline history over abandoned discussion
- separate observed facts from inference

## Recommended section footer

```markdown
### Confidence Assessment
- High: claims directly supported by current repository evidence
- Medium: plausible claims with limited corroboration
- Low: open questions, weak signals, or unresolved conflicts
```
