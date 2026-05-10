#!/usr/bin/env bash
set -euo pipefail
# Deprecated launcher (bash build/cache shim). Prefer the Rust binary on PATH:
#   cargo install --path "$(cd "$(dirname "$0")" && pwd)" --locked --force
#   router-rs self install
if [[ "${1:-}" == */Cargo.toml ]]; then
  shift
fi
if command -v router-rs >/dev/null 2>&1; then
  exec router-rs "$@"
fi
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ -x "$HERE/target/release/router-rs" ]]; then
  exec "$HERE/target/release/router-rs" "$@"
fi
if [[ -x "$HERE/target/debug/router-rs" ]]; then
  exec "$HERE/target/debug/router-rs" "$@"
fi
echo "router-rs not found. Build: cargo build --release --manifest-path \"$HERE/Cargo.toml\"" >&2
exit 127
