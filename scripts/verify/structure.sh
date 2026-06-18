#!/usr/bin/env bash
# Structure verification — LaTeX compilation, ref/label integrity, equation numbering
# Usage: scripts/verify/structure.sh --texdir <dir> [--main main.tex]
set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
PASS() { echo -e "${GREEN}PASS${NC} $1"; }
FAIL() { echo -e "${RED}FAIL${NC} $1"; failures=$((failures+1)); }
WARN() { echo -e "${YELLOW}WARN${NC} $1"; warnings=$((warnings+1)); }

failures=0; warnings=0

TEXDIR="${TEXDIR:-${1:-.}}"
MAIN="${MAIN:-main.tex}"
cd "$TEXDIR"

if [ ! -f "$MAIN" ]; then
  FAIL "LaTeX main file '$MAIN' not found in $TEXDIR"
  echo "Result: $failures failures, $warnings warnings"
  exit 2
fi

# --- 1. LaTeX compilation check ---
if command -v latexmk &>/dev/null; then
  if latexmk -pdf -interaction=nonstopmode -halt-on-error "$MAIN" &>/dev/null; then
    PASS "LaTeX compilation: exit 0"
  else
    FAIL "LaTeX compilation failed (latexmk exit non-zero)"
  fi
else
  # Fallback: check with pdflatex directly
  if command -v pdflatex &>/dev/null; then
    if pdflatex -interaction=nonstopmode -halt-on-error "$MAIN" &>/dev/null; then
      PASS "LaTeX compilation: pdflatex exit 0"
    else
      FAIL "LaTeX compilation failed (pdflatex exit non-zero)"
    fi
  else
    WARN "LaTeX compilation: neither latexmk nor pdflatex available — skipped"
  fi
fi

# --- 2. Ref/label integrity ---
refs=$(grep -oP '\\\\ref\{[^}]+\}' "$MAIN" 2>/dev/null | sort -u || true)
labels=$(grep -oP '\\\\label\{[^}]+\}' "$MAIN" 2>/dev/null | sort -u || true)
missing=0
while IFS= read -r ref; do
  [ -z "$ref" ] && continue
  label="${ref/\\ref/\\label}"
  if ! echo "$labels" | grep -qF "$label"; then
    FAIL "Dangling reference: $ref has no matching \\label"
    missing=$((missing+1))
  fi
done <<< "$refs"
if [ "$missing" -eq 0 ]; then PASS "Ref/label integrity: all refs have matching labels"
fi

# --- 3. Equation numbering continuity ---
if [ -f "$MAIN" ]; then
  eq_nums=$(grep -oP '\\\\tag\{[^}]+\}' "$MAIN" 2>/dev/null | sed 's/.*{\([0-9.]*\)}/\1/' | sort -n || true)
  expected=1
  for n in $eq_nums; do
    if [ "$n" -ne "$expected" ] 2>/dev/null; then
      FAIL "Equation numbering: expected $expected, got $n"
      break
    fi
    expected=$((expected+1))
  done
  PASS "Equation numbering: continuous (or no \\tag used)"
fi

echo "---"
echo "Result: $failures failures, $warnings warnings"
exit $(( failures > 0 ? 2 : 0 ))
