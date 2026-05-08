#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


def read_event() -> dict[str, Any]:
    raw = sys.stdin.read()
    if not raw.strip():
        return {}
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError:
        return {}
    return payload if isinstance(payload, dict) else {}


def print_json(payload: dict[str, Any]) -> None:
    sys.stdout.write(json.dumps(payload, ensure_ascii=False) + "\n")


def tool_name_of(event: dict[str, Any]) -> str:
    return str(event.get("tool_name") or event.get("tool") or event.get("name") or "")


def tool_input_of(event: dict[str, Any]) -> dict[str, Any]:
    value = event.get("tool_input") or event.get("input") or event.get("arguments")
    return value if isinstance(value, dict) else {}


def repo_root(event: dict[str, Any]) -> Path:
    cwd = event.get("cwd")
    if isinstance(cwd, str) and cwd.strip():
        path = Path(cwd).resolve()
        for probe in [path, *path.parents]:
            if (probe / ".git").exists():
                return probe
    here = Path(__file__).resolve()
    return here.parents[2]


def supported_source(path: Path) -> bool:
    return path.suffix.lower() in {".rs", ".py", ".js", ".jsx", ".ts", ".tsx"}


def protected_path(path: Path) -> bool:
    normalized = path.as_posix()
    if normalized.endswith("/AGENTS.md") or normalized.endswith(
        "/.codex/host_entrypoints_sync_manifest.json"
    ):
        return True
    if "/artifacts/" in normalized:
        return True
    if "/.cursor/hooks/" in normalized or "/.cursor/hook-tests/" in normalized:
        return True
    return False


def extract_candidate_paths(event: dict[str, Any], root: Path) -> list[Path]:
    tool_input = tool_input_of(event)
    candidates: list[str] = []
    keys = ("path", "file_path", "filePath", "target_file", "targetFile")
    for key in keys:
        value = event.get(key)
        if isinstance(value, str) and value.strip():
            candidates.append(value)
        value = tool_input.get(key)
        if isinstance(value, str) and value.strip():
            candidates.append(value)
    for key in ("paths", "files"):
        value = event.get(key)
        if isinstance(value, list):
            candidates.extend(v for v in value if isinstance(v, str) and v.strip())
        value = tool_input.get(key)
        if isinstance(value, list):
            candidates.extend(v for v in value if isinstance(v, str) and v.strip())
    edits = tool_input.get("edits")
    if isinstance(edits, list):
        for edit in edits:
            if isinstance(edit, dict):
                value = edit.get("path") or edit.get("file_path")
                if isinstance(value, str) and value.strip():
                    candidates.append(value)

    resolved: list[Path] = []
    seen: set[Path] = set()
    for raw in candidates:
        p = Path(raw)
        path = p if p.is_absolute() else (root / p)
        try:
            path = path.resolve()
        except OSError:
            continue
        if path in seen:
            continue
        seen.add(path)
        resolved.append(path)
    return resolved


def apply_rust_rules(text: str) -> tuple[str, int]:
    rules = (
        (r"\.iter\(\)\.cloned\(\)\.collect::<Vec<_>>\(\)", ".to_vec()"),
        (r"String::from\(\"\"\)", "String::new()"),
        (r"Vec::with_capacity\(0\)", "Vec::new()"),
    )
    changed = 0
    out = text
    for pattern, repl in rules:
        out, count = re.subn(pattern, repl, out)
        changed += count
    return out, changed


def apply_python_rules(text: str) -> tuple[str, int]:
    rules = (
        (r"\blist\(\)", "[]"),
        (r"\bdict\(\)", "{}"),
        (r"\bset\(\)", "set()"),  # keep explicit set constructor
    )
    changed = 0
    out = text
    for pattern, repl in rules:
        out, count = re.subn(pattern, repl, out)
        changed += count
    return out, changed


def apply_js_rules(text: str) -> tuple[str, int]:
    rules = (
        (r"\bnew\s+Array\(\)", "[]"),
        (r"\bnew\s+Object\(\)", "{}"),
    )
    changed = 0
    out = text
    for pattern, repl in rules:
        out, count = re.subn(pattern, repl, out)
        changed += count
    return out, changed


def validate_file(path: Path) -> tuple[bool, str]:
    suffix = path.suffix.lower()
    if suffix == ".py":
        command = [sys.executable, "-m", "py_compile", str(path)]
        proc = subprocess.run(command, capture_output=True, text=True, timeout=5)
        return (proc.returncode == 0, proc.stderr.strip() or proc.stdout.strip())
    if suffix == ".rs" and shutil.which("rustfmt"):
        command = ["rustfmt", "--emit", "stdout", str(path)]
        proc = subprocess.run(command, capture_output=True, text=True, timeout=8)
        return (proc.returncode == 0, proc.stderr.strip() or proc.stdout.strip())
    if suffix in {".js", ".jsx", ".mjs", ".cjs"} and shutil.which("node"):
        command = ["node", "--check", str(path)]
        proc = subprocess.run(command, capture_output=True, text=True, timeout=5)
        return (proc.returncode == 0, proc.stderr.strip() or proc.stdout.strip())
    return True, ""


def optimize_file(path: Path) -> dict[str, Any]:
    if not path.exists() or not path.is_file():
        return {"path": str(path), "status": "skipped", "reason": "not-file"}
    if not supported_source(path):
        return {"path": str(path), "status": "skipped", "reason": "unsupported-extension"}
    if protected_path(path):
        return {"path": str(path), "status": "skipped", "reason": "protected-path"}

    original = path.read_text(encoding="utf-8")
    suffix = path.suffix.lower()
    if suffix == ".rs":
        rewritten, change_count = apply_rust_rules(original)
    elif suffix == ".py":
        rewritten, change_count = apply_python_rules(original)
    else:
        rewritten, change_count = apply_js_rules(original)

    if change_count == 0 or rewritten == original:
        return {"path": str(path), "status": "unchanged", "rules_applied": 0}

    backup = original
    path.write_text(rewritten, encoding="utf-8")
    ok, message = validate_file(path)
    if not ok:
        path.write_text(backup, encoding="utf-8")
        return {
            "path": str(path),
            "status": "reverted",
            "rules_applied": change_count,
            "reason": "validation-failed",
            "validation": message[:500],
        }
    return {"path": str(path), "status": "optimized", "rules_applied": change_count}


def write_summary(root: Path, summary: dict[str, Any]) -> None:
    state_dir = root / ".cursor" / "hook-state"
    state_dir.mkdir(parents=True, exist_ok=True)
    target = state_dir / "auto-optimize-last.json"
    payload = json.dumps(summary, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    tmp = state_dir / f".tmp-auto-opt-{os.getpid()}-{int(time.time() * 1e6)}.json"
    tmp.write_text(payload, encoding="utf-8")
    os.replace(tmp, target)


def handle_event(event: dict[str, Any]) -> int:
    root = repo_root(event)
    candidates = extract_candidate_paths(event, root)
    results = [optimize_file(path) for path in candidates]
    summary = {
        "schema_version": "cursor-auto-optimize-v1",
        "authority": "cursor-hook-auto-optimize",
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "event": event.get("hook_event_name") or "",
        "tool_name": tool_name_of(event),
        "candidate_count": len(candidates),
        "optimized_count": sum(1 for item in results if item.get("status") == "optimized"),
        "reverted_count": sum(1 for item in results if item.get("status") == "reverted"),
        "results": results,
    }
    write_summary(root, summary)
    print_json({})
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument("--event", default="")
    parser.add_argument("--help", action="help")
    _args, _unknown = parser.parse_known_args()
    event = read_event()
    try:
        return handle_event(event)
    except Exception:
        print_json({})
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
