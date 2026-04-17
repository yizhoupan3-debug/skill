# dist/

Packaged skill archives (`.skill` files) produced by `scripts/package_skill.py`.

## Usage

- **Pack**: `python3 scripts/package_skill.py <skill-dir>` → creates `<name>.skill` in this directory.
- **Install**: Use `scripts/install_skills.sh` (Antigravity) or `skill-installer` (Codex) to import `.skill` archives.

This directory is **not** used for routing or validation. Files here are ignored by `check_skills.py` and `sync_skills.py`.
