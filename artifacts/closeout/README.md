# Closeout records (local only)

`verifyx` / `router-rs closeout evaluate` writes **`artifacts/closeout/<task_id>.json`** here at ship time.

- **Default**: `*.json` in this directory is **gitignored** (see repo root `.gitignore`).
- **Do not commit** ephemeral closeout snapshots; keep evidence in task dirs under `artifacts/current/<task_id>/` until verify purge, or in operator notes.
- **Historical samples are not tracked in git**; the 2026-05 purge removed prior indexed JSON from the repository index.
- **Schema**: `configs/framework/CLOSEOUT_RECORD_SCHEMA.json`
- **Operator reference**: `docs/closeout_enforcement.md`
