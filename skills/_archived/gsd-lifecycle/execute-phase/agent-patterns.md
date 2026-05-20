# Agent Patterns

Patterns for effective subagent orchestration.

## Agent Types

### Implementation Agent

**Purpose**: Implement specific feature or module

```yaml
type: implementation
scope: specific_module/
output: code + tests
context_budget: 20%
```

### Verification Agent

**Purpose**: Run verification commands

```yaml
type: verification
scope: all/
output: test_results
context_budget: 10%
```

### Documentation Agent

**Purpose**: Update docs

```yaml
type: documentation
scope: docs/
output: updated_docs
context_budget: 10%
```

### Review Agent

**Purpose**: Code review

```yaml
type: review
scope: changed_files/
output: findings
context_budget: 15%
```

## Agent Spawn Patterns

### Pattern 1: Parallel Same-Phase

Spawn multiple agents for different modules in same phase:

```
Phase 2 (Core Features):
├── Agent A: User authentication
├── Agent B: Product catalog
└── Agent C: Shopping cart

All spawned simultaneously, run in parallel.
```

### Pattern 2: Sequential Dependent

Spawn agents for dependent tasks:

```
Phase 3 (Integration):
├── Agent A: API integration (wait for Phase 2 agents)
└── Agent B: UI integration (wait for Agent A)

Sequential because B depends on A.
```

### Pattern 3: Wave-First

Complete entire wave before next:

```
Wave 1:
├── Phase 1 (Infrastructure)
└── Phase 2 (Core)

Wave 2:
├── Phase 3 (Integration)
└── Phase 4 (Polish)

Wave N+1 starts only after Wave N completes.
```

## Context Budget Allocation

### Recommended Distribution

| Role | Budget | Usage |
|------|--------|-------|
| Main thread | ≤40% | Coordination, monitoring, aggregation |
| Agent 1 | ≤20% | Phase 1 implementation |
| Agent 2 | ≤20% | Phase 2 implementation |
| Agent 3 | ≤20% | Phase 3 implementation |
| Shared resources | ≤20% | Architecture docs, contracts |

### Context Monitoring

Check context usage at:
- Each agent spawn
- Each checkpoint
- At 50% token budget warning

If approaching limit:
1. Pause spawning new agents
2. Aggregate current results
3. Write checkpoint
4. Notify user of context pressure

## Agent Communication

### Via Artifacts

Agents communicate through files:

```
artifacts/current/<task_id>/
├── wave-partial/
│   ├── wave-1/
│   │   ├── agent-a-results.json
│   │   └── agent-b-results.json
│   └── wave-2/
│       └── ...
└── wave-aggregated/
    └── wave-1-summary.json
```

### Main Thread Aggregation

1. Read all agent results from wave-partial/
2. Identify conflicts
3. Resolve or flag for user
4. Write aggregated summary
5. Update WAVE_STATE.json

## Error Handling

### Agent Failure

```
Agent fails → Check if recoverable
├── Yes → Retry with same agent
├── No → Checkpoint → Notify user
└── Dependency blocked → Update wave state → Wait
```

### Context Overflow

```
Context ≥ 85% → Emergency checkpoint
├── Write all state
├── Summarize results
├── Notify user
└── Wait for context to clear
```

## Agent Lifecycle

```yaml
spawned:
  timestamp: ISO8601
  assigned_scope: [paths]
  context_allocated: 20%

running:
  checkpoint_1: {timestamp, state}
  checkpoint_2: {timestamp, state}

completed:
  timestamp: ISO8601
  changed_files: [files]
  verification: {passed|failed}
  findings: [issues]

failed:
  timestamp: ISO8601
  reason: string
  recoverable: boolean
```
