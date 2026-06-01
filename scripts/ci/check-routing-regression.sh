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
route_accuracy=$(echo "$report" | python3 -c "
import json, sys
d = json.load(sys.stdin)
print(d.get('route_accuracy', 0))
")
total_cases=$(echo "$report" | python3 -c "
import json, sys
d = json.load(sys.stdin)
print(d.get('total_cases', 0))
")
passed=$(echo "$report" | python3 -c "
import json, sys
d = json.load(sys.stdin)
print(d.get('passed', 0))
")
failed=$(echo "$report" | python3 -c "
import json, sys
d = json.load(sys.stdin)
print(d.get('failed', 0))
")
wrong_owner_rate=$(echo "$report" | python3 -c "
import json, sys
d = json.load(sys.stdin)
print(d.get('wrong_owner_rate', 0))
")

echo "Routing eval: ${passed}/${total_cases} passed, accuracy=${route_accuracy}, threshold=${threshold}"

# Print individual failures for debugging.
if [[ "$failed" -gt 0 ]]; then
  echo "--- failures ---"
  echo "$report" | python3 -c "
import json, sys
d = json.load(sys.stdin)
for f in d.get('failures', []):
    print(f\"  {f['case_id']}: {f['field']} expected={f['expected']} got={f['got']}\")
"
  echo "--- end failures ---"
fi

# Gate: route_accuracy must meet threshold.
python3 -c "
import sys
acc = float('${route_accuracy}')
thr = float('${threshold}')
if acc < thr:
    print(f'FAIL: route_accuracy {acc:.4f} < {thr:.2f} threshold')
    sys.exit(1)
print(f'OK: route_accuracy {acc:.4f} >= {thr:.2f}')
"
