#!/usr/bin/env bash
# python-env-management health check — uv-only policy
set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

ok=0
warn=0
fail=0

pass() { echo -e "${GREEN}PASS${NC} $*"; ((ok++)) || true; }
info() { echo -e "${YELLOW}INFO${NC} $*"; ((warn++)) || true; }
bad()  { echo -e "${RED}FAIL${NC} $*"; ((fail++)) || true; }

echo "=== python-env-management health-check ==="

if command -v uv >/dev/null 2>&1; then
  pass "uv present: $(command -v uv) ($(uv --version 2>&1 | head -1))"
else
  bad "uv not found in PATH"
fi

uv_expected=""
if command -v uv >/dev/null 2>&1; then
  uv_expected="$( (cd "${HOME:-/}" && uv python find 2>/dev/null) || uv python find 2>/dev/null || true)"
fi

if command -v python3 >/dev/null 2>&1; then
  ver="$(python3 --version 2>&1)"
  exe="$(python3 -c 'import sys, os; print(os.path.realpath(sys.executable))' 2>/dev/null || echo unknown)"
  case "$ver" in
    *"3.12"*) pass "python3 version: $ver" ;;
    *) info "python3 not 3.12 (got $ver) — run: uv python pin --global 3.12" ;;
  esac
  if [[ "$exe" == *".local/share/uv/"* ]]; then
    pass "python3 executable is uv-managed: $exe"
  elif [[ -n "$uv_expected" && "$exe" == "$(cd "${HOME:-/}" && uv python find 2>/dev/null | xargs realpath 2>/dev/null)" ]]; then
    pass "python3 executable matches global uv python find: $exe"
  else
    bad "python3 not uv-managed: $exe (global uv python find: ${uv_expected:-unset})"
  fi
else
  bad "python3 not found"
fi

legacy_path=0
while IFS= read -r d; do
  [[ -z "$d" ]] && continue
  if [[ "$d" == *Python.framework* || "$d" == *Library/Python* ]]; then
    bad "legacy Python on PATH: $d"
    ((legacy_path++)) || true
  fi
done < <(echo "${PATH:-}" | tr ':' '\n')

if (( legacy_path == 0 )); then
  pass "no Python.framework or Library/Python on PATH"
fi

if whence -w pip3 >/dev/null 2>&1; then
  ptype="$(whence -w pip3 2>/dev/null || true)"
  if [[ "$ptype" == pip3:*function* || "$ptype" == *"function"* ]]; then
    pass "pip3 is shell guard (blocked)"
  else
    bad "pip3 is real binary: $ptype — remove from PATH or add guards"
  fi
elif command -v pip3 >/dev/null 2>&1; then
  bad "pip3 is real binary: $(command -v pip3) — remove from PATH or add guards"
else
  pass "pip3 not on PATH"
fi

if command -v pip >/dev/null 2>&1; then
  ptype="$(type pip 2>/dev/null || true)"
  if [[ "$ptype" == *"function pip"* ]]; then
    pass "pip is shell guard (blocked)"
  else
    bad "pip is real binary: $(command -v pip)"
  fi
else
  pass "pip not on PATH"
fi

if command -v uv >/dev/null 2>&1; then
  if [[ -n "$uv_expected" ]]; then
    pass "uv python find: $uv_expected"
  else
    info "uv python find failed — run: uv python install 3.12 && uv python pin --global 3.12"
  fi
fi

if pgrep -lf 'Python.app' >/dev/null 2>&1; then
  info "running Python.app jobs:"
  pgrep -lf 'Python.app' | head -5 || true
else
  pass "no Python.app processes"
fi

echo "=== summary: pass=$ok info=$warn fail=$fail ==="
if (( fail > 0 )); then
  exit 1
fi
exit 0
