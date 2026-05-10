#!/usr/bin/env bash
set -euo pipefail

BASE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$BASE/deck_output.pptx"

echo "Building deck to $OUT"
# Minimal eval harness artifact writer.
printf "FAKE_PPTX" > "$OUT"
echo "Done."
