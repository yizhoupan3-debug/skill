# evolution-rs

High-performance Rust core for the Codex Evolution Engine.

## Commands

### `audit`
Analyzes the evolution journal and suggests repairs or new skills based on pattern matching and Jaccard similarity.

### `manifest`
Generates a blended health manifest combining static scores and dynamic usage data (reroute rate, frequency).

### `sync`
Synchronizes journal entries to a Markdown feedback table with intelligent deduplication.

### `snapshot`
Creates a versioned snapshot of the current skill registry and manifest.

### `heal` (Dry-run supported)
Automatically prunes zero-usage skills and archives them to `.backups/pruned`.

## Architecture
This core is designed for maximum throughput using:
- **`memmap2`**: Zero-copy file access.
- **`rayon`**: Parallel processing of JSONL entries.
- **`serde`**: Highly optimized JSON serialization.
