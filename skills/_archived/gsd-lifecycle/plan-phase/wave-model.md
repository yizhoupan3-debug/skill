# Wave Execution Model

Waves group phases for parallel execution and manageable complexity.

## Wave Principles

1. **Dependencies respected**: Within a wave, phases should have minimal dependencies
2. **Risk balanced**: High-risk items earlier in the project timeline
3. **Value delivery**: Early wins build confidence
4. **Context coherent**: Agents in same wave share context

## Wave Anatomy

```yaml
wave_id: 1
phases: [phase-1, phase-2]
status: pending|running|completed|blocked
started_at: ISO8601
completed_at: ISO8601|null
agents:
  - agent_id: "wave-1-agent-1"
    assigned_phase: "phase-1"
    status: pending|running|completed|failed
    context_usage: 20
  - agent_id: "wave-1-agent-2"
    assigned_phase: "phase-2"
    status: pending|running|completed|failed
    context_usage: 20
checkpoint:
  last_checkpoint: "phase-1-checkpoint-1"
  evidence_files: ["path/to/evidence"]
```

## Wave Execution Rules

1. **Wave boundary**: No cross-wave dependencies
2. **Parallel within wave**: Agents can run in parallel if scopes are disjoint
3. **Sequential between waves**: Wave N+1 starts only after Wave N completes
4. **Checkpoint at wave end**: Write checkpoint before advancing

## Agent Assignment

**Within Wave**:
- One agent per phase (or per module within phase)
- Agents receive disjoint scopes
- Agents report to main thread

**Context Budget**:
- Main thread: ≤40%
- Each agent: ≤20%
- Shared resources: ≤20%

## Wave State Machine

```
┌─────────┐   start wave   ┌────────────┐   all phases done   ┌────────────┐
│ PENDING │ ────────────▶ │  RUNNING   │ ──────────────────▶ │ COMPLETED  │
└─────────┘               └────────────┘                     └────────────┘
                               │
                               │ blocker found
                               ▼
                          ┌─────────┐
                          │ BLOCKED │
                          └─────────┘
                               │
                               │ resolved
                               ▼
                          ┌────────────┐
                          │  RUNNING   │
                          └────────────┘
```

## Checkpoint Protocol

At each checkpoint:
1. Write current state to WAVE_STATE.json
2. Write evidence to EVIDENCE_INDEX.json
3. Write summary to SESSION_SUMMARY.md
4. Main thread updates orchestration state

## Wave Handoff

When Wave N completes:
1. Verify all phase exit criteria
2. Merge any conflicts from parallel work
3. Update WAVE_STATE.json
4. Write integration checkpoint
5. Start Wave N+1 agents

## Example Wave Plan

```yaml
# For a web application

Wave 1:
  phases: [infrastructure, data-model, api-core]
  agents: 3
  expected_duration: 2-3 hours
  exit_criteria:
    - cargo test --lib passes
    - schema migrations validated
    - API endpoints return 200

Wave 2:
  phases: [frontend-core, business-logic, integration]
  agents: 3
  expected_duration: 3-4 hours
  exit_criteria:
    - integration tests pass
    - E2E smoke tests pass

Wave 3:
  phases: [testing, documentation, polish]
  agents: 2
  expected_duration: 1-2 hours
  exit_criteria:
    - coverage ≥ 80%
    - docs build clean
    - clippy passes
```
