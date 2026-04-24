#!/bin/bash
set -euo pipefail

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

exec cargo run --manifest-path "$DIR/rust_tools/anthropic_openai_bridge_rs/Cargo.toml" --release -- \
  --listen "${AOB_LISTEN:-127.0.0.1:8320}" \
  --upstream-base "${AOB_UPSTREAM_BASE:-http://127.0.0.1:8318/v1}" \
  --upstream-key "${AOB_UPSTREAM_KEY:-sk-dummy}" \
  --model "${AOB_MODEL:-gpt-5.5}"
