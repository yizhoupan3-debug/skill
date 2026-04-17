# Source Scoring

Use this quick scoring model to avoid overweighting weak sources.

## Source tiers

| Tier | Source type | Default weight | Notes |
|---|---|---:|---|
| T1 | Official docs, standards, vendor pages, first-party announcements | 4 | Preferred source of truth |
| T2 | Source code, reference repos, release notes, issue threads by maintainers | 3 | Strong for implementation reality |
| T3 | Reputable third-party technical analysis | 2 | Useful, but verify important claims |
| T4 | Aggregators, generic blogs, low-context summaries | 1 | Use only for discovery, not proof |

## Confidence rules

- **High**: Key conclusions rely mostly on T1/T2 sources and have corroboration
- **Medium**: Useful evidence exists, but some important claims rely on one source or stale material
- **Low**: Evidence is sparse, contradictory, or mostly T3/T4

## Conflict handling

When sources disagree:
1. prefer the more primary source
2. prefer the more recent source when the topic is time-sensitive
3. report the conflict explicitly instead of silently choosing one side

## Citation hygiene

For recommendations that may cost substantial time or money, cite the exact source set used.
