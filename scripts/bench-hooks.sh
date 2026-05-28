#!/usr/bin/env bash
# Benchmark Cursor hook subprocess latency (p50/p95).
set -euo pipefail

REPO_ROOT="${REPO_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
if command -v uv >/dev/null 2>&1 && [[ -f "$REPO_ROOT/pyproject.toml" ]]; then
  PYTHON=(uv run --directory "$REPO_ROOT" python)
else
  PYTHON=(python3)
fi
ITERATIONS=20
EVENTS="beforeSubmitPrompt,postToolUse"
REPORT=""
COMPARE=""
FULL_SUITE=0

usage() {
  cat <<'EOF'
Usage: scripts/bench-hooks.sh [options]

  --repo PATH          Repo root (default: parent of scripts/)
  --iterations N       Runs per event (default: 20)
  --events LIST        Comma-separated hook events
  --report PATH        Write JSON report
  --compare PATH       Compare p95 to baseline JSON (needs --report)
  --full-suite         beforeSubmitPrompt,postToolUse,stop,sessionStart,sessionEnd
  -h, --help

Env: ROUTER_RS_BIN, ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE=1 (set by script for stable bench).
  No-op if set during bench: ROUTER_RS_GOAL_CONTINUE_HOOK, ROUTER_RS_RFV_LOOP_HOOK,
  ROUTER_RS_CONTINUITY_STOP_CHECKPOINT, ROUTER_RS_DEPTH_COMPLIANCE_HINT (2026-05 retired hook paths).
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo) REPO_ROOT="$2"; shift 2 ;;
    --iterations) ITERATIONS="$2"; shift 2 ;;
    --events) EVENTS="$2"; shift 2 ;;
    --report) REPORT="$2"; shift 2 ;;
    --compare) COMPARE="$2"; shift 2 ;;
    --full-suite) FULL_SUITE=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown arg: $1" >&2; usage; exit 1 ;;
  esac
done

if [[ "$FULL_SUITE" -eq 1 ]]; then
  EVENTS="beforeSubmitPrompt,postToolUse,stop,sessionStart,sessionEnd"
fi

ROUTER_RS_BIN="${ROUTER_RS_BIN:-}"
TARGET_DIR="${CARGO_TARGET_DIR:-}"
for candidate in \
  ${TARGET_DIR:+"$TARGET_DIR/release/router-rs"} \
  ${TARGET_DIR:+"$TARGET_DIR/debug/router-rs"} \
  "/tmp/skill-cargo-target/release/router-rs" \
  "/tmp/skill-cargo-target/debug/router-rs" \
  "$REPO_ROOT/core/router-rs/target/release/router-rs" \
  "$REPO_ROOT/core/router-rs/target/debug/router-rs"
do
  if [[ -z "$ROUTER_RS_BIN" && -x "$candidate" ]]; then
    ROUTER_RS_BIN="$candidate"
  fi
done
if [[ -z "$ROUTER_RS_BIN" ]]; then
  ROUTER_RS_BIN="$(command -v router-rs 2>/dev/null || true)"
fi
if [[ ! -x "$ROUTER_RS_BIN" ]]; then
  echo "bench-hooks: router-rs binary not found; run: cargo build --manifest-path core/router-rs/Cargo.toml --release" >&2
  exit 1
fi

mkdir -p "$REPO_ROOT/artifacts/current"
export ROUTER_RS_CURSOR_REVIEW_GATE_DISABLE=1
export ROUTER_RS_CURSOR_KILL_STALE_TERMINALS=0
export ROUTER_RS_CURSOR_CARGO_CHECK_SYNC=0

hook_payload() {
  local ev="$1"
  local sid="bench-$$"
  case "$(echo "$ev" | tr '[:upper:]' '[:lower:]')" in
    posttooluse)
      printf '%s' "{\"session_id\":\"$sid\",\"cwd\":\"$REPO_ROOT\",\"tool_name\":\"Read\",\"tool_path\":\"README.md\"}"
      ;;
    stop)
      printf '%s' "{\"session_id\":\"$sid\",\"cwd\":\"$REPO_ROOT\",\"prompt\":\"\",\"agent_response\":\"done\"}"
      ;;
    sessionstart|sessionend)
      printf '%s' "{\"session_id\":\"$sid\",\"cwd\":\"$REPO_ROOT\"}"
      ;;
    *)
      printf '%s' "{\"session_id\":\"$sid\",\"cwd\":\"$REPO_ROOT\",\"prompt\":\"bench ping\"}"
      ;;
  esac
}

percentile() {
  local p="$1"
  shift
  "${PYTHON[@]}" - "$p" "$@" <<'PY'
import sys
p = float(sys.argv[1])
vals = sorted(int(x) for x in sys.argv[2:])
if not vals:
    print(0)
    sys.exit(0)
idx = min(len(vals) - 1, max(0, int(round((p / 100.0) * (len(vals) - 1)))))
print(vals[idx])
PY
}

IFS=',' read -r -a EVENT_ARR <<< "$EVENTS"
TMPDIR_BENCH="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_BENCH"' EXIT

echo "bench-hooks repo=$REPO_ROOT bin=$ROUTER_RS_BIN iterations=$ITERATIONS"
REPORT_LINES=()

for ev in "${EVENT_ARR[@]}"; do
  ev="$(echo "$ev" | xargs)"
  [[ -z "$ev" ]] && continue
  times_file="$TMPDIR_BENCH/${ev}.txt"
  : > "$times_file"
  payload="$(hook_payload "$ev")"
  for ((i = 1; i <= ITERATIONS; i++)); do
    start_ms=$("${PYTHON[@]}" -c 'import time; print(int(time.time()*1000))')
    printf '%s' "$payload" | "$ROUTER_RS_BIN" host cursor hook --event="$ev" --repo-root "$REPO_ROOT" >/dev/null 2>/dev/null || true
    end_ms=$("${PYTHON[@]}" -c 'import time; print(int(time.time()*1000))')
    echo $((end_ms - start_ms)) >> "$times_file"
  done
  mapfile -t samples < "$times_file"
  p50=$(percentile 50 "${samples[@]}")
  p95=$(percentile 95 "${samples[@]}")
  echo "  $ev: p50=${p50}ms p95=${p95}ms (n=${#samples[@]})"
  REPORT_LINES+=("$ev $p50 $p95 ${#samples[@]}")
done

"${PYTHON[@]}" - "${REPORT:-}" "${REPORT_LINES[@]}" <<'PY'
import json, sys
out_path = sys.argv[1]
rows = sys.argv[2:]
report = {}
for row in rows:
    ev, p50, p95, n = row.split()
    report[ev] = {"p50_ms": int(p50), "p95_ms": int(p95), "n": int(n)}
text = json.dumps(report, indent=2)
if out_path:
    import os
    os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)
    open(out_path, "w").write(text + "\n")
    print(f"wrote {out_path}")
else:
    print(text)
PY

if [[ -n "$COMPARE" && -f "$COMPARE" && -n "$REPORT" ]]; then
  "${PYTHON[@]}" - "$COMPARE" "$REPORT" <<'PY'
import json, sys
base = json.load(open(sys.argv[1]))
cur = json.load(open(sys.argv[2]))
for ev, c in cur.items():
    b = base.get(ev, {})
    bp95 = b.get("p95_ms", 0)
    cp95 = c.get("p95_ms", 0)
    if bp95:
        pct = (bp95 - cp95) / bp95 * 100.0
        print(f"  {ev}: p95 {cp95}ms vs baseline {bp95}ms ({pct:+.1f}%)")
PY
fi
