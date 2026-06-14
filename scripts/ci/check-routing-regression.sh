#!/usr/bin/env bash
# Routing regression gate: evaluate all routing_eval_cases.json fixtures and
# assert route_accuracy >= 0.95 (configurable via ACCURACY_THRESHOLD env).
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"

cases="tests/routing_eval_cases.json"
if [[ ! -f "$cases" ]]; then
  echo "FAIL: $cases not found"
  exit 1
fi

threshold="${ACCURACY_THRESHOLD:-0.95}"

report=$(cargo run --quiet --manifest-path core/router-rs/Cargo.toml -- \
  eval route --cases "$cases" 2>&1) || {
  echo "FAIL: cargo eval route command failed"
  echo "$report"
  exit 1
}

# Extract key metrics from the JSON report.
route_accuracy=$(echo "$report" | jq -r '.route_accuracy // 0')
total_cases=$(echo "$report" | jq -r '.total_cases // 0')
passed=$(echo "$report" | jq -r '.passed // 0')
failed=$(echo "$report" | jq -r '.failed // 0')
wrong_owner_rate=$(echo "$report" | jq -r '.wrong_owner_rate // 0')

echo "Routing eval: ${passed}/${total_cases} passed, accuracy=${route_accuracy}, threshold=${threshold}"

# Print individual failures for debugging.
if [[ "$failed" -gt 0 ]]; then
  echo "--- failures ---"
  echo "$report" | jq -r '.failures[]? | "  \(.case_id): \(.field) expected=\(.expected) got=\(.got)"'
  echo "--- end failures ---"
fi

# Gate: route_accuracy must meet threshold.
acc_ok=$(echo "$report" | jq -e ".route_accuracy >= $threshold" >/dev/null 2>&1 && echo 1 || echo 0)
if [[ "$acc_ok" -eq 0 ]]; then
  actual=$(echo "$report" | jq -r '.route_accuracy // 0')
  printf 'FAIL: route_accuracy %.4f < %.2f threshold\n' "$actual" "$threshold"
  exit 1
fi
printf 'OK: route_accuracy %.4f >= %.2f\n' "$route_accuracy" "$threshold"
