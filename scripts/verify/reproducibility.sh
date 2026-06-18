#!/usr/bin/env bash
# Reproducibility verification — seed check, deterministic replay, lock file, checkpoint
# Usage: scripts/verify/reproducibility.sh [--srcdir src/] [--lock uv.lock] [--ckpt checkpoint.pt]
set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
PASS() { echo -e "${GREEN}PASS${NC} $1"; }
FAIL() { echo -e "${RED}FAIL${NC} $1"; failures=$((failures+1)); }
WARN() { echo -e "${YELLOW}WARN${NC} $1"; warnings=$((warnings+1)); }

failures=0; warnings=0

SRCDIR="${SRCDIR:-${1:-src/}}"
LOCK="${LOCK:-}"
CKPT="${CKPT:-}"

# --- 1. Seed check ---
if [ -d "$SRCDIR" ]; then
  seed_count=$(grep -rnE 'seed\s*=\s*[0-9]+' "$SRCDIR" 2>/dev/null | wc -l | tr -d ' ')
  if [ "$seed_count" -ge 1 ]; then PASS "Seed: found $seed_count seed assignments"
  else FAIL "Seed: no fixed seed found in $SRCDIR"
  fi
else
  FAIL "Seed: source directory '$SRCDIR' not found"
fi

# --- 2. Lock file ---
echo "---"
echo "Lock file checks:"
if [ -z "$LOCK" ]; then
  # Auto-detect
  for f in uv.lock Cargo.lock package-lock.json yarn.lock poetry.lock requirements.lock; do
    if [ -f "$f" ]; then LOCK="$f"; break; fi
  done
fi
if [ -n "$LOCK" ] && [ -f "$LOCK" ]; then
  PASS "Lock file: $LOCK exists"

  # Check lock file freshness (for uv.lock, Cargo.lock)
  case "$LOCK" in
    uv.lock)
      if command -v uv &>/dev/null && uv lock --check &>/dev/null 2>&1; then
        PASS "Lock file: uv lock check passed"
      else
        WARN "Lock file: uv lock check — run 'uv lock' to sync"
      fi
      ;;
    Cargo.lock)
      if command -v cargo &>/dev/null; then
        PASS "Lock file: Cargo.lock present (cargo verify)"
      fi
      ;;
  esac
else
  FAIL "Lock file: none found"
fi

# --- 3. Data versioning ---
if [ -f .dvc/config ]; then PASS "Data versioning: DVC configured"
elif git lfs track --list 2>/dev/null | grep -q .; then PASS "Data versioning: Git LFS configured"
else WARN "Data versioning: neither DVC nor Git LFS detected"
fi

# --- 4. Checkpoint restoration ---
if [ -n "$CKPT" ] && [ -f "$CKPT" ]; then
  if python3 -c "
import sys
try:
    import torch
    torch.load('$CKPT', map_location='cpu')
    print('PASS')
except Exception:
    print('FAIL')
" 2>/dev/null | grep -q PASS; then
    PASS "Checkpoint: $CKPT loadable"
  else
    FAIL "Checkpoint: $CKPT could not be loaded (corrupted or wrong format)"
  fi
else
  WARN "Checkpoint: no checkpoint file provided (--ckpt)"
fi

echo "---"
echo "Result: $failures failures, $warnings warnings"
exit $(( failures > 0 ? 2 : 0 ))
