# dist/

Optional packaged skill archives (`.skill` files). The old root-level packaging script is retired.

## Usage

- **Pack**: use the host's native export/import path when a `.skill` archive is required.
- **Install**: Use `router-rs --host-integration install-skills` (Antigravity) or `skill-installer` (Codex) to import `.skill` archives.

This directory is **not** used for routing or validation. Rust routing artifacts are generated from `skills/`.
