#!/usr/bin/env bash
# Workspace bootstrap template must match repo .cursor/hooks.json (7-event subtraction set).
# Event lists are loaded from `router-rs schema-drift contract` (single source with subtraction.rs).
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
hooks="$root/.cursor/hooks.json"
template="$root/configs/framework/cursor-hooks.workspace-template.json"
for f in "$hooks" "$template"; do
  [[ -f "$f" ]] || {
    echo "FAIL: missing $f"
    exit 1
  }
done
uv run python - <<'PY'
import json
import subprocess
import sys
from pathlib import Path

root = Path(".").resolve()
manifest = root / "scripts/router-rs/Cargo.toml"

def load_contract() -> dict:
    out = subprocess.check_output(
        [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(manifest),
            "--",
            "schema-drift",
            "contract",
        ],
        cwd=root,
        text=True,
    )
    return json.loads(out)

contract = load_contract()
REQUIRED = contract["cursor_hooks_required"]
FORBIDDEN = contract["cursor_hooks_forbidden"]

hooks = json.loads((root / ".cursor/hooks.json").read_text())
template = json.loads(
    (root / "configs/framework/cursor-hooks.workspace-template.json").read_text()
)

GATE_TIMEOUT_EVENTS = {
    "beforeSubmitPrompt": 20,
    "stop": 20,
    "postToolUse": 20,
    "subagentStart": 20,
    "subagentStop": 20,
    "sessionStart": 5,
    "sessionEnd": 15,
}


def hook_map(doc: dict) -> dict:
    h = doc.get("hooks") or {}
    out = {}
    for ev, entries in h.items():
        cmds = [e.get("command", "") for e in entries if isinstance(e, dict)]
        timeouts = [e.get("timeout") for e in entries if isinstance(e, dict)]
        out[ev] = {"commands": cmds, "timeouts": timeouts}
    return out


def errors_for(label: str, hm: dict) -> list[str]:
    errs = []
    keys = set(hm)
    for ev in REQUIRED:
        if ev not in keys:
            errs.append(f"{label}: missing required event {ev}")
    for ev in FORBIDDEN:
        if ev in keys:
            errs.append(f"{label}: forbidden removed event {ev} still registered")
    for ev, want in GATE_TIMEOUT_EVENTS.items():
        if ev not in hm:
            continue
        ts = hm[ev]["timeouts"]
        if not ts or ts[0] != want:
            errs.append(
                f"{label}: {ev} timeout must be {want}s (got {ts!r}); "
                "PostToolUse 20s avoids hung review multiset on slow disks"
            )
        cmd = hm[ev]["commands"][0] if hm[ev]["commands"] else ""
        if "cursor-router-rs-hook.sh" not in cmd:
            errs.append(f"{label}: {ev} must invoke cursor-router-rs-hook.sh")
    return errs


h = hook_map(hooks)
t = hook_map(template)
errs = errors_for(".cursor/hooks.json", h) + errors_for("workspace-template", t)
if set(h.keys()) != set(t.keys()):
    errs.append(
        f"event key mismatch: hooks={sorted(h.keys())} template={sorted(t.keys())}"
    )
else:
    for ev in sorted(h.keys()):
        if h[ev]["timeouts"] != t[ev]["timeouts"]:
            errs.append(
                f"timeout mismatch on {ev}: hooks={h[ev]['timeouts']} template={t[ev]['timeouts']}"
            )
        if h[ev]["commands"] != t[ev]["commands"]:
            errs.append(f"command mismatch on {ev}")

if errs:
    print("\n".join(errs), file=sys.stderr)
    sys.exit(1)
print(
    "OK: .cursor/hooks.json matches cursor-hooks.workspace-template.json "
    f"({len(REQUIRED)} events, contract-driven lists)"
)
PY
