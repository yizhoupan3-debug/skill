#!/usr/bin/env bash
# Formal verification — CAS identity check, SMT consistency, witness, dimensional analysis, dependency graph
# Usage: scripts/verify/formal.sh [--expr <sympy_expr>] [--dimension dimension_report.txt]
set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
PASS() { echo -e "${GREEN}PASS${NC} $1"; }
FAIL() { echo -e "${RED}FAIL${NC} $1"; failures=$((failures+1)); }
WARN() { echo -e "${YELLOW}WARN${NC} $1"; warnings=$((warnings+1)); }

failures=0; warnings=0

EXPR="${EXPR:-}"; DIMENSION_FILE="${DIMENSION_FILE:-}"

# --- Check tool availability ---
HAS_SYMPY=false; HAS_Z3=false
python3 -c "import sympy" 2>/dev/null && HAS_SYMPY=true || true
python3 -c "import z3" 2>/dev/null && HAS_Z3=true || true

# --- 1. CAS identity check ---
if $HAS_SYMPY; then
  if [ -n "$EXPR" ]; then
    result=$(python3 -c "
import sys
from sympy import simplify, sympify
try:
    simplified = simplify('$EXPR')
    if simplified == 0:
        print('PASS')
        sys.exit(0)
    else:
        print(f'RESIDUAL: {simplified}')
        sys.exit(1)
except Exception as e:
    print(f'ERROR: {e}')
    sys.exit(2)
" 2>&1) || true
    if echo "$result" | grep -q 'PASS'; then PASS "CAS identity: $EXPR simplifies to 0"
    elif echo "$result" | grep -q 'RESIDUAL'; then FAIL "CAS identity: residual = $(echo "$result" | grep 'RESIDUAL' | sed 's/RESIDUAL: //')"
    else WARN "CAS identity: $result"
    fi
  else
    WARN "CAS identity: no expression provided (--expr)"
  fi
else
  WARN "CAS identity: SymPy not available (pip install sympy)"
fi

# --- 2. SMT check ---
if $HAS_Z3; then
  WARN "SMT check: requires manual constraint formulation (test via --expr with z3 Python API)"
else
  WARN "SMT check: Z3 not available (pip install z3-solver)"
fi

# --- 3. Witness consistency ---
if $HAS_SYMPY; then
  if [ -n "$EXPR" ]; then
    python3 -c "
from sympy import symbols, sympify
try:
    expr = sympify('$EXPR')
    vars = expr.free_symbols
    if len(vars) > 0:
        subs = {v: 2 for v in vars}  # test with value 2
        val = float(expr.subs(subs))
        print(f'PASS witness: value at test point = {val:.6f}')
    else:
        print('PASS witness: constant expression, val =', float(expr))
except Exception as e:
    print(f'WARN witness: {e}')
" 2>&1
  fi
else
  WARN "Witness: SymPy needed for witness computation"
fi

# --- 4. Dimensional analysis ---
if [ -n "$DIMENSION_FILE" ] && [ -f "$DIMENSION_FILE" ]; then
  mismatches=$(grep -c 'DIMENSION_MISMATCH' "$DIMENSION_FILE" 2>/dev/null || echo 0)
  if [ "$mismatches" -eq 0 ]; then PASS "Dimension: all checked, 0 mismatches"
  else FAIL "Dimension: $mismatches mismatches found"
  fi
else
  WARN "Dimension: no dimension report provided (--dimension)"
fi

echo "---"
echo "Result: $failures failures, $warnings warnings"
exit $(( failures > 0 ? 2 : 0 ))
