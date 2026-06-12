#!/usr/bin/env bash
# bench-memory.sh — Memory profiling for router-rs using dhat
#
# Usage:
#   bash scripts/bench-memory.sh [binary_name]
#
# Requirements:
#   cargo install dhat  # or: cargo install dhat-rs
#
# Output:
#   dhat output to stderr + dhat-heap.json for DHAT viewer
#   Summary: peak/total allocations, surviving objects

set -euo pipefail

BINARY="${1:-router-rs}"
MANIFEST="core/router-rs/Cargo.toml"

echo "=== Memory Profile: $BINARY ==="
echo "Building with dhat-heap feature..."

# Build with dhat feature
cargo build --manifest-path "$MANIFEST" --features dhat-heap --release 2>&1 | tail -3

echo ""
echo "Running with dhat allocator..."
echo "Output: dhat-heap.json (view with dhat/dh_view.html)"
echo ""

# Run the binary briefly to capture allocation profile
./target-router-rs/release/"$BINARY" framework snapshot 2>&1 | head -20 || true

echo ""
echo "=== Summary ==="
if [ -f dhat-heap.json ]; then
    echo "dhat-heap.json generated successfully"
    echo "View with: open https://nnethercote.github.io/dh_view/dh_view.html"
else
    echo "No dhat-heap.json generated (binary may not have dhat feature)"
fi
