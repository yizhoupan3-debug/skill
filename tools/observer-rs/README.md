# observer-rs

Telemetry observation & analysis tool. Reads the telemetry journal and produces
actionable analysis outputs for framework maintainers.

## Commands

### `analyze`
Reads `artifacts/telemetry/events.jsonl`, applies `--days` window filtering, and
writes `artifacts/observer/analysis.json` with aggregate metrics and
recommendations.

### `audit`
Analyzes the telemetry journal and suggests repairs or new skills based on
pattern matching and Jaccard similarity.

### `manifest`
Emits registry / usage snapshots (journal-driven; not a live skill health score
for routing).

### `health-score`
Computes per-skill health scores from telemetry events.

### `inspect`
SHA-256 integrity check of a skill directory.

### `sync`
Synchronizes journal entries to a Markdown feedback table with deduplication.

### `snapshot`
Creates a versioned snapshot of the current skill registry and manifest.

## Output paths

| Output | Path |
|---|---|
| Analysis report | `artifacts/observer/analysis.json` |
| Health scores | `artifacts/observer/health-score.json` |
| Online alerts | `artifacts/observer/alerts.jsonl` |

## Architecture
- **`serde`**: JSON serialization.
- **`sha2`**: Content-addressed hashing for deduplication.
