---
name: gsd-execute-phase
description: |
  Execute all phases in waves with multi-agent pattern. Main thread lightweight (≤40% context).
  One-breath execution: don't ask user at every step, execute through waves autonomously.
  Use after /gsd-plan-phase. Spawns parallel subagents per wave, maintains state in WAVE_STATE.json.
routing_layer: L1
routing_owner: owner
routing_gate: evidence
routing_gate_evidence: "ROADMAP.md exists, WAVE_STATE.json exists"
routing_priority: P1
session_start: n/a
user-invocable: true
trigger_hints:
  - /gsd-execute-phase
  - gsd execute
  - gsd run
  - execute phases
  - run waves
metadata:
  version: "0.1.0"
  platforms: [supported]
  tags: [gsd, execution, multi-agent, waves]
---

# gsd-execute-phase

Execute all phases in waves with one-breath, multi-agent pattern.

## Philosophy

| Principle | Description |
|-----------|-------------|
| Main thread lightweight | Only coordination, ≤40% context |
| Subagent dense | Spawn subagents for specific tasks |
| One-breath | Don't ask user at every step |
| Evidence driven | Every verification → EVIDENCE_INDEX |
| Checkpoint at boundaries | State persists at each wave end |

## Pre-Conditions

1. `ROADMAP.md` exists
2. `WAVE_STATE.json` exists with wave plan
3. `GOAL_STATE.json` exists
4. Task directory is writable

## Execution Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                    Main Thread (Coordinator)                     │
│                    Context Budget: ≤40%                         │
│                         │                                        │
│         ┌───────────────┼───────────────┐                        │
│         ▼               ▼               ▼                        │
│   ┌──────────┐   ┌──────────┐   ┌──────────┐                   │
│   │ Wave 1   │   │ Wave 1   │   │ Wave 1   │                   │
│   │ Phase 1  │   │ Phase 2  │   │ Phase 3  │                   │
│   │ Agent    │   │ Agent    │   │ Agent    │                   │
│   └──────────┘   └──────────┘   └──────────┘                   │
│         │               │               │                        │
│         └───────────────┴───────────────┘                        │
│                         │                                        │
│                    Wave 1 Complete                               │
│                         │                                        │
│         ┌───────────────┼───────────────┐                        │
│         ▼               ▼               ▼                        │
│   ┌──────────┐   ┌──────────┐   ┌──────────┐                   │
│   │ Wave 2   │   │ Wave 2   │   │ Wave 2   │                   │
│   │ Phase 4  │   │ Phase 5  │   │ Phase 6  │                   │
│   └──────────┘   └──────────┘   └──────────┘                   │
│                         │                                        │
│                    Wave 2 Complete                               │
│                         │                                        │
│                    ┌──────────┐                                  │
│                    │ Wave 3   │                                  │
│                    │Testing   │                                  │
│                    │ Docs     │                                  │
│                    └──────────┘                                  │
│                         │                                        │
│                    All Complete                                   │
└─────────────────────────────────────────────────────────────────┘
```

## Agent Contract

### Spawn Agent

Each spawned agent must receive:

```json
{
  "task_description": "Specific task goal",
  "scope": ["allowed/files", "allowed/paths"],
  "forbidden": ["forbidden/paths"],
  "output_format": "json|markdown",
  "checkpoint_interval": 5,
  "max_context_usage": 20,
  "verification_commands": ["cmd1", "cmd2"]
}
```

### Agent Response

Each agent must return:

```json
{
  "agent_id": "unique-id",
  "changed_files": ["file1", "file2"],
  "verification_results": [
    {"command": "cargo test", "exit_code": 0, "output": "..."}
  ],
  "findings": [
    {"severity": "P2", "description": "..."}
  ],
  "next_action": "suggested next step",
  "context_used": 15
}
```

## One-Breath Execution Rules

### CAN Continue (don't ask user):
- Next subagent in current wave can start
- Merge conflict exists but is resolvable
- Verification failed but fix path is clear
- Technical issue has alternative approach
- Waiting for dependency (automatic retry)

### MUST Stop (ask user):
- Scope change or requirement error discovered
- Business decision or technical choice needed
- External dependency unavailable (API down, permission)
- P0/P1 security issue discovered
- Wave boundary reached (auto-pause before next wave)
- Context budget ≥35% (near limit)

### Emergency Stop:
- 3 consecutive retries failed
- Unrecoverable error
- User explicitly requests stop

## Checkpoint Protocol

At each checkpoint:
1. Update WAVE_STATE.json
2. Write evidence to EVIDENCE_INDEX.json
3. Update SESSION_SUMMARY.md
4. Verify GOAL_STATE.json still valid

```bash
# Update wave state
printf '%s\n' '{"id":1,"op":"framework_autopilot_goal","payload":{"operation":"checkpoint","repo_root":"<path>","note":"wave-1-complete phase-1-3 done"}}' | router-rs --stdio-json

# Write evidence
printf '%s\n' '{"id":2,"op":"framework_hook_evidence_append","payload":{"repo_root":"<path>","command_preview":"cargo test","result":"passed","kind":"gsd-execute-phase"}}' | router-rs --stdio-json
```

## Resume Mechanism

If execution is interrupted:

1. Check WAVE_STATE.json for current_wave
2. Verify goal still valid (GOAL_STATE.json)
3. Read SESSION_SUMMARY.md for context
4. Resume from current checkpoint

## Main Thread Responsibilities

- Wave orchestration and sequencing
- Agent spawn and monitoring
- Cross-agent conflict resolution
- Checkpoint management
- Result aggregation and summary
- User communication (at stop points)

## Anti-Patterns

- Don't execute work yourself (use agents)
- Don't ask user at every subagent completion
- Don't exceed context budget
- Don't skip checkpoints
- Don't proceed past blockers without resolution

## Next Step

After all waves complete, proceed to `/gsd-verify-work`.

---

## ⚠️ Desktop MCP Self-Discipline Reminders

> **Required**: Desktop MCP cannot auto-record evidence. Manual action needed.

### During Execution

1. **After each verification** → Call `record_evidence`
   ```
   record_evidence command="cargo test" result="pass"
   ```

2. **At wave boundaries** → Call `session_checkpoint`

3. **Before stopping** → Update `WAVE_STATE.json`

### Quick Reference

```bash
# Record evidence
record_evidence command="cargo test" result="pass"

# Checkpoint
session_checkpoint
goal_state_manage operation=checkpoint

# Update state
jq ".current_wave = 2" WAVE_STATE.json > tmp.json && mv tmp.json WAVE_STATE.json
```

See `shared/desktop-mcp-self-discipline.md` for full checklist.

