#!/usr/bin/env python3
"""Consolidate short-term execution artifacts into long-term memory files."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.memory_store import EvidenceRecord, MemoryItem, open_workspace_store
from scripts.memory_support import (
    DEFAULT_MEMORY_ROOT,
    current_local_date,
    current_local_timestamp,
    format_bullets,
    latest_session_note_path,
    load_runtime_snapshot,
    markdown_block,
    normalize_evidence_index,
    normalize_next_actions,
    normalize_trace_skills,
    read_text_if_exists,
    resolve_effective_memory_dir,
    safe_slug,
    stable_line_items,
    supervisor_contract,
    workspace_dir,
    write_text_if_changed,
)


def _extract_markdown_goal(text: str) -> str:
    for line in text.splitlines():
        if line.startswith("- task:"):
            return line.split(":", 1)[1].strip()
    return ""


def _extract_markdown_phase(text: str) -> str:
    for line in text.splitlines():
        if line.startswith("- phase:"):
            return line.split(":", 1)[1].strip()
    return ""


def _extract_markdown_route(text: str) -> str:
    return ", ".join(stable_line_items(line[2:].strip() for line in text.splitlines() if line.startswith("- ")))


def _json_as_lines(value: Any) -> list[str]:
    if isinstance(value, list):
        return [str(item).strip() for item in value if str(item).strip()]
    if isinstance(value, dict):
        return [f"{key}: {val}" for key, val in value.items() if str(val).strip()]
    return []


def _extract_bullet_lines(raw: str) -> list[str]:
    """Extract user-meaningful bullet lines from markdown."""

    items: list[str] = []
    for line in raw.splitlines():
        if line.startswith("- "):
            items.append(line[2:].strip())
    return items


def _memory_category_for_file(file_name: str) -> str:
    mappings = {
        "MEMORY.md": "invariant",
        "preferences.md": "preference",
        "decisions.md": "decision",
        "lessons.md": "lesson",
        "runbooks.md": "runbook",
    }
    return mappings.get(file_name, "general")


def _memory_item_id(workspace: str, category: str, index: int, summary: str, fallback: str) -> str:
    summary_slug = safe_slug(summary[:80], fallback=fallback)
    return f"{safe_slug(workspace)}:{category}:{index}:{summary_slug}"


def _join_or_na(values: list[str]) -> str:
    return " / ".join(stable_line_items(values)) if values else "暂无"


def _current_state_lines(snapshot: Any) -> list[str]:
    summary = snapshot.session_summary_text
    task = _extract_markdown_goal(summary) or snapshot.supervisor_state.get("task_summary", "")
    phase = _extract_markdown_phase(summary) or snapshot.supervisor_state.get("active_phase", "")
    status = snapshot.supervisor_state.get("verification", {}).get("verification_status", "") or _extract_markdown_route(summary)
    route = ", ".join(normalize_trace_skills(snapshot.trace_metadata))
    next_actions = normalize_next_actions(snapshot.next_actions)
    open_blockers = snapshot.supervisor_state.get("blockers", {}).get("open_blockers", [])
    contract = supervisor_contract(snapshot.supervisor_state)
    lines = [
        f"task={task}" if task else "",
        f"phase={phase}" if phase else "",
        f"status={status}" if status else "",
        f"route={route}" if route else "",
        f"next_actions={_join_or_na(next_actions)}" if next_actions else "",
        f"blockers={_join_or_na(open_blockers)}" if open_blockers else "",
        f"scope={_join_or_na(contract.get('scope', []))}" if contract.get("scope") else "",
        f"forbidden={_join_or_na(contract.get('forbidden_scope', []))}" if contract.get("forbidden_scope") else "",
        f"acceptance={_join_or_na(contract.get('acceptance_criteria', []))}" if contract.get("acceptance_criteria") else "",
        f"evidence={_join_or_na(contract.get('evidence_required', []))}" if contract.get("evidence_required") else "",
        f"sidecars={_join_or_na(snapshot.supervisor_state.get('delegation', {}).get('delegated_sidecars', []))}"
        if snapshot.supervisor_state.get("delegation", {}).get("delegated_sidecars")
        else "",
        f"trace_skills={route}" if route else "",
    ]
    return [line for line in lines if line]


def _current_state_memory_items(workspace: str, snapshot: Any) -> list[MemoryItem]:
    task = _extract_markdown_goal(snapshot.session_summary_text) or snapshot.supervisor_state.get("task_summary", "current-task")
    route = ", ".join(normalize_trace_skills(snapshot.trace_metadata))
    next_actions = normalize_next_actions(snapshot.next_actions)
    blockers = snapshot.supervisor_state.get("blockers", {}).get("open_blockers", [])
    contract = supervisor_contract(snapshot.supervisor_state)
    items = [
        MemoryItem(
            item_id=f"{safe_slug(workspace)}:task_state:current-task",
            category="task_state",
            source="current_state_extractor",
            summary=task,
            notes=f"phase={snapshot.supervisor_state.get('active_phase', '')}; status={snapshot.supervisor_state.get('verification', {}).get('verification_status', '') or 'in_progress'}; route={route}",
            confidence=1.0,
            metadata={"slot": "current_task"},
            keywords=[task, "current_task"],
        )
    ]
    for idx, action in enumerate(next_actions, start=1):
        items.append(
            MemoryItem(
                item_id=f"{safe_slug(workspace)}:task_state:next-actions:{idx}",
                category="task_state",
                source="current_state_extractor",
                summary=action,
                notes=f"route={route}",
                confidence=0.9,
                metadata={"slot": "next_actions"},
                keywords=[action, "next_actions"],
            )
        )
    for idx, blocker in enumerate(blockers, start=1):
        items.append(
            MemoryItem(
                item_id=f"{safe_slug(workspace)}:blocker:open-blockers:{idx}",
                category="blocker",
                source="current_state_extractor",
                summary=str(blocker),
                notes=f"route={route}",
                confidence=0.95,
                metadata={"slot": "open_blockers"},
                keywords=[str(blocker), "blocker"],
            )
        )
    for idx, constraint in enumerate(contract.get("scope", []), start=1):
        items.append(
            MemoryItem(
                item_id=f"{safe_slug(workspace)}:constraint:execution-constraints:{idx}",
                category="constraint",
                source="current_state_extractor",
                summary=str(constraint),
                notes=f"scope={_join_or_na(contract.get('scope', []))} | forbidden={_join_or_na(contract.get('forbidden_scope', []))} | acceptance={_join_or_na(contract.get('acceptance_criteria', []))} | evidence={_join_or_na(contract.get('evidence_required', []))}",
                confidence=0.85,
                metadata={"slot": "execution_constraints"},
                keywords=[str(constraint), "constraint"],
            )
        )
    history_lines = _current_state_lines(snapshot)
    if history_lines:
        items.append(
            MemoryItem(
                item_id=f"{safe_slug(workspace)}:task_state_history:{current_local_date()}",
                category="general",
                source="current_state_history",
                summary=f"{task}; next_actions={_join_or_na(next_actions)}; blockers={_join_or_na(blockers)}",
                notes="\n".join(history_lines),
                confidence=0.7,
                metadata={"slot": "task_history"},
                keywords=[task, "history"],
            )
        )
    return items


def persist_memory_bundle(
    workspace: str,
    documents: dict[str, str],
    *,
    memory_root: Path | None = None,
    resolved_dir: Path | None = None,
) -> dict[str, Any]:
    """Persist the generated memory bundle into the SQLite store."""

    store = open_workspace_store(workspace, memory_root=memory_root, resolved_dir=resolved_dir)
    persisted_items = 0
    for file_name, text in documents.items():
        if file_name == "MEMORY_AUTO.md":
            continue
        category = _memory_category_for_file(file_name)
        bullets = _extract_bullet_lines(text)
        for index, summary in enumerate(bullets, start=1):
            store.upsert_memory_item(
                MemoryItem(
                    item_id=_memory_item_id(workspace, category, index, summary, file_name),
                    category=category,
                    source=file_name,
                    summary=summary,
                    notes=text[:2000],
                    confidence=0.8,
                    metadata={"document": file_name},
                    keywords=[summary, category, file_name],
                )
            )
            persisted_items += 1
    session_path = latest_session_note_path(workspace)
    session_text = documents.get("SESSION_NOTE", "")
    if session_text:
        store.sync_session_notes(session_path.stem, [line for line in session_text.splitlines() if line.strip()])
    evidence_items = []
    for row in json.loads(documents.get("_EVIDENCE_JSON", "[]")):
        evidence_items.append(
            EvidenceRecord(
                kind=str(row.get("kind", "artifact")),
                path=str(row.get("path", "")),
                content=str(row.get("content", "")),
                artifact_id=str(row.get("artifact_id", row.get("path", ""))),
            )
        )
    if evidence_items:
        store.write_evidence(evidence_items)
    return {
        "db_path": str(store.db_path),
        "memory_items": len(store.list_memory_items(limit=1000, active=False)),
        "session_notes": len(store.list_recent_session_notes(limit=1000)),
        "evidence_records": len(store.list_evidence(limit=1000)),
        "persisted_items": persisted_items,
        "persisted_evidence": len(evidence_items),
    }


def build_memory_documents(
    *,
    workspace: str,
    snapshot: Any,
) -> dict[str, str]:
    """Build memory markdown documents from current runtime artifacts."""

    summary_fields = {
        "task": _extract_markdown_goal(snapshot.session_summary_text) or snapshot.supervisor_state.get("task_summary", ""),
        "phase": _extract_markdown_phase(snapshot.session_summary_text) or snapshot.supervisor_state.get("active_phase", ""),
        "status": snapshot.supervisor_state.get("verification", {}).get("verification_status", "") or "in_progress",
        "route": ", ".join(normalize_trace_skills(snapshot.trace_metadata)) or "未显式定义",
    }
    contract = supervisor_contract(snapshot.supervisor_state)
    next_actions = normalize_next_actions(snapshot.next_actions)
    evidence_index = normalize_evidence_index(snapshot.evidence_index)
    lines = [
        "# MEMORY",
        "",
        f"- workspace: {workspace}",
        f"- generated_at: {current_local_timestamp()}",
        f"- source_root: {snapshot.current_root.parent.parent if snapshot.current_root.parent.parent.exists() else ''}",
        "",
    ]
    facts = [
        f"workspace: {workspace}",
        f"当前主线任务：{summary_fields['task']}" if summary_fields["task"] else "",
        f"当前阶段：{summary_fields['phase']}" if summary_fields["phase"] else "",
        f"当前状态：{summary_fields['status']}" if summary_fields["status"] else "",
        "短期工件 contract 已启用，继续承担工作记忆职责。",
        "长期记忆采用文件型、本地化、低依赖实现，不依赖外部服务。",
        "复杂任务优先把状态外置到 SESSION_SUMMARY / NEXT_ACTIONS / EVIDENCE_INDEX / TRACE_METADATA / .supervisor_state。",
        "高风险动作优先 report-first，禁止直接做破坏性清理。",
        f"已使用的路由/编排技能：{summary_fields['route']}" if summary_fields["route"] else "",
    ]
    lines.append(markdown_block("稳定事实", facts))
    task_state = [
        f"当前主线任务：{summary_fields['task']}" if summary_fields["task"] else "",
        f"当前阶段：{summary_fields['phase']}" if summary_fields["phase"] else "",
        f"当前状态：{summary_fields['status']}" if summary_fields["status"] else "",
        f"当前路由：{summary_fields['route']}" if summary_fields["route"] else "",
        f"下一步动作：{_join_or_na(next_actions)}",
        f"阻塞项：{_join_or_na(snapshot.supervisor_state.get('blockers', {}).get('open_blockers', []))}",
        f"作用域：{_join_or_na(contract.get('scope', []))}" if contract.get("scope") else "",
        f"禁止范围：{_join_or_na(contract.get('forbidden_scope', []))}" if contract.get("forbidden_scope") else "",
        f"验收标准：{_join_or_na(contract.get('acceptance_criteria', []))}" if contract.get("acceptance_criteria") else "",
        f"证据要求：{_join_or_na(contract.get('evidence_required', []))}" if contract.get("evidence_required") else "",
        f"sidecar：{_join_or_na(snapshot.supervisor_state.get('delegation', {}).get('delegated_sidecars', []))}",
        f"当前技能链：{summary_fields['route']}" if summary_fields["route"] else "",
    ]
    lines.append(markdown_block("当前任务态", task_state))
    constraints = [
        f"scope: {_join_or_na(contract.get('scope', []))}" if contract.get("scope") else "",
        f"forbidden_scope: {_join_or_na(contract.get('forbidden_scope', []))}" if contract.get("forbidden_scope") else "",
        f"acceptance_criteria: {_join_or_na(contract.get('acceptance_criteria', []))}" if contract.get("acceptance_criteria") else "",
        f"evidence_required: {_join_or_na(contract.get('evidence_required', []))}" if contract.get("evidence_required") else "",
        f"delegated_sidecars: {_join_or_na(snapshot.supervisor_state.get('delegation', {}).get('delegated_sidecars', []))}",
        f"open_blockers: {_join_or_na(snapshot.supervisor_state.get('blockers', {}).get('open_blockers', []))}",
    ]
    lines.append(markdown_block("当前约束", constraints))
    evidence_lines = [f"{row.get('kind', 'artifact')}: {row.get('path', '')} ({row.get('status', 'unknown')})" for row in evidence_index]
    lines.append(markdown_block("证据索引", evidence_lines))
    memory_auto = "\n".join(lines).strip() + "\n"

    memory_md = read_text_if_exists(resolve_effective_memory_dir(workspace=workspace, repo_root=snapshot.current_root.parent.parent) / "MEMORY.md")
    if not memory_md:
        memory_md = "\n".join(
            [
                "# 项目长期记忆",
                "",
                "_本文件沉淀跨会话稳定的项目事实、决策与约定。当日事项写入 `sessions/`，稳定结论才升级到这里。_",
                "",
                "## 项目身份",
                "",
                f"- **仓库**: `{snapshot.current_root.parent.parent}`",
                "- **核心关注**: Codex 记忆闭环与自动化",
                "- **闭环事实源**: `SESSION_SUMMARY.md` / `NEXT_ACTIONS.json` / `EVIDENCE_INDEX.json` / `TRACE_METADATA.json` / `.supervisor_state.json` / `./.codex/memory/`",
                "",
            ]
        )
    documents = {
        "MEMORY.md": memory_md,
        "MEMORY_AUTO.md": memory_auto,
        "decisions.md": read_text_if_exists(resolve_effective_memory_dir(workspace=workspace, repo_root=snapshot.current_root.parent.parent) / "decisions.md") or "# decisions\n",
        "lessons.md": read_text_if_exists(resolve_effective_memory_dir(workspace=workspace, repo_root=snapshot.current_root.parent.parent) / "lessons.md") or "# lessons\n",
        "preferences.md": read_text_if_exists(resolve_effective_memory_dir(workspace=workspace, repo_root=snapshot.current_root.parent.parent) / "preferences.md") or "# preferences\n",
        "runbooks.md": read_text_if_exists(resolve_effective_memory_dir(workspace=workspace, repo_root=snapshot.current_root.parent.parent) / "runbooks.md") or "# runbooks\n",
        "SESSION_NOTE": "\n".join(_current_state_lines(snapshot)),
        "_EVIDENCE_JSON": json.dumps(evidence_index, ensure_ascii=False),
    }
    return documents


def write_documents(documents: dict[str, str], resolved_dir: Path) -> list[str]:
    """Write markdown documents into the resolved memory directory."""

    changed_files: list[str] = []
    resolved_dir.mkdir(parents=True, exist_ok=True)
    for file_name, text in documents.items():
        if file_name.startswith("_") or file_name == "SESSION_NOTE":
            continue
        if write_text_if_changed(resolved_dir / file_name, text):
            changed_files.append(str((resolved_dir / file_name).resolve()))
    session_path = resolved_dir / "sessions" / f"{current_local_date()}.md"
    if documents.get("SESSION_NOTE") and write_text_if_changed(session_path, documents["SESSION_NOTE"] + "\n"):
        changed_files.append(str(session_path.resolve()))
    return changed_files


def main() -> int:
    """CLI entry point."""

    parser = argparse.ArgumentParser(description="Consolidate short-term artifacts into long-term memory.")
    parser.add_argument("--workspace", required=True)
    parser.add_argument("--source-root", type=Path, default=None)
    parser.add_argument("--memory-root", type=Path, default=None)
    parser.add_argument("--json", action="store_true", dest="json_output")
    args = parser.parse_args()
    source_root = (args.source_root or Path(__file__).resolve().parents[1]).resolve()
    snapshot = load_runtime_snapshot(source_root)
    resolved_dir = resolve_effective_memory_dir(workspace=args.workspace, memory_root=args.memory_root, repo_root=source_root)
    documents = build_memory_documents(workspace=args.workspace, snapshot=snapshot)
    changed_files = write_documents(documents, resolved_dir)
    sqlite_result = persist_memory_bundle(args.workspace, documents, memory_root=args.memory_root, resolved_dir=resolved_dir)
    payload = {
        "workspace": args.workspace,
        "memory_root": str(resolved_dir),
        "changed_files": changed_files,
        "sqlite_result": sqlite_result,
    }
    if args.json_output:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    else:
        print(payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
