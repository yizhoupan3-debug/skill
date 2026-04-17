# Checklist Fixer — Priority Classification Reference

Reference table for auto-classifying fix items when priorities are not explicit.

## Severity Keywords → Priority Mapping

| Keywords | Priority |
|----------|----------|
| security, data loss, crash, vulnerability, CVE | P0 |
| bug, wrong behavior, regression, broken, failing test | P1 |
| code smell, duplication, missing validation, dead code | P2 |
| style, naming, documentation, optimization, cleanup | P3 |

## Commit Message Format

```
fix(#N): <description>
```

Where `N` is the checklist item number.

## Checkpoint Format

```markdown
<!-- checklist-fixer checkpoint -->
<!-- completed: 1,2,4 -->
<!-- failed: 3 -->
<!-- remaining: 5,6,7 -->
<!-- last-updated: ISO-8601 -->
```
