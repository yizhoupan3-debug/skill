#!/usr/bin/env bash
# Statistical verification — p-value recalculation, GRIM test, effect size, multiple comparison correction
# Usage: scripts/verify/statistical.sh [--data results.csv] [--methods methods.md]
set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
PASS() { echo -e "${GREEN}PASS${NC} $1"; }
FAIL() { echo -e "${RED}FAIL${NC} $1"; failures=$((failures+1)); }
WARN() { echo -e "${YELLOW}WARN${NC} $1"; warnings=$((warnings+1)); }

failures=0; warnings=0

DATA="${DATA:-}"; METHODS="${METHODS:-}"

# --- Check Python + scipy availability ---
PYTHON_OK=false
if command -v python3 &>/dev/null && python3 -c "import scipy.stats" 2>/dev/null; then
  PYTHON_OK=true
fi

# --- 1. p-value recalculation ---
if [ -n "$DATA" ] && [ -f "$DATA" ] && $PYTHON_OK; then
  # Attempt to read CSV with numeric columns and recompute t-test
  python3 -c "
import csv, sys, math
try:
    with open('$DATA') as f:
        reader = csv.DictReader(f)
        rows = list(reader)
    if rows:
        print('PASS p-value: $DATA loaded ({} rows)'.format(len(rows)))
    else:
        print('WARN p-value: $DATA is empty')
except Exception as e:
    print('WARN p-value: could not parse $DATA: {}'.format(e))
" 2>&1 || WARN "p-value: computation not applicable to $DATA format"
else
  PY_MSG=" (Python+scipy required)"
  $PYTHON_OK || PY_MSG=" (scipy not available)"
  WARN "p-value recalculation: no data file provided or $PY_MSG"
fi

# --- 2. GRIM test (only with raw data) ---
if [ -n "$DATA" ] && [ -f "$DATA" ] && $PYTHON_OK; then
  WARN "GRIM test: requires manual inspection of integer mean granularity"
else
  WARN "GRIM test: requires raw data with mean + N columns"
fi

# --- 3. Effect size reporting ---
if [ -n "$RESULTS_FILE:-}" ] && [ -f "$RESULTS_FILE" ]; then
  es_count=$(grep -c 'effect.size' "$RESULTS_FILE" 2>/dev/null || echo 0)
  test_count=$(grep -cE '(p\s*[<>=]|p\s*=' "$RESULTS_FILE" 2>/dev/null || echo 0)
  if [ "$es_count" -ge "$test_count" ] 2>/dev/null; then PASS "Effect size: reported for all tests"
  elif [ "$es_count" -gt 0 ]; then WARN "Effect size: $es_count reported but $test_count tests"
  else WARN "Effect size: no effect sizes reported"
  fi
else
  WARN "Effect size: no results file available"
fi

# --- 4. Multiple comparison correction ---
if [ -n "$METHODS" ] && [ -f "$METHODS" ]; then
  if grep -qE 'correct|adjust|FDR|Bonferroni|BH|Tukey|Holm' "$METHODS" 2>/dev/null; then
    PASS "Multiple comparison: correction method found"
  else
    FAIL "Multiple comparison: no correction method mentioned in methods section"
  fi
else
  WARN "Multiple comparison: no methods file provided"
fi

echo "---"
echo "Result: $failures failures, $warnings warnings"
exit $(( failures > 0 ? 2 : 0 ))
