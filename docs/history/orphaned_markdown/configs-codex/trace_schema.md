# Trace Schema

## Goal
- Record why a complex task routed the way it did.
- Preserve enough metadata to audit reroutes, retries, and produced artifacts.

## Canonical File
- `TRACE_METADATA.json`

## Required Keys
- `version`
- `ts`
- `task`
- `framework_version`
- `routing_runtime_version`
- `matched_skills`
- `decision.owner`
- `decision.gate`
- `decision.overlay`
- `reroute_count`
- `retry_count`
- `artifact_paths`
- `verification_status`

## Usage Rule
- Complex tasks must emit trace metadata before final sign-off.
- Artifact paths should point to stable files, not ephemeral chat context.
