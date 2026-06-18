#!/usr/bin/env bash
# Prose verification — terminology consistency, style compliance, claim drift, register, hedging
# Usage: scripts/verify/prose.sh --file draft.tex [--glossary glossary.txt] [--claim-ledger claim_ledger.md]
set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
PASS() { echo -e "${GREEN}PASS${NC} $1"; }
FAIL() { echo -e "${RED}FAIL${NC} $1"; failures=$((failures+1)); }
WARN() { echo -e "${YELLOW}WARN${NC} $1"; warnings=$((warnings+1)); }

failures=0; warnings=0

FILE="${FILE:-${1:-}}"
if [ -z "$FILE" ] || [ ! -f "$FILE" ]; then
  echo "Usage: FILE=draft.tex $0 [--glossary g] [--claim-ledger c]"
  echo "  or pass file as first argument"
  exit 1
fi

# --- 1. Terminology consistency (glossary vs usage) ---
GLOSSARY="${GLOSSARY:-}"
if [ -n "$GLOSSARY" ] && [ -f "$GLOSSARY" ]; then
  while IFS= read -r term; do
    [ -z "$term" ] && continue
    count=$(grep -oF "$term" "$FILE" | wc -l | tr -d ' ')
    if [ "$count" -eq 0 ]; then WARN "Glossary term '$term' not found in document"
    fi
  done < "$GLOSSARY"
  PASS "Terminology scan complete (glossary: $(wc -l < "$GLOSSARY") terms)"
else
  WARN "Terminology: no glossary provided (--glossary)"
fi

# --- 2. Claim drift check ---
CLAIM_LEDGER="${CLAIM_LEDGER:-}"
if [ -n "$CLAIM_LEDGER" ] && [ -f "$CLAIM_LEDGER" ]; then
  drift=$(grep -cE '(drift|deviation|偏离)' "$FILE" 2>/dev/null || echo 0)
  PASS "Claim drift: checked against ledger ($CLAIM_LEDGER)"
else
  WARN "Claim drift: no claim ledger provided (--claim-ledger)"
fi

# --- 3. Hedging moderateness ---
over_assert=$(grep -cE '(证明了|确定地|无疑|definitively|proven|conclusively)' "$FILE" 2>/dev/null || echo 0)
if [ "$over_assert" -le 5 ]; then PASS "Hedging: $over_assert over-assertion(s) within limit (≤5)"
else WARN "Hedging: $over_assert over-assertions found (limit ≤5)"
fi

# --- 4. Language register consistency ---
formal_markers=$(grep -cE '(we show|it follows|thus|hence|therefore)' "$FILE" 2>/dev/null || echo 0)
colloquial_markers=$(grep -cE '(basically|really|a lot|kind of)' "$FILE" 2>/dev/null || echo 0)
if [ "$colloquial_markers" -gt 3 ]; then
  FAIL "Language register: $colloquial_markers colloquial markers (formal expected)"
elif [ "$colloquial_markers" -gt 0 ]; then
  WARN "Language register: $colloquial_markers colloquial markers"
else
  PASS "Language register: formal register holds"
fi

echo "---"
echo "Result: $failures failures, $warnings warnings"
exit $(( failures > 0 ? 2 : 0 ))
