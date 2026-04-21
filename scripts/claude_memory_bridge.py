#!/usr/bin/env python3
"""Project-local Claude memory bridge backed by shared runtime artifacts."""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.consolidate_memory import (
    archive_legacy_memory_bundle,
    build_memory_documents,
    persist_memory_bundle,
    write_documents,
    write_memory_state,
)
from scripts.memory_support import (
    classify_runtime_continuity,
    describe_continuity_layout,
    describe_project_local_memory_layout,
    format_repo_relative_path,
    get_repo_root,
    load_runtime_snapshot,
    read_text_if_exists,
    resolve_effective_memory_dir,
    stable_line_items,
    write_text_if_changed,
)

CLAUDE_MEMORY_PATH = Path(".codex") / "memory" / "CLAUDE_MEMORY.md"
DEFAULT_MAX_LINES = 6
ROOT_CONTINUITY_ARTIFACTS = (
    "SESSION_SUMMARY.md",
    "NEXT_ACTIONS.json",
    "EVIDENCE_INDEX.json",
    "TRACE_METADATA.json",
    ".supervisor_state.json",
)
COMMAND_ALIASES = {
    "sync": "refresh-projection",
}
COMMAND_CONTRACTS: dict[str, dict[str, Any]] = {
    "refresh-workflow": {
        "writes": [
            "system clipboard",
        ],
        "forbidden_writes": [
            *ROOT_CONTINUITY_ARTIFACTS,
            str(CLAUDE_MEMORY_PATH),
        ],
        "consolidates_shared_memory": False,
        "summary": "Build the next-turn prompt and copy it to the clipboard without refreshing memory artifacts.",
    },
    "refresh-projection": {
        "writes": ["project-local Claude memory projection"],
        "forbidden_writes": list(ROOT_CONTINUITY_ARTIFACTS),
        "consolidates_shared_memory": False,
        "summary": "Refresh the imported Claude projection without touching shared continuity artifacts.",
    },
    "session-start": {
        "writes": ["project-local Claude memory projection"],
        "forbidden_writes": list(ROOT_CONTINUITY_ARTIFACTS),
        "consolidates_shared_memory": False,
        "summary": "Refresh the imported Claude projection at session start.",
    },
    "session-stop": {
        "writes": ["project-local Claude memory projection"],
        "forbidden_writes": list(ROOT_CONTINUITY_ARTIFACTS),
        "consolidates_shared_memory": False,
        "summary": "Perform a lightweight post-turn projection refresh only.",
    },
    "pre-compact": {
        "writes": ["project-local Claude memory projection"],
        "forbidden_writes": list(ROOT_CONTINUITY_ARTIFACTS),
        "consolidates_shared_memory": False,
        "summary": "Refresh the projection before compaction without running consolidation.",
    },
    "subagent-stop": {
        "writes": ["project-local Claude memory projection"],
        "forbidden_writes": list(ROOT_CONTINUITY_ARTIFACTS),
        "consolidates_shared_memory": False,
        "summary": "Refresh the projection after subagent completion without taking over subagent orchestration.",
    },
    "session-end": {
        "writes": [
            "project-local shared memory bundle",
            "project-local Claude memory projection",
        ],
        "forbidden_writes": list(ROOT_CONTINUITY_ARTIFACTS),
        "consolidates_shared_memory": True,
        "summary": "Consolidate the project-local memory bundle, then refresh the imported Claude projection.",
    },
}


def _extract_section(text: str, headings: tuple[str, ...]) -> str:
    for heading in headings:
        pattern = re.compile(
            rf"^#{{2,6}}\s+{re.escape(heading)}\s*\n(.*?)(?=^#{{2,6}}\s|\Z)",
            re.MULTILINE | re.DOTALL,
        )
        match = pattern.search(text)
        if match:
            return match.group(1).strip()
    return ""



def _extract_bullets(text: str, *, limit: int) -> list[str]:
    items: list[str] = []
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped:
            continue
        value = ""
        if stripped.startswith("- ") or stripped.startswith("* "):
            value = stripped[2:].strip()
        else:
            ordered_match = re.match(r"^\d+[.)]\s+(.*)$", stripped)
            if ordered_match:
                value = ordered_match.group(1).strip()
        if value:
            items.append(value)
    return stable_line_items(items)[:limit]



def _markdown_block(title: str, items: list[str]) -> str:
    if not items:
        items = ["暂无"]
    return "\n".join([f"## {title}", "", *[f"- {item}" for item in items], ""])



def _join_lines(values: list[str]) -> str:
    return " / ".join(str(item) for item in values if str(item).strip())



def _current_state_section(continuity: dict[str, Any]) -> tuple[str, list[str]]:
    if continuity["state"] == "active" and continuity.get("current_execution"):
        current = continuity["current_execution"]
        return (
            "Current Execution State",
            [
                f"task: {current.get('task') or '未记录'}",
                f"phase: {current.get('phase') or '未记录'}",
                f"status: {current.get('status') or 'in_progress'}",
                f"route: {_join_lines(current.get('route', []))}" if current.get("route") else "",
                f"next_actions: {_join_lines(current.get('next_actions', []))}"
                if current.get("next_actions")
                else "",
                f"blockers: {_join_lines(current.get('blockers', []))}" if current.get("blockers") else "",
                f"scope: {_join_lines(current.get('scope', []))}" if current.get("scope") else "",
                f"acceptance: {_join_lines(current.get('acceptance_criteria', []))}"
                if current.get("acceptance_criteria")
                else "",
            ],
        )
    if continuity["state"] == "completed" and continuity.get("recent_completed_execution"):
        completed = continuity["recent_completed_execution"]
        return (
            "Recent Completed Task",
            [
                f"task: {completed.get('task') or '未记录'}",
                f"phase: {completed.get('phase') or '未记录'}",
                f"status: {completed.get('status') or 'completed'}",
                f"route: {_join_lines(completed.get('route', []))}" if completed.get("route") else "",
                (
                    f"terminal_reasons: {_join_lines(completed.get('terminal_reasons', []))}"
                    if completed.get("terminal_reasons")
                    else ""
                ),
                (
                    f"follow_up_notes: {_join_lines(completed.get('follow_up_notes', []))}"
                    if completed.get("follow_up_notes")
                    else ""
                ),
                "current_execution_injection: blocked",
            ],
        )
    warning_title = {
        "stale": "Stale Continuity Warning",
        "inconsistent": "Inconsistent Continuity Warning",
        "missing": "No Active Continuity State",
    }.get(continuity["state"], "Continuity Warning")
    reason_key = {
        "stale": "stale_reasons",
        "inconsistent": "inconsistency_reasons",
        "missing": "recovery_hints",
    }.get(continuity["state"], "recovery_hints")
    reasons = continuity.get(reason_key, []) or continuity.get("recovery_hints", [])
    lines = [
        f"last_known_task: {continuity.get('task') or '未记录'}" if continuity.get("task") else "",
        f"last_known_phase: {continuity.get('phase') or '未记录'}" if continuity.get("phase") else "",
        f"reasons: {_join_lines(reasons)}" if reasons else "",
        f"recovery_hints: {_join_lines(continuity.get('recovery_hints', []))}"
        if continuity.get("recovery_hints")
        else "",
        "current_execution_injection: blocked",
    ]
    return warning_title, [line for line in lines if line]



def _workflow_section(title: str, items: list[str], *, fallback: str = "暂无") -> str:
    lines = stable_line_items(items)
    if not lines:
        lines = [fallback]
    return "\n".join([f"## {title}", "", *[f"- {line}" for line in lines], ""])



def build_refresh_workflow_prompt(
    repo_root: Path,
    *,
    max_lines: int = DEFAULT_MAX_LINES,
    snapshot: Any | None = None,
    continuity: dict[str, Any] | None = None,
) -> str:
    snapshot = snapshot or load_runtime_snapshot(
        repo_root,
        repair=False,
        include_contract_snapshots=False,
    )
    continuity = continuity or classify_runtime_continuity(snapshot)
    current = continuity.get("current_execution") or {}
    completed = continuity.get("recent_completed_execution") or {}
    paths = continuity.get("paths") or {}

    state = continuity.get("state") or "missing"
    task = continuity.get("task") or "未记录"
    phase = continuity.get("phase") or "未记录"
    status = continuity.get("status") or state
    route = continuity.get("route") or []

    if state == "active" and current:
        remaining_tasks = stable_line_items(
            [
                *[str(item).strip() for item in current.get("scope", []) if str(item).strip()],
                *[
                    str(item).strip()
                    for item in current.get("acceptance_criteria", [])
                    if str(item).strip()
                ],
            ]
        )[:max_lines]
        next_steps = stable_line_items(
            ["先核对恢复锚点与当前代码状态", *current.get("next_actions", [])]
        )[:max_lines]
        blockers = stable_line_items(current.get("blockers", []))[:max_lines]
    elif state == "completed" and completed:
        remaining_tasks = stable_line_items(completed.get("follow_up_notes", []))[:max_lines]
        next_steps = stable_line_items(
            [
                "如果要继续相关工作，先新开独立任务，不要直接续接已完成任务",
                *continuity.get("recovery_hints", []),
            ]
        )[:max_lines]
        blockers = []
    elif state == "stale":
        remaining_tasks = stable_line_items(continuity.get("recovery_hints", []))[:max_lines]
        next_steps = stable_line_items(
            [
                "先重读恢复锚点并重建新鲜任务上下文",
                *(continuity.get("next_actions", []) or continuity.get("recovery_hints", [])),
            ]
        )[:max_lines]
        blockers = stable_line_items(continuity.get("blockers", []))[:max_lines]
    elif state == "inconsistent":
        remaining_tasks = stable_line_items(continuity.get("inconsistency_reasons", []))[:max_lines]
        next_steps = stable_line_items(
            [
                "先对齐 SESSION_SUMMARY、TRACE_METADATA 和 SUPERVISOR_STATE",
                *continuity.get("recovery_hints", []),
            ]
        )[:max_lines]
        blockers = stable_line_items(continuity.get("blockers", []))[:max_lines]
    else:
        remaining_tasks = stable_line_items(continuity.get("recovery_hints", []))[:max_lines]
        next_steps = stable_line_items(
            [
                "先补齐缺失锚点并确认任务状态",
                *(continuity.get("next_actions", []) or continuity.get("recovery_hints", [])),
            ]
        )[:max_lines]
        blockers = stable_line_items(continuity.get("blockers", []))[:max_lines]

    execution_instruction = "参考prompt设置的串并行分工，直接开始执行！"

    anchors = stable_line_items(
        [
            f"SESSION_SUMMARY: {paths.get('session_summary')}" if paths.get("session_summary") else "",
            f"NEXT_ACTIONS: {paths.get('next_actions')}" if paths.get("next_actions") else "",
            f"TRACE_METADATA: {paths.get('trace_metadata')}" if paths.get("trace_metadata") else "",
            f"SUPERVISOR_STATE: {paths.get('supervisor_state')}" if paths.get("supervisor_state") else "",
        ]
    )

    lines = [
        "继续当前仓库的工作。先阅读并使用这些恢复锚点：",
        *[f"- {anchor}" for anchor in anchors],
        "",
        "当前上下文：",
        f"- task: {task}",
        f"- phase: {phase}",
        f"- status: {status}",
        f"- continuity_state: {state}",
    ]
    if route:
        lines.append(f"- route: {_join_lines(route)}")
    if remaining_tasks:
        lines.extend(["", "待完成事项：", *[f"- {item}" for item in remaining_tasks]])
    if next_steps:
        lines.extend(["", "必须先做的下一步：", *[f"- {item}" for item in next_steps]])
    if blockers:
        lines.extend(["", "阻塞：", *[f"- {item}" for item in blockers]])
    lines.extend(["", f"执行要求：{execution_instruction}"])
    return "\n".join(lines).strip() + "\n"



def _copy_to_clipboard(prompt: str) -> dict[str, Any]:
    if sys.platform != "darwin":
        return {
            "attempted": False,
            "status": "unsupported",
            "message": "Clipboard copy is only available on macOS via pbcopy.",
        }
    try:
        subprocess.run(
            ["pbcopy"],
            input=prompt,
            text=True,
            capture_output=True,
            check=True,
        )
    except FileNotFoundError:
        return {
            "attempted": True,
            "status": "failed",
            "message": "pbcopy is not available on this machine.",
        }
    except subprocess.CalledProcessError as exc:
        stderr = (exc.stderr or "").strip()
        return {
            "attempted": True,
            "status": "failed",
            "message": stderr or "pbcopy failed.",
        }
    return {
        "attempted": True,
        "status": "copied",
        "message": "Copied refresh prompt to the macOS clipboard.",
    }



def build_claude_memory_projection(
    repo_root: Path,
    *,
    max_lines: int = DEFAULT_MAX_LINES,
    snapshot: Any | None = None,
    continuity: dict[str, Any] | None = None,
) -> str:
    """Render a concise Claude-friendly projection from shared memory and artifacts."""

    snapshot = snapshot or load_runtime_snapshot(repo_root, repair=False)
    continuity = continuity or classify_runtime_continuity(snapshot)
    memory_dir = resolve_effective_memory_dir(repo_root=repo_root)
    memory_md = read_text_if_exists(memory_dir / "MEMORY.md")
    stable_patterns = _extract_bullets(
        _extract_section(memory_md, ("Active Patterns", "项目约定", "稳定事实")),
        limit=max_lines,
    )
    stable_decisions = _extract_bullets(
        _extract_section(memory_md, ("稳定决策", "Decisions")),
        limit=max_lines,
    )
    lessons = _extract_bullets(
        _extract_section(memory_md, ("Lessons", "经验教训")),
        limit=max_lines,
    )
    continuity_layout = describe_continuity_layout(repo_root)
    memory_layout = describe_project_local_memory_layout(repo_root)
    artifact_paths = [
        f"root task mirror: `{continuity_layout['root_task_mirror']['supervisor_state']}`",
        "`SESSION_SUMMARY.md`",
        "`NEXT_ACTIONS.json`",
        "`EVIDENCE_INDEX.json`",
        "`TRACE_METADATA.json`",
        f"active task pointer: `{continuity_layout['task_scoped_current']['active_task_pointer']}`",
        "current session mirror: `artifacts/current/SESSION_SUMMARY.md`",
        "`artifacts/current/SESSION_SUMMARY.md`",
        "`artifacts/current/NEXT_ACTIONS.json`",
        "`artifacts/current/EVIDENCE_INDEX.json`",
        "`artifacts/current/TRACE_METADATA.json`",
        "`artifacts/current/<task_id>/`",
        "`./.codex/memory/`",
        (
            f"logical->physical memory mapping: `./.codex/memory/` -> "
            f"`{format_repo_relative_path(Path(memory_layout['physical_root']), repo_root)}`"
        ),
        f"sync rule: {continuity_layout['sync_responsibility']}",
    ]
    lines = [
        "# Claude Shared Memory Projection",
        "",
        "_Generated from shared runtime artifacts and `./.codex/memory/`. Do not edit manually._",
        "",
        f"- repo_root: `{repo_root}`",
        "",
        _markdown_block(*_current_state_section(continuity)).rstrip(),
        "",
        _markdown_block("Stable Project Patterns", stable_patterns).rstrip(),
        "",
        _markdown_block("Stable Decisions", stable_decisions).rstrip(),
        "",
        _markdown_block("Recent Lessons", lessons).rstrip(),
        "",
        _markdown_block("Artifact Anchors", artifact_paths).rstrip(),
    ]
    return "\n".join(lines).strip() + "\n"



def sync_claude_memory_projection(
    repo_root: Path,
    *,
    max_lines: int = DEFAULT_MAX_LINES,
    snapshot: Any | None = None,
    continuity: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Write the Claude memory projection into the shared project memory directory."""

    snapshot = snapshot or load_runtime_snapshot(repo_root, repair=False)
    continuity = continuity or classify_runtime_continuity(snapshot)
    target = repo_root / CLAUDE_MEMORY_PATH
    content = build_claude_memory_projection(
        repo_root,
        max_lines=max_lines,
        snapshot=snapshot,
        continuity=continuity,
    )
    changed = write_text_if_changed(target, content)
    return {
        "status": "updated" if changed else "unchanged",
        "target_path": str(target),
        "changed": changed,
    }



def consolidate_shared_memory(repo_root: Path) -> dict[str, Any]:
    """Refresh the shared project memory bundle from current runtime artifacts."""

    snapshot = load_runtime_snapshot(repo_root)
    workspace = repo_root.name
    resolved_dir = resolve_effective_memory_dir(workspace=workspace, repo_root=repo_root)
    archive = archive_legacy_memory_bundle(workspace, resolved_dir)
    documents = build_memory_documents(workspace=workspace, snapshot=snapshot)
    changed_files = write_documents(documents, resolved_dir)
    state_path = write_memory_state(snapshot, resolved_dir)
    if state_path:
        changed_files.append(state_path)
    sqlite_result = persist_memory_bundle(workspace, documents, resolved_dir=resolved_dir)
    return {
        "memory_root": str(resolved_dir),
        "changed_files": changed_files,
        "archive": archive,
        "sqlite_result": sqlite_result,
    }



def _resolve_command(command: str) -> tuple[str, dict[str, Any]]:
    canonical_command = COMMAND_ALIASES.get(command, command)
    if canonical_command not in COMMAND_CONTRACTS:
        raise ValueError(f"Unsupported bridge command: {command}")
    return canonical_command, COMMAND_CONTRACTS[canonical_command]



def run_bridge(
    command: str,
    repo_root: Path,
    *,
    max_lines: int = DEFAULT_MAX_LINES,
    auto_clear_ui: bool = False,
) -> dict[str, Any]:
    """Run one lifecycle bridge command."""

    canonical_command, contract = _resolve_command(command)
    result: dict[str, Any] = {
        "command": command,
        "canonical_command": canonical_command,
        "repo_root": str(repo_root),
        "contract": contract,
    }
    if contract["consolidates_shared_memory"]:
        result["consolidation"] = consolidate_shared_memory(repo_root)
    snapshot = load_runtime_snapshot(
        repo_root,
        repair=False,
        include_contract_snapshots=canonical_command != "refresh-workflow",
    )
    continuity = classify_runtime_continuity(snapshot)
    if canonical_command == "refresh-workflow":
        prompt = build_refresh_workflow_prompt(
            repo_root,
            max_lines=max_lines,
            snapshot=snapshot,
            continuity=continuity,
        )
        result["workflow_prompt"] = prompt
        result["clipboard"] = _copy_to_clipboard(prompt)
        result["auto_clear"] = {
            "attempted": False,
            "mode": "off",
            "status": "skipped",
            "message": "Auto-clear is disabled for refresh-workflow.",
        }
        return result
    result["projection"] = sync_claude_memory_projection(
        repo_root,
        max_lines=max_lines,
        snapshot=snapshot,
        continuity=continuity,
    )
    return result



def main() -> int:
    parser = argparse.ArgumentParser(description="Bridge shared project memory into Claude-readable context.")
    parser.add_argument(
        "command",
        choices=tuple((*COMMAND_ALIASES.keys(), *COMMAND_CONTRACTS.keys())),
        help="Lifecycle action to run.",
    )
    parser.add_argument("--repo-root", type=Path, default=None, help="Repository root. Defaults to the detected git root.")
    parser.add_argument("--max-lines", type=int, default=DEFAULT_MAX_LINES, help="Maximum bullets per section.")
    parser.add_argument("--auto-clear-ui", action="store_true", help="Trigger /clear via macOS UI automation after refreshing.")
    parser.add_argument("--json", action="store_true", dest="json_output", help="Emit JSON.")
    args = parser.parse_args()

    repo_root = (args.repo_root or get_repo_root()).resolve()
    result = run_bridge(args.command, repo_root, max_lines=args.max_lines, auto_clear_ui=args.auto_clear_ui)
    if args.json_output:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    elif result.get("workflow_prompt"):
        print(result["workflow_prompt"].rstrip())
    else:
        print(result["projection"]["target_path"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
