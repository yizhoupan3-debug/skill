# Middleware Contracts

## Goal
- Move repeated cross-skill runtime behavior out of prose and into explicit contracts.

## Contract Set

### routing-middleware
- Input: task text, route map, gate rules
- Output: owner / gate / overlay decision

### memory-middleware
- Input: `AGENTS.md`, runtime overlay, stable user/project preferences
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
