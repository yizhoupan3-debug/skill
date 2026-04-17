# Eval Suite

## Required Eval Tracks

### routing_accuracy
- Does the task hit the correct owner/gate/overlay?
- Can the route be explained from artifacts alone?

### token_efficiency
- Did new runtime capabilities reduce or inflate context cost?
- Are long outputs externalized to artifacts instead of main-thread dumps?

### long_task_continuity
- Can a mobile / bridge follow-up resume from `SESSION_SUMMARY.md`, `EVIDENCE_INDEX.json`, and `TRACE_METADATA.json`?
- Does completion behavior remain consistent across desktop and bridge?

## Initial Success Criteria
- No regression in owner precision.
- No uncontrolled token growth.
- Long tasks resume without rereading the entire chat.
