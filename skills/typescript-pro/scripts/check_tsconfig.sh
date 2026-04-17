#!/usr/bin/env bash
# typescript-pro tsconfig diagnostic script
# Checks TypeScript project settings: version, strict mode, module resolution, and tooling
set -euo pipefail

echo "=== typescript-pro tsconfig diagnostics ==="
echo ""

# 1. TypeScript version
if command -v npx &>/dev/null && [ -f "node_modules/.package-lock.json" ] || [ -f "node_modules/typescript/package.json" ]; then
  TS_VER=$(npx tsc --version 2>/dev/null || echo "not installed")
  echo "[ts] $TS_VER"
elif command -v tsc &>/dev/null; then
  echo "[ts] $(tsc --version)"
else
  echo "[ts] WARN: TypeScript not found"
fi

# 2. tsconfig.json detection
echo ""
echo "--- tsconfig files ---"
TSCONFIGS=$(find . -maxdepth 3 -name "tsconfig*.json" -not -path "*/node_modules/*" 2>/dev/null)
if [ -n "$TSCONFIGS" ]; then
  echo "$TSCONFIGS" | while read -r f; do
    echo "[config] found: $f"
  done
else
  echo "[config] WARN: no tsconfig*.json found"
  echo "=== diagnostics complete ==="
  exit 0
fi

# 3. Strict mode check (main tsconfig)
MAIN_TSCONFIG="tsconfig.json"
if [ -f "$MAIN_TSCONFIG" ]; then
  echo ""
  echo "--- strict mode ($MAIN_TSCONFIG) ---"

  # Check strict flag
  if grep -q '"strict"' "$MAIN_TSCONFIG" 2>/dev/null; then
    STRICT_VAL=$(grep '"strict"' "$MAIN_TSCONFIG" | grep -o 'true\|false' | head -1)
    if [ "$STRICT_VAL" = "true" ]; then
      echo "[strict] strict: true ✓"
    else
      echo "[strict] WARN: strict is false — consider enabling"
    fi
  else
    echo "[strict] WARN: 'strict' not set — defaults to false"
  fi

  # Check individual strict flags
  for flag in noImplicitAny strictNullChecks strictFunctionTypes noImplicitReturns noUnusedLocals noUnusedParameters; do
    if grep -q "\"$flag\"" "$MAIN_TSCONFIG" 2>/dev/null; then
      VAL=$(grep "\"$flag\"" "$MAIN_TSCONFIG" | grep -o 'true\|false' | head -1)
      echo "[strict] $flag: $VAL"
    fi
  done

  # 4. Module resolution
  echo ""
  echo "--- module resolution ---"
  if grep -q '"moduleResolution"' "$MAIN_TSCONFIG" 2>/dev/null; then
    MOD_RES=$(grep '"moduleResolution"' "$MAIN_TSCONFIG" | grep -oE '"[^"]*"' | tail -1 | tr -d '"')
    echo "[module] moduleResolution: $MOD_RES"
  else
    echo "[module] moduleResolution: not set (defaults to classic or node)"
  fi

  if grep -q '"module"' "$MAIN_TSCONFIG" 2>/dev/null; then
    MOD=$(grep '"module"' "$MAIN_TSCONFIG" | head -1 | grep -oE '"[^"]*"' | tail -1 | tr -d '"')
    echo "[module] module: $MOD"
  fi

  if grep -q '"target"' "$MAIN_TSCONFIG" 2>/dev/null; then
    TARGET=$(grep '"target"' "$MAIN_TSCONFIG" | grep -oE '"[^"]*"' | tail -1 | tr -d '"')
    echo "[module] target: $TARGET"
  fi

  # 5. Important settings
  echo ""
  echo "--- important settings ---"
  for setting in verbatimModuleSyntax isolatedModules esModuleInterop skipLibCheck declaration composite incremental; do
    if grep -q "\"$setting\"" "$MAIN_TSCONFIG" 2>/dev/null; then
      VAL=$(grep "\"$setting\"" "$MAIN_TSCONFIG" | grep -o 'true\|false' | head -1)
      echo "[setting] $setting: $VAL"
    fi
  done

  # 6. Extends check
  if grep -q '"extends"' "$MAIN_TSCONFIG" 2>/dev/null; then
    EXTENDS=$(grep '"extends"' "$MAIN_TSCONFIG" | grep -oE '"[^"]*"' | tail -1 | tr -d '"')
    echo ""
    echo "[extends] extends: $EXTENDS"
  fi
fi

# 7. Type-checking tools
echo ""
echo "--- type-checking tools ---"
if [ -f "package.json" ]; then
  for tool in "@typescript-eslint/parser" "@typescript-eslint/eslint-plugin" "oxlint"; do
    if grep -q "\"$tool\"" package.json 2>/dev/null; then
      echo "[lint] $tool configured"
    fi
  done
fi

# 8. Any count
echo ""
echo "--- any usage ---"
ANY_COUNT=$(grep -r '\bany\b' --include="*.ts" --include="*.tsx" -l . 2>/dev/null | grep -v node_modules | wc -l | tr -d ' ')
echo "[quality] Files containing 'any': $ANY_COUNT"

echo ""
echo "=== diagnostics complete ==="
