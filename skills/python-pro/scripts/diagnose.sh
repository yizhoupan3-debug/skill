#!/usr/bin/env bash
# python-pro diagnostic script
# Checks Python project conventions: version, package manager, venv, linting, testing
set -euo pipefail

echo "=== python-pro diagnostics ==="
echo ""

# 1. Python version
if command -v python3 &>/dev/null; then
  PY_VER=$(python3 --version 2>&1)
  echo "[python] $PY_VER"
else
  echo "[python] WARN: python3 not found in PATH"
fi

# 2. Package manager detection
echo ""
echo "--- package manager ---"
if [ -f "uv.lock" ] || [ -f "uv.toml" ]; then
  echo "[pm] uv detected (uv.lock or uv.toml present)"
elif [ -f "poetry.lock" ]; then
  echo "[pm] poetry detected (poetry.lock present)"
elif [ -f "Pipfile.lock" ]; then
  echo "[pm] pipenv detected (Pipfile.lock present)"
elif [ -f "requirements.txt" ]; then
  echo "[pm] pip detected (requirements.txt present)"
else
  echo "[pm] WARN: no lockfile or requirements.txt found"
fi

# 3. pyproject.toml check
echo ""
echo "--- pyproject.toml ---"
if [ -f "pyproject.toml" ]; then
  echo "[config] pyproject.toml found"
  # Check for build system
  if grep -q '\[build-system\]' pyproject.toml 2>/dev/null; then
    echo "[config] build-system configured"
  else
    echo "[config] WARN: no [build-system] section"
  fi
  # Check for ruff config
  if grep -q '\[tool.ruff\]' pyproject.toml 2>/dev/null; then
    echo "[lint] ruff configured in pyproject.toml"
  fi
  # Check for mypy config
  if grep -q '\[tool.mypy\]' pyproject.toml 2>/dev/null; then
    echo "[types] mypy configured in pyproject.toml"
  fi
  # Check for pytest config
  if grep -q '\[tool.pytest\]' pyproject.toml 2>/dev/null; then
    echo "[test] pytest configured in pyproject.toml"
  fi
else
  echo "[config] WARN: no pyproject.toml found"
  if [ -f "setup.py" ]; then
    echo "[config] legacy setup.py found — consider migrating to pyproject.toml"
  fi
fi

# 4. Virtual environment check
echo ""
echo "--- virtual environment ---"
if [ -n "${VIRTUAL_ENV:-}" ]; then
  echo "[venv] active: $VIRTUAL_ENV"
elif [ -d ".venv" ]; then
  echo "[venv] .venv directory exists but not activated"
elif [ -d "venv" ]; then
  echo "[venv] venv directory exists but not activated"
else
  echo "[venv] WARN: no virtual environment detected"
fi

# 5. Linter availability
echo ""
echo "--- linting tools ---"
for tool in ruff black isort flake8 pylint; do
  if command -v "$tool" &>/dev/null; then
    echo "[lint] $tool $(${tool} --version 2>&1 | head -1)"
  fi
done

# 6. Type checker availability
echo ""
echo "--- type checking ---"
for tool in mypy pyright; do
  if command -v "$tool" &>/dev/null; then
    echo "[types] $tool available"
  fi
done

# 7. Testing
echo ""
echo "--- testing ---"
if command -v pytest &>/dev/null; then
  echo "[test] pytest available"
  # Count test files
  TEST_COUNT=$(find . -name "test_*.py" -o -name "*_test.py" 2>/dev/null | wc -l | tr -d ' ')
  echo "[test] $TEST_COUNT test file(s) found"
else
  echo "[test] WARN: pytest not available"
fi

# 8. Python version compatibility
echo ""
echo "--- version compatibility ---"
if [ -f "pyproject.toml" ]; then
  PY_REQ=$(grep -o 'requires-python.*' pyproject.toml 2>/dev/null | head -1 || true)
  if [ -n "$PY_REQ" ]; then
    echo "[compat] $PY_REQ"
  fi
fi

echo ""
echo "=== diagnostics complete ==="
