#!/usr/bin/env python3
"""Adapt shared Claude hook output into the shared framework hook schema."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any

COMMON_ALLOWED_FIELDS = (
    "continue",
    "decision",
    "reason",
    "stopReason",
    "suppressOutput",
    "systemMessage",
)

HOOK_SPECIFIC_FIELDS = {
    "pre-tool-use": (
        "hookEventName",
        "permissionDecision",
        "permissionDecisionReason",
        "additionalContext",
        "updatedInput",
    ),
    "user-prompt-submit": (
        "hookEventName",
        "additionalContext",
    ),
}

SHARED_HOOK_SCRIPTS = {
    "pre-tool-use": ".claude/hooks/pre_tool_use.sh",
    "user-prompt-submit": ".claude/hooks/user_prompt_submit.sh",
}


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", required=True)
    parser.add_argument(
        "--event",
        required=True,
        choices=sorted(SHARED_HOOK_SCRIPTS),
    )
    return parser.parse_args()


def _normalize_decision(value: Any) -> str | None:
    if not isinstance(value, str):
        return None
    if value in {"block", "approve"}:
        return value
    if value == "deny":
        return "block"
    if value == "allow":
        return "approve"
    return None


def _filter_hook_specific_output(event: str, payload: dict[str, Any]) -> dict[str, Any] | None:
    raw = payload.get("hookSpecificOutput")
    if not isinstance(raw, dict):
        return None
    filtered = {
        key: raw[key]
        for key in HOOK_SPECIFIC_FIELDS[event]
        if key in raw and raw[key] is not None
    }
    if not filtered:
        return None
    return filtered


def _bridge_payload(event: str, payload: dict[str, Any]) -> dict[str, Any]:
    bridged: dict[str, Any] = {}
    for key in COMMON_ALLOWED_FIELDS:
        if key not in payload or payload[key] is None:
            continue
        if key == "decision":
            normalized = _normalize_decision(payload[key])
            if normalized is not None:
                bridged[key] = normalized
            continue
        bridged[key] = payload[key]

    hook_specific = _filter_hook_specific_output(event, payload)
    if hook_specific is not None:
        bridged["hookSpecificOutput"] = hook_specific
        if event == "user-prompt-submit":
            additional_context = hook_specific.get("additionalContext")
            if isinstance(additional_context, str) and additional_context.strip():
                existing_system = bridged.get("systemMessage")
                if isinstance(existing_system, str) and existing_system.strip():
                    if additional_context.strip() not in existing_system:
                        bridged["systemMessage"] = (
                            existing_system.rstrip() + "\n\n" + additional_context.strip()
                        )
                else:
                    bridged["systemMessage"] = additional_context.strip()
    return bridged


def _run_shared_hook(repo_root: Path, event: str, stdin_text: str) -> subprocess.CompletedProcess[str]:
    script_path = repo_root / SHARED_HOOK_SCRIPTS[event]
    if not script_path.is_file():
        raise FileNotFoundError(f"missing shared hook script: {script_path}")
    env = os.environ.copy()
    env["CLAUDE_PROJECT_DIR"] = str(repo_root)
    return subprocess.run(
        ["sh", str(script_path)],
        cwd=repo_root,
        env=env,
        input=stdin_text,
        text=True,
        capture_output=True,
    )


def main() -> int:
    args = _parse_args()
    repo_root = Path(args.repo_root).resolve()
    stdin_text = sys.stdin.read()

    try:
        result = _run_shared_hook(repo_root, args.event, stdin_text)
    except FileNotFoundError as exc:
        if args.event == "user-prompt-submit":
            return 0
        print(str(exc), file=sys.stderr)
        return 1

    if result.stderr:
        sys.stderr.write(result.stderr)
    if result.returncode != 0:
        if args.event == "user-prompt-submit":
            return 0
        if result.stdout:
            sys.stdout.write(result.stdout)
        return result.returncode

    stdout = result.stdout.strip()
    if not stdout:
        return 0

    try:
        payload = json.loads(stdout)
    except json.JSONDecodeError as exc:
        print(f"shared hook returned invalid JSON: {exc}", file=sys.stderr)
        return 1

    if not isinstance(payload, dict):
        print("shared hook returned non-object JSON", file=sys.stderr)
        return 1

    bridged = _bridge_payload(args.event, payload)
    if not bridged:
        return 0
    json.dump(bridged, sys.stdout, ensure_ascii=False)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
