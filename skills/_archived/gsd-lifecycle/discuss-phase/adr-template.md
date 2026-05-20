# ADR Template

Use this template for architecture decisions.

## Full ADR Template

```markdown
# ADR-XXX: <Title>

> Replace XXX with next sequential number

## Status

**Proposed** | Accepted | Deprecated | Superseded

*Use "Proposed" for decisions under review. Update to "Accepted" after approval.*

## Date

<YYYY-MM-DD>

## Context

### Problem Statement
What problem are we solving?

### Constraints
What constraints affect this decision?
- Technical constraints
- Business constraints
- Time constraints
- Team constraints

### Decision Drivers
What factors are driving this decision?
1. Driver 1
2. Driver 2
3. Driver 3

## Decision

### Outcome
What have we decided to do?

### Rationale
Why did we make this decision?

## Consequences

### Positive
What benefits does this decision bring?
- Benefit 1
- Benefit 2

### Negative
What downsides does this decision bring?
- Downside 1
- Downside 2

### Neutral
What are the neutral consequences?
- Effect 1

## Alternatives Considered

### Option A: <Name>
**Decision**: Chose this / Did not choose this

**Pros**:
- Pro 1
- Pro 2

**Cons**:
- Con 1
- Con 2

**Why not**: <Reason>

### Option B: <Name>
**Decision**: Chose this / Did not choose this

...

## Implementation Plan

How will we implement this decision?

### Phase 1: <Name>
- [ ] Task 1
- [ ] Task 2

### Phase 2: <Name>
- [ ] Task 3
- [ ] Task 4

## Review Schedule

When should this decision be reviewed?

- **Review date**: <YYYY-MM-DD>
- **Review trigger**: <Event that triggers review>
- **Reviewer**: <Who owns the review>

## Related ADRs

- [ADR-001](ADR-001.md) - Related decision
- [ADR-002](ADR-002.md) - Another related
```

## ADR Examples

### Example 1: Technology Choice

```markdown
# ADR-001: Use PostgreSQL for Primary Database

## Status
Accepted

## Date
2026-05-19

## Context
We need to choose a database for storing user data and application state.

Constraints:
- Must support JSON columns
- Must have good Rust driver support
- Must handle 10K concurrent connections

## Decision
Use **PostgreSQL 15** as the primary database.

## Consequences

### Positive
- Excellent JSON support with JSONB
- Battle-tested reliability
- Great Rust driver (tokio-postgres, sqlx)

### Negative
- Requires separate installation
- More complex than SQLite

### Neutral
- Standard industry choice

## Alternatives Considered

### Option A: SQLite
**Decision**: Did not choose

**Pros**: Zero config, fast for single-user

**Cons**: Limited concurrency, no true JSONB

**Why not**: Doesn't meet 10K concurrent connection requirement

### Option B: DynamoDB
**Decision**: Did not choose

**Pros**: Managed, auto-scaling

**Cons**: Expensive at scale, vendor lock-in

**Why not**: Cost and lock-in concerns

## Review Schedule
Review in 6 months or when scaling issues arise.
```

### Example 2: Architecture Pattern

```markdown
# ADR-002: Use Repository Pattern for Data Access

## Status
Accepted

## Context
We need to structure data access layer for testability and flexibility.

## Decision
Use Repository Pattern with trait-based abstractions.

## Consequences

### Positive
- Easy to mock in tests
- Can swap implementations
- Clear separation of concerns

### Negative
- More abstraction layers
- Requires async trait support

## Review Schedule
Review after first major refactor cycle.
```
