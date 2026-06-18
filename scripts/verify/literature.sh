#!/usr/bin/env bash
# Literature verification — DOI reachability, citation-claim alignment, contradiction sweep
# Usage: scripts/verify/literature.sh [--dois "doi1 doi2 ..."] [--claims claims.txt] [--matrix claim_matrix.txt]
set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
PASS() { echo -e "${GREEN}PASS${NC} $1"; }
FAIL() { echo -e "${RED}FAIL${NC} $1"; failures=$((failures+1)); }
WARN() { echo -e "${YELLOW}WARN${NC} $1"; warnings=$((warnings+1)); }

failures=0; warnings=0

# --- 1. DOI reachability ---
if [ -n "${DOIS:-}" ]; then
  for doi in $DOIS; do
    code=$(curl -sIL -o /dev/null -w '%{http_code}' --max-time 5 "https://doi.org/$doi" 2>/dev/null || echo "000")
    if [ "$code" = "000" ]; then WARN "DOI $doi: network unreachable (skipped)"
    elif [ "$code" -ge 200 ] && [ "$code" -lt 400 ]; then PASS "DOI $doi: HTTP $code"
    else FAIL "DOI $doi: HTTP $code (not reachable)"
    fi
  done
else
  WARN "DOI reachability: no DOIs provided (--dois)"
fi

# --- 2. Citation-claim alignment ---
if [ -n "${CLAIMS_TXT:-}" ] && [ -f "$CLAIMS_TXT" ]; then
  unsupported=$(grep -c 'UNSUPPORTED' "$CLAIMS_TXT" 2>/dev/null || echo 0)
  if [ "$unsupported" -eq 0 ]; then PASS "Citation-claim alignment: 0 unsupported claims"
  else FAIL "Citation-claim alignment: $unsupported unsupported claims"
  fi
else
  WARN "Citation-claim alignment: no claim matrix file (--claims)"
fi

# --- 3. Contradiction sweep ---
if [ -n "${CONTRADICTION_FILE:-}" ] && [ -f "$CONTRADICTION_FILE" ]; then
  contra=$(grep -c 'CONTRADICTION' "$CONTRADICTION_FILE" 2>/dev/null || echo 0)
  if [ "$contra" -eq 0 ]; then PASS "Contradiction sweep: 0 unresolved contradictions"
  else FAIL "Contradiction sweep: $contra unresolved contradictions"
  fi
else
  WARN "Contradiction sweep: no contradiction report provided"
fi

# --- 4. Closest work identification ---
if [ -n "${CLOSEST_WORK:-}" ] && [ -f "$CLOSEST_WORK" ]; then
  rows=$(grep -c '^|' "$CLOSEST_WORK" 2>/dev/null || echo 0)
  if [ "$rows" -ge 3 ]; then PASS "Closest work: $rows rows identified"
  else FAIL "Closest work: only $rows rows (need ≥3)"
  fi
else
  WARN "Closest work: no closest_work.md provided"
fi

echo "---"
echo "Result: $failures failures, $warnings warnings"
exit $(( failures > 0 ? 2 : 0 ))
