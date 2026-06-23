# Schema drift — task headings contract

**Owner**: `core/runtime-core/src/schema_drift.rs`  
**Evidence protocol**: 项目通用 evidence 协议（见 `docs/` 目录）  
**Verification**: 项目通用验证流程

## Task artifact headings

For `artifacts/current/<task_id>/`, `REQUIREMENTS.md` and `ROADMAP.md` must expose the same `##` / `###` heading lines (SHA-256 of joined headings). Record baseline with:

```bash
cargo run --manifest-path core/router-rs/Cargo.toml -- schema-drift baseline --repo-root . --task-id <task_id>
```

## Post-verify purge

After verification PASS and closeout write, remove `artifacts/current/<task_id>/` entirely. See verification skill documentation for task-dir purge details.
