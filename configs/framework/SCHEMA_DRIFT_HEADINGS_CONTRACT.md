# Schema drift — task headings contract

**Owner**: `core/router-rs/src/schema_drift.rs`  
**Evidence protocol**: `skills/verifyx/references/evidence-protocol.md`  
**Verify skill**: `skills/verifyx/SKILL.md`

## Task artifact headings

For `artifacts/current/<task_id>/`, `REQUIREMENTS.md` and `ROADMAP.md` must expose the same `##` / `###` heading lines (SHA-256 of joined headings). Record baseline with:

```bash
cargo run --manifest-path core/router-rs/Cargo.toml -- schema-drift baseline --repo-root . --task-id <task_id>
```

## Post-verify purge

After `/verifyx` PASS and closeout write, remove `artifacts/current/<task_id>/` entirely. See `skills/verifyx/SKILL.md` § Post-verify task-dir purge.
