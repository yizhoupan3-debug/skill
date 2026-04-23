"""Regression tests for the framework MCP server."""

from __future__ import annotations

import io
import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.framework_mcp import FrameworkMcpServer
from scripts.memory_support import build_memory_state, load_runtime_snapshot


def _write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def _write_json(path: Path, payload: dict[str, object]) -> None:
    _write_text(path, json.dumps(payload, ensure_ascii=False, indent=2) + "\n")


def _seed_runtime_artifacts(repo_root: Path, *, terminal: bool) -> None:
    task_id = "checklist-series-final-closeout-20260418210000" if terminal else "active-bootstrap-repair-20260418210000"
    task_root = repo_root / "artifacts" / "current" / task_id
    if terminal:
        summary_lines = [
            "- task: checklist-series final closeout",
            "- phase: finalized",
            "- status: completed",
        ]
        supervisor_state = {
            "task_id": task_id,
            "task_summary": "checklist-series final closeout",
            "active_phase": "finalized",
            "verification": {"verification_status": "completed"},
            "continuity": {"story_state": "completed", "resume_allowed": False},
            "execution_contract": {
                "goal": "Do not treat closeout as active continuity",
                "scope": ["memory/CLAUDE_MEMORY.md"],
            },
        }
        trace_metadata = {"task": "checklist-series final closeout", "matched_skills": ["checklist-fixer"]}
        next_actions = {
            "next_actions": ["Start a new standalone task before continuing related work"],
        }
        focus_task_id = task_id
        registry_tasks = [
            {
                "task_id": task_id,
                "task": "checklist-series final closeout",
                "phase": "finalized",
                "status": "completed",
                "resume_allowed": False,
            }
        ]
    else:
        summary_lines = [
            "- task: active bootstrap repair",
            "- phase: implementation",
            "- status: in_progress",
        ]
        supervisor_state = {
            "task_id": task_id,
            "task_summary": "active bootstrap repair",
            "active_phase": "implementation",
            "verification": {"verification_status": "in_progress"},
            "continuity": {"story_state": "active", "resume_allowed": True},
            "primary_owner": "skill-framework-developer",
            "execution_contract": {
                "goal": "Repair stale bootstrap injection",
                "scope": ["scripts/memory_support.py"],
                "acceptance_criteria": ["completed tasks never appear as current execution"],
            },
            "blockers": {"open_blockers": ["Need regression coverage"]},
        }
        trace_metadata = {
            "task": "active bootstrap repair",
            "matched_skills": ["execution-controller-coding", "skill-framework-developer"],
        }
        next_actions = {"next_actions": ["Patch classifier", "Run MCP regression tests"]}
        focus_task_id = task_id
        registry_tasks = [
            {
                "task_id": task_id,
                "task": "active bootstrap repair",
                "phase": "implementation",
                "status": "in_progress",
                "resume_allowed": True,
            }
        ]
    _write_text(task_root / "SESSION_SUMMARY.md", "\n".join(summary_lines) + "\n")
    _write_json(task_root / "NEXT_ACTIONS.json", next_actions)
    _write_json(task_root / "EVIDENCE_INDEX.json", {"artifacts": []})
    _write_json(task_root / "TRACE_METADATA.json", trace_metadata)
    _write_text(repo_root / "artifacts" / "current" / "SESSION_SUMMARY.md", "\n".join(summary_lines) + "\n")
    _write_json(repo_root / "artifacts" / "current" / "NEXT_ACTIONS.json", next_actions)
    _write_json(repo_root / "artifacts" / "current" / "EVIDENCE_INDEX.json", {"artifacts": []})
    _write_json(repo_root / "artifacts" / "current" / "TRACE_METADATA.json", trace_metadata)
    _write_json(
        repo_root / "artifacts" / "current" / "active_task.json",
        {"task_id": task_id, "task": supervisor_state["task_summary"]},
    )
    _write_json(
        repo_root / "artifacts" / "current" / "focus_task.json",
        {"task_id": focus_task_id, "task": supervisor_state["task_summary"]},
    )
    _write_json(
        repo_root / "artifacts" / "current" / "task_registry.json",
        {
            "schema_version": "task-registry-v1",
            "focus_task_id": focus_task_id,
            "tasks": registry_tasks,
        },
    )
    _write_json(repo_root / ".supervisor_state.json", supervisor_state)


def _seed_memory_state(repo_root: Path) -> None:
    memory_root = repo_root / ".codex" / "memory"
    memory_root.mkdir(parents=True, exist_ok=True)
    snapshot = load_runtime_snapshot(repo_root)
    _write_json(memory_root / "state.json", build_memory_state(snapshot))


def _call(server: FrameworkMcpServer, request_id: int, method: str, params: dict) -> dict:
    response = server.handle_request(
        {"jsonrpc": "2.0", "id": request_id, "method": method, "params": params}
    )
    assert response is not None
    return response


def _tool_call(server: FrameworkMcpServer, request_id: int, name: str, arguments: dict) -> dict:
    response = _call(
        server=server,
        request_id=request_id,
        method="tools/call",
        params={"name": name, "arguments": arguments},
    )
    return response["result"]["structuredContent"]


def test_tools_and_resources_list_expose_framework_surface(tmp_path: Path) -> None:
    server = FrameworkMcpServer(repo_root=PROJECT_ROOT, output_dir=tmp_path)
    tools = _call(server=server, request_id=1, method="tools/list", params={})
    resources = _call(server=server, request_id=2, method="resources/list", params={})
    tool_names = {tool["name"] for tool in tools["result"]["tools"]}
    resource_uris = {resource["uri"] for resource in resources["result"]["resources"]}
    assert {
        "framework_bootstrap_refresh",
        "framework_memory_recall",
        "framework_skill_search",
        "framework_runtime_snapshot",
        "framework_contract_summary",
    }.issubset(tool_names)
    assert {
        "framework://memory/project",
        "framework://routing/runtime",
        "framework://bootstrap/default",
        "framework://supervisor/state",
        "framework://artifacts/index",
    }.issubset(resource_uris)


def test_bootstrap_refresh_materializes_payload_in_requested_output_dir(tmp_path: Path) -> None:
    server = FrameworkMcpServer(repo_root=PROJECT_ROOT, output_dir=tmp_path)
    payload = _tool_call(
        server=server,
        request_id=3,
        name="framework_bootstrap_refresh",
        arguments={"query": "memory integration", "top": 4},
    )
    bootstrap_path = Path(payload["bootstrap_path"])
    assert payload["ok"] is True
    assert bootstrap_path.is_file()
    assert bootstrap_path.parent.parent == tmp_path
    assert bootstrap_path.name == "framework_default_bootstrap.json"
    assert payload["task_id"]


def test_memory_recall_and_resource_read_return_repo_backed_content(tmp_path: Path) -> None:
    _seed_runtime_artifacts(tmp_path, terminal=False)
    _write_text(tmp_path / ".codex" / "memory" / "MEMORY.md", "# 项目长期记忆\n")
    _write_text(tmp_path / ".codex" / "memory" / "preferences.md", "# preferences\n\n- prefer concise closeouts\n")
    _write_text(tmp_path / "memory" / "MEMORY.md", "# 项目长期记忆\n")
    _seed_memory_state(tmp_path)
    server = FrameworkMcpServer(repo_root=tmp_path, output_dir=tmp_path / "out")
    recall = _tool_call(
        server=server,
        request_id=4,
        name="framework_memory_recall",
        arguments={"query": "active bootstrap repair", "top": 3, "mode": "active"},
    )
    resource = _call(
        server=server,
        request_id=5,
        method="resources/read",
        params={"uri": "framework://memory/project"},
    )
    assert recall["ok"] is True
    assert "memory_root" in recall
    assert recall["continuity"]["state"] == "active"
    assert recall["retrieval"]["active_task_included"] is True
    assert "context" not in recall["retrieval"]
    assert "prompt_payload" not in recall
    assert "source_artifacts" in recall
    assert "项目长期记忆" in resource["result"]["contents"][0]["text"]
    assert "prefer concise closeouts" in resource["result"]["contents"][0]["text"]


def test_memory_recall_without_query_stays_compact(tmp_path: Path) -> None:
    _seed_runtime_artifacts(tmp_path, terminal=False)
    _write_text(
        tmp_path / ".codex" / "memory" / "MEMORY.md",
        "\n".join(
            [
                "# 项目长期记忆",
                "",
                "## Active Patterns",
                "",
                "- AP-1: keep recall compact",
                "- AP-2: use stable summaries",
                "",
                "## 稳定决策",
                "",
                "- SD-1: avoid full document injection",
                "",
            ]
        )
        + "\n",
    )
    _write_text(
        tmp_path / ".codex" / "memory" / "runbooks.md",
        "\n".join(
            [
                "# runbooks",
                "",
                "## 标准操作",
                "",
                "- step 1",
                "- step 2",
                "- step 3",
                "",
            ]
        )
        + "\n",
    )
    _seed_memory_state(tmp_path)
    server = FrameworkMcpServer(repo_root=tmp_path, output_dir=tmp_path / "out")

    recall = _tool_call(
        server=server,
        request_id=54,
        name="framework_memory_recall",
        arguments={"query": "", "top": 8, "mode": "stable"},
    )

    memory_item = next(item for item in recall["retrieval"]["items"] if item["path"] == "MEMORY.md")
    runbook_item = next(item for item in recall["retrieval"]["items"] if item["path"] == "runbooks.md")
    assert "context" not in recall["retrieval"]
    assert "AP-1: keep recall compact" in memory_item["content"]
    assert "avoid full document injection" not in memory_item["content"]
    assert "step 1" in runbook_item["content"]
    assert "step 3" not in runbook_item["content"]


def test_memory_project_resource_reads_logical_codex_memory_root(tmp_path: Path) -> None:
    _write_text(tmp_path / ".codex" / "memory" / "MEMORY.md", "# 逻辑长期记忆\n")
    server = FrameworkMcpServer(repo_root=tmp_path, output_dir=tmp_path / "out")

    resource = _call(
        server=server,
        request_id=51,
        method="resources/read",
        params={"uri": "framework://memory/project"},
    )

    assert "逻辑长期记忆" in resource["result"]["contents"][0]["text"]


def test_memory_project_resource_bundles_stable_memory_documents(tmp_path: Path) -> None:
    _write_text(tmp_path / ".codex" / "memory" / "MEMORY.md", "# 项目长期记忆\n")
    _write_text(tmp_path / ".codex" / "memory" / "preferences.md", "# preferences\n\n- prefer direct answers\n")
    _write_text(tmp_path / ".codex" / "memory" / "decisions.md", "# decisions\n\n- stable-first memory\n")
    server = FrameworkMcpServer(repo_root=tmp_path, output_dir=tmp_path / "out")

    resource = _call(
        server=server,
        request_id=53,
        method="resources/read",
        params={"uri": "framework://memory/project"},
    )

    text = resource["result"]["contents"][0]["text"]
    assert text.startswith("# Project Memory Bundle")
    assert "## preferences.md" in text
    assert "prefer direct answers" in text
    assert "stable-first memory" in text


def test_memory_project_resource_does_not_fallback_to_physical_memory_dir(tmp_path: Path) -> None:
    _write_text(tmp_path / "memory" / "MEMORY.md", "# physical only\n")
    server = FrameworkMcpServer(repo_root=tmp_path, output_dir=tmp_path / "out")

    response = _call(
        server=server,
        request_id=52,
        method="resources/read",
        params={"uri": "framework://memory/project"},
    )

    assert response["error"]["data"]["code"] == "MISSING_RESOURCE"


def test_memory_recall_defaults_to_stable_mode_and_blocks_active_injection(tmp_path: Path) -> None:
    _seed_runtime_artifacts(tmp_path, terminal=False)
    _write_text(tmp_path / ".codex" / "memory" / "MEMORY.md", "# 项目长期记忆\n")
    _seed_memory_state(tmp_path)
    server = FrameworkMcpServer(repo_root=tmp_path, output_dir=tmp_path / "out")

    recall = _tool_call(
        server=server,
        request_id=40,
        name="framework_memory_recall",
        arguments={"query": "active bootstrap repair", "top": 3},
    )

    assert recall["retrieval"]["mode"] == "stable"
    assert recall["retrieval"]["active_task_included"] is False


def test_memory_recall_ignores_unrelated_active_task_continuity(tmp_path: Path) -> None:
    _seed_runtime_artifacts(tmp_path, terminal=False)
    _write_text(tmp_path / ".codex" / "memory" / "MEMORY.md", "# 项目长期记忆\n")
    _write_text(tmp_path / "memory" / "MEMORY.md", "# 项目长期记忆\n")
    server = FrameworkMcpServer(repo_root=tmp_path, output_dir=tmp_path / "out")

    recall = _tool_call(
        server=server,
        request_id=41,
        name="framework_memory_recall",
        arguments={"query": "totally unrelated prompt", "top": 3, "mode": "active"},
    )

    assert recall["continuity"]["state"] == "query-mismatch"
    assert recall["continuity_decision"]["ignored_root_continuity"] is True
    assert recall["continuity"]["current_execution"] is None


def test_memory_recall_uses_rust_adapter(tmp_path: Path) -> None:
    server = FrameworkMcpServer(repo_root=tmp_path, output_dir=tmp_path / "out")
    seen: list[tuple[Path, str, int, str]] = []

    def fake_framework_memory_recall(
        *, repo_root: Path, query: str = "", top: int = 8, mode: str = "stable"
    ) -> dict[str, object]:
        seen.append((repo_root, query, top, mode))
        return {
            "ok": True,
            "workspace": tmp_path.name,
            "memory_root": str(tmp_path / ".codex" / "memory"),
            "retrieval": {
                "mode": mode,
                "active_task_included": False,
                "items": [],
                "context": "",
            },
            "continuity": {"state": "query-mismatch", "current_execution": None},
            "continuity_decision": {"ignored_root_continuity": True},
            "source_artifacts": {},
        }

    server._rust_adapter.framework_memory_recall = fake_framework_memory_recall  # type: ignore[method-assign]

    recall = _tool_call(
        server=server,
        request_id=42,
        name="framework_memory_recall",
        arguments={"query": "unrelated", "top": 5, "mode": "active"},
    )

    assert seen == [(tmp_path, "unrelated", 5, "active")]
    assert recall["ok"] is True
    assert recall["retrieval"]["mode"] == "active"
    assert recall["continuity"]["state"] == "query-mismatch"
    assert "context" not in recall["retrieval"]
    assert "prompt_payload" not in recall


def test_skill_search_and_runtime_snapshot_are_actionable(tmp_path: Path) -> None:
    _seed_runtime_artifacts(tmp_path, terminal=False)
    server = FrameworkMcpServer(repo_root=tmp_path, output_dir=tmp_path / "out")
    search = _tool_call(
        server=server,
        request_id=6,
        name="framework_skill_search",
        arguments={"query": "memory", "limit": 5},
    )
    snapshot = _tool_call(
        server=server,
        request_id=7,
        name="framework_runtime_snapshot",
        arguments={},
    )
    assert search["ok"] is True
    assert any(match["slug"] == "agent-memory" for match in search["matches"])
    assert snapshot["ok"] is True
    assert snapshot["paths"]["supervisor_state"].endswith(".supervisor_state.json")
    assert snapshot["continuity"]["state"] == "active"
    assert snapshot["continuity"]["current_execution"]["task"] == "active bootstrap repair"
    assert snapshot["focus_task_id"] == "active-bootstrap-repair-20260418210000"
    assert snapshot["known_task_ids"] == ["active-bootstrap-repair-20260418210000"]
    assert snapshot["recoverable_task_ids"] == ["active-bootstrap-repair-20260418210000"]
    assert snapshot["parallel_task_count"] == 1
    assert snapshot["registered_tasks"]["task_count"] == 1


def test_contract_summary_and_artifact_index_are_compact_and_actionable(tmp_path: Path) -> None:
    _seed_runtime_artifacts(tmp_path, terminal=True)
    server = FrameworkMcpServer(repo_root=tmp_path, output_dir=tmp_path / "out")
    contract = _tool_call(
        server=server,
        request_id=8,
        name="framework_contract_summary",
        arguments={},
    )
    resource = _call(
        server=server,
        request_id=9,
        method="resources/read",
        params={"uri": "framework://artifacts/index"},
    )
    payload = json.loads(resource["result"]["contents"][0]["text"])
    assert contract["ok"] is True
    assert contract["continuity"]["state"] == "completed"
    assert contract["goal"] is None
    assert contract["next_actions"] == []
    assert contract["recent_completed_execution"]["task"] == "checklist-series final closeout"
    assert payload["workspace"] == tmp_path.name
    assert isinstance(payload["next_actions"], list)


def test_runtime_snapshot_falls_back_to_trace_skill_for_primary_owner(tmp_path: Path) -> None:
    _seed_runtime_artifacts(tmp_path, terminal=False)
    supervisor_state = json.loads((tmp_path / ".supervisor_state.json").read_text(encoding="utf-8"))
    supervisor_state.pop("primary_owner", None)
    _write_json(tmp_path / ".supervisor_state.json", supervisor_state)

    server = FrameworkMcpServer(repo_root=tmp_path, output_dir=tmp_path / "out")
    snapshot = _tool_call(
        server=server,
        request_id=60,
        name="framework_runtime_snapshot",
        arguments={},
    )

    assert snapshot["supervisor_state"]["primary_owner"] == "execution-controller-coding"


def test_runtime_snapshot_surfaces_background_tasks_without_changing_current_execution(tmp_path: Path) -> None:
    _seed_runtime_artifacts(tmp_path, terminal=False)
    registry_path = tmp_path / "artifacts" / "current" / "task_registry.json"
    registry = json.loads(registry_path.read_text(encoding="utf-8"))
    registry["tasks"].append(
        {
            "task_id": "background-lane-cleanup-20260418213000",
            "task": "background lane cleanup",
            "phase": "implementation",
            "status": "in_progress",
            "resume_allowed": True,
        }
    )
    registry_path.write_text(json.dumps(registry, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    server = FrameworkMcpServer(repo_root=tmp_path, output_dir=tmp_path / "out")
    snapshot = _tool_call(
        server=server,
        request_id=61,
        name="framework_runtime_snapshot",
        arguments={},
    )

    assert snapshot["focus_task_id"] == "active-bootstrap-repair-20260418210000"
    assert snapshot["known_task_ids"] == [
        "active-bootstrap-repair-20260418210000",
        "background-lane-cleanup-20260418213000",
    ]
    assert snapshot["recoverable_task_ids"] == [
        "active-bootstrap-repair-20260418210000",
        "background-lane-cleanup-20260418213000",
    ]
    assert snapshot["parallel_task_count"] == 2
    assert snapshot["continuity"]["current_execution"]["task"] == "active bootstrap repair"


def test_runtime_snapshot_uses_rust_adapter(tmp_path: Path) -> None:
    _seed_runtime_artifacts(tmp_path, terminal=False)
    server = FrameworkMcpServer(repo_root=tmp_path, output_dir=tmp_path / "out")
    seen: dict[str, Path] = {}

    def fake_runtime_snapshot(*, repo_root: Path) -> dict[str, object]:
        seen["repo_root"] = repo_root
        return {
            "ok": True,
            "workspace": tmp_path.name,
            "focus_task_id": "focus-task-20260423",
            "known_task_ids": ["focus-task-20260423", "background-task-20260423"],
            "recoverable_task_ids": ["background-task-20260423"],
            "parallel_task_count": 2,
            "registered_tasks": {"task_count": 2, "tasks": []},
            "continuity": {"state": "active"},
            "paths": {"supervisor_state": str(tmp_path / ".supervisor_state.json")},
        }

    server._rust_adapter.framework_runtime_snapshot = fake_runtime_snapshot  # type: ignore[method-assign]

    snapshot = _tool_call(
        server=server,
        request_id=70,
        name="framework_runtime_snapshot",
        arguments={},
    )

    assert seen["repo_root"] == tmp_path
    assert snapshot["continuity"]["state"] == "active"
    assert snapshot["parallel_task_count"] == 2
    assert snapshot["focus_task_id"] == "focus-task-20260423"


def test_contract_summary_surfaces_rust_adapter_failures(tmp_path: Path) -> None:
    _seed_runtime_artifacts(tmp_path, terminal=False)
    server = FrameworkMcpServer(repo_root=tmp_path, output_dir=tmp_path / "out")

    def fail_contract_summary(*, repo_root: Path) -> dict[str, object]:
        raise RuntimeError(f"boom for {repo_root.name}")

    server._rust_adapter.framework_contract_summary = fail_contract_summary  # type: ignore[method-assign]

    response = _call(
        server=server,
        request_id=71,
        method="tools/call",
        params={"name": "framework_contract_summary", "arguments": {}},
    )

    assert response["result"]["isError"] is True
    payload = response["result"]["structuredContent"]
    assert payload["ok"] is False
    assert payload["error"]["code"] == "RUST_CONTRACT_SUMMARY_FAILED"
    assert "boom for" in payload["error"]["message"]


def test_stdio_loop_handles_resource_listing(tmp_path: Path) -> None:
    server = FrameworkMcpServer(repo_root=PROJECT_ROOT, output_dir=tmp_path)
    stdin = io.StringIO(
        "\n".join(
            [
                json.dumps({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
                json.dumps({"jsonrpc": "2.0", "id": 2, "method": "resources/list", "params": {}}),
                "",
            ]
        )
    )
    stdout = io.StringIO()
    exit_code = server.run_stdio_loop(stdin=stdin, stdout=stdout)
    lines = [json.loads(line) for line in stdout.getvalue().strip().splitlines()]
    assert exit_code == 0
    assert lines[0]["result"]["serverInfo"]["name"] == "skill-framework-mcp"
    assert any(resource["uri"] == "framework://memory/project" for resource in lines[1]["result"]["resources"])
