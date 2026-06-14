#!/usr/bin/env bash
# Benchmark Cursor hook subprocess latency (p50/p95).
# Dependencies: jq, awk (no python).
set -euo pipefail

REPO_ROOT="${REPO_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
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
  "/tmp/skill-${UID:-0}-cargo-target/release/router-rs" \
  "/tmp/skill-${UID:-0}-cargo-target/debug/router-rs" \
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
      jq -n --arg sid "$sid" --arg cwd "$REPO_ROOT" \
        '{session_id:$sid, cwd:$cwd, tool_name:"Read", tool_path:"README.md"}'
      ;;
    stop)
      jq -n --arg sid "$sid" --arg cwd "$REPO_ROOT" \
        '{session_id:$sid, cwd:$cwd, prompt:"", agent_response:"done"}'
      ;;
    sessionstart|sessionend)
      jq -n --arg sid "$sid" --arg cwd "$REPO_ROOT" \
        '{session_id:$sid, cwd:$cwd}'
      ;;
    *)
      jq -n --arg sid "$sid" --arg cwd "$REPO_ROOT" \
        '{session_id:$sid, cwd:$cwd, prompt:"bench ping"}'
      ;;
  esac
}

percentile() {
  local p="$1"
  shift
  printf '%s\n' "$@" | sort -n | awk -v p="$p" '
    { vals[NR] = $1 }
    END {
      if (NR == 0) { print 0; exit }
      idx = int((p / 100.0) * (NR - 1) + 0.5)
      if (idx < 0) idx = 0
      if (idx >= NR) idx = NR - 1
      print vals[idx + 1]
    }
  '
}

now_ms() {
  perl -MTime::HiRes=time -e 'printf "%d\n", time()*1000' 2>/dev/null || echo $(($(date +%s) * 1000))
}

IFS=',' read -r -a EVENT_ARR <<< "$EVENTS"
TMPDIR_BENCH="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_BENCH"' EXIT

echo "bench-hooks repo=$REPO_ROOT bin=$ROUTER_RS_BIN iterations=$ITERATIONS"
REPORT_JSON='{}'

for ev in "${EVENT_ARR[@]}"; do
  ev="$(echo "$ev" | xargs)"
  [[ -z "$ev" ]] && continue
  times_file="$TMPDIR_BENCH/${ev}.txt"
  : > "$times_file"
  payload="$(hook_payload "$ev")"
  for ((i = 1; i <= ITERATIONS; i++)); do
    start_ms=$(now_ms)
    printf '%s' "$payload" | "$ROUTER_RS_BIN" host cursor hook --event="$ev" --repo-root "$REPO_ROOT" >/dev/null 2>/dev/null || true
    end_ms=$(now_ms)
    echo $((end_ms - start_ms)) >> "$times_file"
  done
  mapfile -t samples < "$times_file"
  p50=$(percentile 50 "${samples[@]}")
  p95=$(percentile 95 "${samples[@]}")
  echo "  $ev: p50=${p50}ms p95=${p95}ms (n=${#samples[@]})"
  REPORT_JSON=$(echo "$REPORT_JSON" | jq --arg ev "$ev" --argjson p50 "$p50" --argjson p95 "$p95" --argjson n "${#samples[@]}" \
    '. + {($ev): {"p50_ms": $p50, "p95_ms": $p95, "n": $n}}')
done

if [[ -n "$REPORT" ]]; then
  mkdir -p "$(dirname "$REPORT")"
  echo "$REPORT_JSON" > "$REPORT"
  echo "wrote $REPORT"
else
  echo "$REPORT_JSON" | jq .
fi

if [[ -n "$COMPARE" && -f "$COMPARE" && -n "$REPORT" ]]; then
  jq -r --slurpfile base "$COMPARE" '
    to_entries[] |
    .key as $ev |
    .value.p95_ms as $cp95 |
    ($base[0][$ev].p95_ms // 0) as $bp95 |
    (if $bp95 > 0 then (($bp95 - $cp95) / $bp95 * 100) else 0 end) as $pct |
    (if $pct > 0 then "+" else "" end) as $sign |
    "  \($ev): p95 \($cp95)ms vs baseline \($bp95)ms (\($sign)\($pct | . * 10 | round / 10 | tostring)%)"
  ' "$REPORT"
fi
