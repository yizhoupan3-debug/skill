# Stdio Contracts

All interaction with router-rs framework happens via stdio JSON.

## Goal Operations

```bash
# Start goal
printf '%s\n' '{"id":1,"op":"framework_autopilot_goal","payload":{"operation":"start","repo_root":"<path>","goal":"<goal>","non_goals":["<ng1>"],"done_when":["<dw1>"],"validation_commands":["<vc1>"],"drive_until_done":true}}' | router-rs --stdio-json

# Checkpoint
printf '%s\n' '{"id":2,"op":"framework_autopilot_goal","payload":{"operation":"checkpoint","repo_root":"<path>","note":"<note>"}}' | router-rs --stdio-json

# Complete
printf '%s\n' '{"id":3,"op":"framework_autopilot_goal","payload":{"operation":"complete","repo_root":"<path>"}}' | router-rs --stdio-json

# Status
printf '%s\n' '{"id":4,"op":"framework_autopilot_goal","payload":{"operation":"status","repo_root":"<path>"}}' | router-rs --stdio-json
```

## RFV Loop Operations

```bash
# Start RFV loop
printf '%s\n' '{"id":5,"op":"framework_rfv_loop","payload":{"operation":"start","repo_root":"<path>","goal":"<goal>","max_rounds":3,"allow_external_research":true,"review_scope":"<scope>","fix_scope":"<scope>","verify_commands":["<cmd>"],"stop_when":["<condition>"]}}' | router-rs --stdio-json

# Append round
printf '%s\n' '{"id":6,"op":"framework_rfv_loop","payload":{"operation":"append_round","repo_root":"<path>","round":1,"review_summary":"<summary>","fix_summary":"<summary>","verify_result":"PASS|FAIL|SKIPPED","supervisor_decision":"continue|close|block"}}' | router-rs --stdio-json

# Status
printf '%s\n' '{"id":7,"op":"framework_rfv_loop","payload":{"operation":"status","repo_root":"<path>"}}' | router-rs --stdio-json
```

## Evidence Operations

```bash
# Append evidence
printf '%s\n' '{"id":8,"op":"framework_hook_evidence_append","payload":{"repo_root":"<path>","command_preview":"<cmd>","result":"<result>","kind":"manual_verification"}}' | router-rs --stdio-json
```

## Closeout Operations

```bash
# Evaluate closeout
printf '%s\n' '{"id":9,"op":"framework_closeout_evaluate","payload":{"repo_root":"<path>","task_id":"<task_id>","verification_status":"<status>","commands_run":[{"command":"<cmd>","exit_code":0}]}}' | router-rs --stdio-json
```

## Task State Operations

```bash
# Write session summary
printf '%s\n' '{"id":10,"op":"framework_session_summary_write","payload":{"repo_root":"<path>","content":"<markdown>"}}' | router-rs --stdio-json

# Read continuity
printf '%s\n' '{"id":11,"op":"framework_continuity_digest","payload":{"repo_root":"<path>"}}' | router-rs --stdio-json
```
