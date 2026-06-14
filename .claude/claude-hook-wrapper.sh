#!/usr/bin/env bash
# Fallback hook dispatch — only used if the Rust binary is missing.
# Tries PATH, then common locations.
set -euo pipefail
BIN="${HOME}/.local/bin/router-rs"
[[ -x "$BIN" ]] || BIN="$(command -v router-rs 2>/dev/null || true)"
[[ -x "$BIN" ]] || BIN="/tmp/skill-cargo-target/release/router-rs-cli"
[[ -x "$BIN" ]] || { echo '{"decision":"allow","reason":"router-rs binary not found","suppressOutput":true}'; exit 0; }
exec "$BIN" host claude claude-hook "$@"
