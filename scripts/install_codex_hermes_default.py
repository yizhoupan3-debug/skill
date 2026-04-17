#!/usr/bin/env python3
"""Install a managed Hermes-default block into the Codex model instructions file."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any

START_MARKER = "<!-- HERMES_DEFAULT_RUNTIME_START -->"
END_MARKER = "<!-- HERMES_DEFAULT_RUNTIME_END -->"
DEFAULT_INSTRUCTIONS_PATH = Path(".codex") / "model_instructions.md"
MANAGED_BLOCK = f"""{START_MARKER}
- **Hermes 默认增强层**
  - 使用 `scripts/hermes_default_bootstrap.py`
  - Hermes 相关启动必须在 Codex 会话 / first-turn / conversation start 时先读取默认 bundle
  - `python3 scripts/hermes_default_bootstrap.py --query "<query>" --json`
  - 关注 `bootstrap_path` / `paths` / `memory_items` / `proposal_count`
  - Hermes bundle 只读 task artifacts 与 `.supervisor_state.json`
  - bootstrap 走 artifact-first 策略，不直接替代 Codex runtime 判断
{END_MARKER}
"""


def strip_managed_block(text: str) -> str:
    start = text.find(START_MARKER)
    end = text.find(END_MARKER)
    if start == -1 or end == -1:
        return text
    after = text[end + len(END_MARKER):]
    return (text[:start] + after).strip() + ("\n" if text.strip() else "")


def install_block(path: Path) -> dict[str, Any]:
    """Install the managed block."""

    path.parent.mkdir(parents=True, exist_ok=True)
    original = path.read_text(encoding="utf-8") if path.is_file() else ""
    base = strip_managed_block(original).rstrip()
    updated = (base + "\n\n" + MANAGED_BLOCK.strip() + "\n").lstrip() if base else MANAGED_BLOCK.strip() + "\n"
    changed = updated != original
    if changed:
        path.write_text(updated, encoding="utf-8")
    return {"success": True, "path": str(path), "changed": changed, "status": "installed"}


def remove_block(path: Path) -> dict[str, Any]:
    """Remove the managed block."""

    if not path.exists():
        return {"success": True, "path": str(path), "changed": False, "status": "missing"}
    original = path.read_text(encoding="utf-8")
    updated = strip_managed_block(original)
    changed = updated != original
    if changed:
        path.write_text(updated, encoding="utf-8")
    return {"success": True, "path": str(path), "changed": changed, "status": "removed"}


def main() -> int:
    parser = argparse.ArgumentParser(description="Install/remove the Hermes default runtime block for Codex.")
    sub = parser.add_subparsers(dest="cmd", required=True)
    for name in ("install", "remove"):
        child = sub.add_parser(name)
        child.add_argument("--path", type=Path, default=DEFAULT_INSTRUCTIONS_PATH)
        child.add_argument("--json", action="store_true", dest="json_output")
    args = parser.parse_args()
    payload = install_block(args.path) if args.cmd == "install" else remove_block(args.path)
    if args.json_output:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    else:
        print(payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
