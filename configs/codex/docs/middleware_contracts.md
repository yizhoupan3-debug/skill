# Middleware Contracts

## Goal
- Move repeated cross-skill runtime behavior out of prose and into explicit contracts.

## Contract Set

### routing-middleware
- Input: task text, route map, gate rules
- Output: owner / gate / overlay decision

### memory-middleware
- Input: `AGENTS.md`, task-scoped continuity artifacts, stable user/project preferences
- Output: normalized memory context

### compression-middleware
- Input: verbose outputs, runtime evidence, artifact directory
- Output: `SESSION_SUMMARY.md`, `NEXT_ACTIONS.json`, `EVIDENCE_INDEX.json`

### approval-middleware
- Input: `skills/SKILL_APPROVAL_POLICY.json`, requested action
- Output: allow / request approval / deny

### checkpoint-middleware
- Input: task state and phase transitions
- Output: `.supervisor_state.json` updates and checkpoint markers

### bridge-continuity-middleware
- Input: thread key, sender binding, mobile completion rules
- Output: sticky-thread continuity and completion notifications

### contract-guard-middleware
- Input: live `framework contract-summary`, optional expected `contract_digest`, proposed owner/task/goal/evidence intent
- Output: compact `prompt_lines`, stable `contract_digest`, `drift_flags`, allow/block decision
- Drift classes: `scope_drift`, `owner_drift`, `evidence_drift`, `contract_digest_drift`
- Fail closed when: digest changes, owner changes, active task/goal changes, or evidence requirements are dropped without explicit contract update intent
- Auto-repair allowed: re-inject compact `prompt_lines`, point to current task artifacts, and preserve the live owner/phase
- Must ask/update explicitly: changing owner, widening scope, replacing the goal, or relaxing verification evidence
- Hook posture: Codex hooks stay disabled by default; `codex hook contract-guard` is an explicit opt-in audit command backed by Rust `framework contract-summary`
