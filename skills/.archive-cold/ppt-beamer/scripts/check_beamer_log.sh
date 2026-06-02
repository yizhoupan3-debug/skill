#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: check_beamer_log.sh <log-file>
EOF
}

if [[ $# -ne 1 ]]; then
  usage
  exit 1
fi

log_file="$1"

if [[ ! -f "$log_file" ]]; then
  echo "Log file not found: $log_file" >&2
  exit 1
fi

echo "== High-signal log lines =="
grep -nE 'Overfull|Underfull|LaTeX Warning|Package .* Warning|Undefined control sequence|Missing character|Emergency stop|Fatal error' "$log_file" || true

echo
echo "== Counts =="
overfull_count=$(grep -c 'Overfull' "$log_file" || true)
underfull_count=$(grep -c 'Underfull' "$log_file" || true)
warning_count=$(grep -cE 'LaTeX Warning|Package .* Warning' "$log_file" || true)
fatal_count=$(grep -cE 'Undefined control sequence|Emergency stop|Fatal error' "$log_file" || true)
printf 'Overfull: %s\n' "$overfull_count"
printf 'Underfull: %s\n' "$underfull_count"
printf 'Warnings: %s\n' "$warning_count"
printf 'Fatal-ish: %s\n' "$fatal_count"
