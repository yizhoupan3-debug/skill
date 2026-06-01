#!/usr/bin/env bash
# Fail if skills/**/*.md teach operator-facing pip (allow normative negatives in python-env-management).
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
# Prefer ripgrep; fall back to grep -r if unavailable.
if command -v rg >/dev/null 2>&1; then
  _search() { rg "$@"; }
else
  _search() {
    # Map rg flags to grep equivalents for CI environments without ripgrep.
    local -a args=()
    local pattern="" targets=()
    local -a exclude_globs=()
    local in_g=0
    for arg in "$@"; do
      if (( in_g )); then
        in_g=0
        case "$arg" in
          !*) exclude_globs+=("${arg#!}") ;;  # negative glob: file-level exclude
          *.md) args+=("--include=$arg") ;;
          *.yml) args+=("--include=$arg") ;;
        esac
        continue
      fi
      case "$arg" in
        -n) args+=("-n") ;;
        -g) in_g=1 ;;
        -v) args+=("-v") ;;
        -) targets+=("-") ;;
        /*) targets+=("$arg") ;;
        *)
          if [[ -z "$pattern" && ${#args[@]} -ge 0 && ! "$arg" =~ ^- ]]; then
            pattern="$arg"
          else
            targets+=("$arg")
          fi
          ;;
      esac
    done
    for excl in "${exclude_globs[@]}"; do
      args+=("--exclude=$excl")
    done
    command grep -rE "${args[@]}" "$pattern" "${targets[@]}" 2>/dev/null || true
  }
fi

# Broad patterns (normative negatives in python-env-management are allowlisted below).
mapfile -t hits < <(
  _search -n \
    'pip install|pip3 install|uv pip install|uv pip sync|uv pip compile|python -m pip|python3 -m pip|\bpip3?\s+install\b' \
    skills/ -g '*.md' 2>/dev/null || true
)

allow_line() {
  local line="$1"
  case "$line" in
    *python-env-management/*) return 0 ;;
    *"Do not add"*) return 0 ;;
    *"Do **not**"*) return 0 ;;
    *"never \`pip"*) return 0 ;;
    *"never pip"*) return 0 ;;
    *"not \`pip"*) return 0 ;;
    *"not pip install"*) return 0 ;;
    *forbidden*) return 0 ;;
    *禁用*) return 0 ;;
    *"Must NOT"*) return 0 ;;
    *"MUST NOT"*) return 0 ;;
    *"Do not use"*) return 0 ;;
    *"not \`setup-python\`"*) return 0 ;;
    *"not \`pip install\`"*) return 0 ;;
    *"Bad"*) return 0 ;; # ci-contract contrast examples
    *"Good"*) return 0 ;;
  esac
  return 1
}

filtered=()
for line in "${hits[@]}"; do
  [[ -z "$line" ]] && continue
  if allow_line "$line"; then
    continue
  fi
  filtered+=("$line")
done

if ((${#filtered[@]} > 0)); then
  printf '%s\n' "${filtered[@]}"
  echo "FAIL: operator-facing pip found in skills/*.md. Use uv add/sync/uvx per python-env-management/SPEC.md."
  exit 1
fi

# Workflows must not reintroduce bare pip for governed Python jobs.
# Exclude the check-skills file itself by name (file-level, not line-level).
_wf_hits=$(
  _search -n 'pip install|setup-python' .github/workflows/ \
    -g '*.yml' -g '!check-skills-no-operator-pip.yml' 2>/dev/null || true
)
if [[ -n "$_wf_hits" ]]; then
  echo "FAIL: .github/workflows contains pip install or setup-python (use setup-uv + uv sync)"
  printf '%s\n' "$_wf_hits"
  exit 1
fi

echo "OK: no operator pip regressions (skills/*.md + workflows)"
