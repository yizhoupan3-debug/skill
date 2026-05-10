#!/usr/bin/env bash
# Installs Codex CLI hooks into ~/.codex/{config.toml,hooks.json}.
# Invokes the Rust `router-rs` binary directly (no bash build shim required).
# JSON/TOML merge logic lives in scripts/router-rs/src/codex_hooks.rs.
set -euo pipefail

CODEX_HOME="${CODEX_HOME:-$HOME/.codex}"
SCRIPT_DIR="$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." && pwd -P)"
ROUTER_MANIFEST="$REPO_ROOT/scripts/router-rs/Cargo.toml"

printf 'Building router-rs (release)...\n' >&2
CARGO_TARGET_DIR="$REPO_ROOT/scripts/router-rs/target" cargo build --release --manifest-path "$ROUTER_MANIFEST"
ROUTER_BIN="$REPO_ROOT/scripts/router-rs/target/release/router-rs"

if [[ ! -x "$ROUTER_BIN" ]]; then
  printf 'router-rs binary missing or not executable: %s\n' "$ROUTER_BIN" >&2
  exit 1
fi

mkdir -p "$CODEX_HOME"

if ! "$ROUTER_BIN" \
  codex install-hooks \
  --codex-home "$CODEX_HOME" \
  --apply; then
  printf 'codex install-hooks failed (see stderr above)\n' >&2
  exit 1
fi

printf 'Installed codex-cli hooks into %s\n- %s\n- %s\n' \
  "$CODEX_HOME" \
  "$CODEX_HOME/config.toml" \
  "$CODEX_HOME/hooks.json"
