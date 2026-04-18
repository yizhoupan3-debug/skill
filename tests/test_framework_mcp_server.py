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
    assert bootstrap_path.parent == tmp_path


def test_memory_recall_and_resource_read_return_repo_backed_content(tmp_path: Path) -> None:
    server = FrameworkMcpServer(repo_root=PROJECT_ROOT, output_dir=tmp_path)
    recall = _tool_call(
        server=server,
        request_id=4,
        name="framework_memory_recall",
        arguments={"query": "长期记忆", "top": 3},
    )
    resource = _call(
        server=server,
        request_id=5,
        method="resources/read",
        params={"uri": "framework://memory/project"},
    )
    assert recall["ok"] is True
    assert "memory_root" in recall
    assert "source_artifacts" in recall
    assert "项目长期记忆" in resource["result"]["contents"][0]["text"]


def test_memory_project_resource_reads_logical_codex_memory_root(tmp_path: Path) -> None:
    logical_memory_root = tmp_path / ".codex" / "memory"
    logical_memory_root.mkdir(parents=True)
    (logical_memory_root / "MEMORY.md").write_text("# 逻辑长期记忆\n", encoding="utf-8")

    server = FrameworkMcpServer(repo_root=tmp_path, output_dir=tmp_path / "out")
    resource = _call(
        server=server,
        request_id=51,
        method="resources/read",
        params={"uri": "framework://memory/project"},
    )

    assert "逻辑长期记忆" in resource["result"]["contents"][0]["text"]


def test_memory_project_resource_does_not_fallback_to_physical_memory_dir(tmp_path: Path) -> None:
    physical_memory_root = tmp_path / "memory"
    physical_memory_root.mkdir(parents=True)
    (physical_memory_root / "MEMORY.md").write_text("# physical only\n", encoding="utf-8")

    server = FrameworkMcpServer(repo_root=tmp_path, output_dir=tmp_path / "out")
    response = _call(
        server=server,
        request_id=52,
        method="resources/read",
        params={"uri": "framework://memory/project"},
    )

    assert response["error"]["data"]["code"] == "MISSING_RESOURCE"


def test_skill_search_and_runtime_snapshot_are_actionable(tmp_path: Path) -> None:
    server = FrameworkMcpServer(repo_root=PROJECT_ROOT, output_dir=tmp_path)
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


def test_contract_summary_and_artifact_index_are_compact_and_actionable(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    current_root = repo_root / "artifacts" / "current"
    current_root.mkdir(parents=True)
    (repo_root / ".supervisor_state.json").write_text(
        json.dumps(
            {
                "primary_owner": "execution-controller-coding",
                "active_phase": "implementation",
                "open_blockers": ["none"],
                "execution_contract": {
                    "goal": "Validate compact MCP contract summary",
                    "scope": ["artifacts/current"],
                    "acceptance_criteria": ["summary is actionable"],
                    "evidence_required": ["artifact index"],
                },
            },
            ensure_ascii=False,
        ),
        encoding="utf-8",
    )
    (current_root / "NEXT_ACTIONS.json").write_text(
        json.dumps({"next_actions": ["verify contract summary"]}, ensure_ascii=False),
        encoding="utf-8",
    )
    (current_root / "EVIDENCE_INDEX.json").write_text(
        json.dumps({"artifacts": [{"kind": "report", "path": "artifacts/report.md"}]}, ensure_ascii=False),
        encoding="utf-8",
    )
    (current_root / "TRACE_METADATA.json").write_text(
        json.dumps({"skills": ["execution-controller-coding", "git-workflow"]}, ensure_ascii=False),
        encoding="utf-8",
    )
    (current_root / "SESSION_SUMMARY.md").write_text(
        "- task: compact summary fixture\n- phase: implementation\n",
        encoding="utf-8",
    )

    server = FrameworkMcpServer(repo_root=repo_root, output_dir=tmp_path / "out")
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
    assert contract["primary_owner"] == "execution-controller-coding"
    assert isinstance(contract["next_actions"], list)
    assert payload["workspace"] == repo_root.name
    assert isinstance(payload["next_actions"], list)
    assert isinstance(payload["evidence"], list)


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
