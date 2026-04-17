"""Local MCP server exposing framework skills, memory, and runtime artifacts."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any, TextIO

from scripts.hermes_bridge import build_memory_bootstrap, export_skills_for_hermes
from scripts.hermes_default_bootstrap import run_default_bootstrap
from scripts.memory_support import (
    get_repo_root,
    load_runtime_snapshot,
    normalize_evidence_index,
    normalize_next_actions,
    normalize_trace_skills,
    parse_session_summary,
    supervisor_contract,
    workspace_name_from_root,
)

JSONDict = dict[str, Any]
PROTOCOL_VERSION = "2024-11-05"


class FrameworkServerError(Exception):
    """Structured recoverable error for framework MCP calls."""

    def __init__(
        self,
        *,
        code: str,
        message: str,
        suggested_next_actions: list[str] | None = None,
        recoverable: bool = True,
    ) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
        self.suggested_next_actions = suggested_next_actions or []
        self.recoverable = recoverable

    def to_payload(self) -> dict[str, Any]:
        """Return a structured error payload."""

        return {
            "code": self.code,
            "message": self.message,
            "recoverable": self.recoverable,
            "suggested_next_actions": self.suggested_next_actions,
        }


class FrameworkMcpServer:
    """Expose the local framework as an MCP-compatible stdio server."""

    def __init__(
        self,
        *,
        repo_root: Path | None = None,
        output_dir: Path | None = None,
        server_name: str = "skill-framework-mcp",
        server_version: str = "0.1.0",
    ) -> None:
        self._repo_root = (repo_root or get_repo_root()).resolve()
        self._workspace = workspace_name_from_root(self._repo_root)
        self._output_dir = (output_dir or self._repo_root / "artifacts" / "current").resolve()
        self._server_name = server_name
        self._server_version = server_version
        self._tools = self._build_tool_definitions()
        self._resources = self._build_resource_definitions()

    def handle_request(self, request: JSONDict) -> JSONDict | None:
        """Handle one JSON-RPC request."""

        request_id = request.get("id")
        method = request.get("method")
        params = request.get("params", {})
        if method == "notifications/initialized":
            return None
        try:
            if method == "initialize":
                result = self._handle_initialize()
            elif method == "ping":
                result = {}
            elif method == "tools/list":
                result = {"tools": list(self._tools.values())}
            elif method == "tools/call":
                result = self._handle_tools_call(params=params)
            elif method == "resources/list":
                result = {"resources": list(self._resources.values())}
            elif method == "resources/read":
                result = self._handle_resources_read(params=params)
            else:
                raise FrameworkServerError(
                    code="UNSUPPORTED_OPERATION",
                    message=f"Unsupported JSON-RPC method: {method}",
                    suggested_next_actions=["call initialize", "call tools/list", "call resources/list"],
                )
            return self._success_response(request_id=request_id, result=result)
        except FrameworkServerError as error:
            return self._error_response(request_id=request_id, error=error)
        except Exception as error:  # pragma: no cover - defensive guard
            return self._error_response(
                request_id=request_id,
                error=FrameworkServerError(
                    code="INTERNAL_ERROR",
                    message=f"Unhandled server error: {error}",
                    suggested_next_actions=["inspect server logs", "retry with narrower inputs"],
                ),
            )

    def run_stdio_loop(self, stdin: TextIO | None = None, stdout: TextIO | None = None) -> int:
        """Run the line-delimited stdio request loop."""

        input_stream = stdin or sys.stdin
        output_stream = stdout or sys.stdout
        for raw_line in input_stream:
            line = raw_line.strip()
            if not line:
                continue
            try:
                request = json.loads(line)
            except json.JSONDecodeError as exc:
                response = self._error_response(
                    request_id=None,
                    error=FrameworkServerError(
                        code="INVALID_INPUT",
                        message=f"Invalid JSON input: {exc.msg}",
                        suggested_next_actions=["send one JSON-RPC object per line"],
                    ),
                )
            else:
                response = self.handle_request(request)
            if response is not None:
                output_stream.write(json.dumps(response, ensure_ascii=False) + "\n")
                output_stream.flush()
        return 0

    def _handle_initialize(self) -> JSONDict:
        return {
            "protocolVersion": PROTOCOL_VERSION,
            "serverInfo": {"name": self._server_name, "version": self._server_version},
            "capabilities": {
                "tools": {"listChanged": False},
                "resources": {"subscribe": False, "listChanged": False},
            },
        }

    def _handle_tools_call(self, params: JSONDict) -> JSONDict:
        tool_name = self._require_str(arguments=params, key="name")
        arguments = params.get("arguments", {})
        if tool_name not in self._tools:
            raise FrameworkServerError(
                code="INVALID_INPUT",
                message=f"Unknown tool name: {tool_name}",
                suggested_next_actions=["call tools/list to inspect available framework tools"],
            )
        try:
            structured = self._call_tool(tool_name=tool_name, arguments=arguments)
            return {
                "structuredContent": structured,
                "content": [{"type": "text", "text": json.dumps(structured, ensure_ascii=False)}],
                "isError": False,
            }
        except FrameworkServerError as error:
            structured = {"ok": False, "error": error.to_payload()}
            return {
                "structuredContent": structured,
                "content": [{"type": "text", "text": json.dumps(structured, ensure_ascii=False)}],
                "isError": True,
            }

    def _handle_resources_read(self, params: JSONDict) -> JSONDict:
        uri = self._require_str(arguments=params, key="uri")
        if uri not in self._resources:
            raise FrameworkServerError(
                code="INVALID_INPUT",
                message=f"Unknown resource URI: {uri}",
                suggested_next_actions=["call resources/list to inspect available framework resources"],
            )
        resource = self._read_resource(uri=uri)
        return {"contents": [resource]}

    def _call_tool(self, *, tool_name: str, arguments: JSONDict) -> JSONDict:
        if tool_name == "framework_bootstrap_refresh":
            return self._bootstrap_refresh(
                query=self._optional_str(arguments=arguments, key="query", default=""),
                top=self._optional_int(arguments=arguments, key="top", default=8, minimum=1),
            )
        if tool_name == "framework_memory_recall":
            return self._memory_recall(
                query=self._optional_str(arguments=arguments, key="query", default=""),
                top=self._optional_int(arguments=arguments, key="top", default=8, minimum=1),
            )
        if tool_name == "framework_skill_search":
            return self._skill_search(
                query=self._optional_str(arguments=arguments, key="query", default=""),
                limit=self._optional_int(arguments=arguments, key="limit", default=10, minimum=1),
            )
        if tool_name == "framework_runtime_snapshot":
            return self._runtime_snapshot()
        if tool_name == "framework_contract_summary":
            return self._contract_summary()
        raise FrameworkServerError(
            code="UNSUPPORTED_OPERATION",
            message=f"Tool is registered but not implemented: {tool_name}",
            suggested_next_actions=["call tools/list to inspect supported tools"],
        )

    def _bootstrap_refresh(self, *, query: str, top: int) -> JSONDict:
        result = run_default_bootstrap(
            query=query,
            repo_root=self._repo_root,
            output_dir=self._output_dir,
            workspace=self._workspace,
            top=top,
        )
        return {
            "ok": True,
            "workspace": self._workspace,
            "query": query,
            "bootstrap_path": result["bootstrap_path"],
            "paths": result["paths"],
            "memory_items": result["memory_items"],
            "proposal_count": result["proposal_count"],
        }

    def _memory_recall(self, *, query: str, top: int) -> JSONDict:
        payload = build_memory_bootstrap(
            workspace=self._workspace,
            query=query,
            source_root=self._repo_root,
            top=top,
        )
        return {"ok": True, **payload}

    def _skill_search(self, *, query: str, limit: int) -> JSONDict:
        exported = export_skills_for_hermes()
        skills = exported.get("skills", [])
        rows = [item for item in skills if isinstance(item, dict)]
        if query.strip():
            tokens = [token.casefold() for token in query.split() if token.strip()]
            scored: list[tuple[int, dict[str, Any]]] = []
            for row in rows:
                haystack = " ".join(
                    [
                        str(row.get("slug", "")),
                        str(row.get("layer", "")),
                        str(row.get("owner", "")),
                        str(row.get("gate", "")),
                        str(row.get("summary", "")),
                        " ".join(str(item) for item in row.get("triggers", [])),
                    ]
                ).casefold()
                score = sum(token in haystack for token in tokens)
                if score > 0:
                    scored.append((score, row))
            matches = [row for _, row in sorted(scored, key=lambda item: (-item[0], item[1].get("slug", "")))]
        else:
            matches = sorted(rows, key=lambda item: str(item.get("slug", "")))
        return {
            "ok": True,
            "query": query,
            "match_count": len(matches[:limit]),
            "matches": matches[:limit],
            "source": exported.get("source"),
        }

    def _runtime_snapshot(self) -> JSONDict:
        snapshot = load_runtime_snapshot(self._repo_root)
        supervisor = snapshot.supervisor_state
        return {
            "ok": True,
            "workspace": self._workspace,
            "artifact_base": str(snapshot.artifact_base),
            "current_root": str(snapshot.current_root),
            "collected_at": snapshot.collected_at,
            "session_summary_present": bool(snapshot.session_summary_text.strip()),
            "next_action_count": len(snapshot.next_actions.get("next_actions", snapshot.next_actions.get("actions", []))),
            "evidence_count": len(snapshot.evidence_index.get("artifacts", snapshot.evidence_index.get("evidence", []))),
            "trace_skill_count": len(snapshot.trace_metadata.get("skills", snapshot.trace_metadata.get("matched_skills", []))),
            "supervisor_state": {
                "task_id": supervisor.get("task_id"),
                "task_summary": supervisor.get("task_summary"),
                "active_phase": supervisor.get("active_phase"),
                "primary_owner": supervisor.get("primary_owner"),
                "verification_status": (
                    supervisor.get("verification", {}).get("verification_status")
                    if isinstance(supervisor.get("verification"), dict)
                    else None
                ),
            },
            "paths": {
                "session_summary": str(snapshot.current_root / "SESSION_SUMMARY.md"),
                "next_actions": str(snapshot.current_root / "NEXT_ACTIONS.json"),
                "evidence_index": str(snapshot.current_root / "EVIDENCE_INDEX.json"),
                "trace_metadata": str(snapshot.current_root / "TRACE_METADATA.json"),
                "supervisor_state": str(self._repo_root / ".supervisor_state.json"),
            },
        }

    def _contract_summary(self) -> JSONDict:
        snapshot = load_runtime_snapshot(self._repo_root)
        contract = supervisor_contract(snapshot.supervisor_state)
        blockers = snapshot.supervisor_state.get("open_blockers")
        blocker_list = [str(item).strip() for item in blockers if str(item).strip()] if isinstance(blockers, list) else []
        return {
            "ok": True,
            "workspace": self._workspace,
            "goal": contract.get("goal"),
            "scope": contract.get("scope", []),
            "forbidden_scope": contract.get("forbidden_scope", []),
            "acceptance_criteria": contract.get("acceptance_criteria", []),
            "evidence_required": contract.get("evidence_required", []),
            "active_phase": snapshot.supervisor_state.get("active_phase"),
            "primary_owner": snapshot.supervisor_state.get("primary_owner"),
            "next_actions": normalize_next_actions(snapshot.next_actions),
            "open_blockers": blocker_list,
            "trace_skills": normalize_trace_skills(snapshot.trace_metadata),
            "session_summary": parse_session_summary(snapshot.session_summary_text),
            "evidence_count": len(normalize_evidence_index(snapshot.evidence_index)),
            "artifacts_root": str(snapshot.current_root),
        }

    def _build_tool_definitions(self) -> dict[str, dict[str, Any]]:
        return {
            "framework_bootstrap_refresh": {
                "name": "framework_bootstrap_refresh",
                "description": (
                    "Refresh the local Hermes bootstrap bundle that packages skill routing, "
                    "memory recall, and evolution proposals for this workspace."
                ),
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "Optional focus topic for memory recall."},
                        "top": {"type": "integer", "minimum": 1, "description": "Maximum memory items to include."},
                    },
                },
            },
            "framework_memory_recall": {
                "name": "framework_memory_recall",
                "description": "Recall long-term framework memory plus current execution artifacts for this workspace.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "Topic or keyword to retrieve."},
                        "top": {"type": "integer", "minimum": 1, "description": "Maximum retrieved items."},
                    },
                },
            },
            "framework_skill_search": {
                "name": "framework_skill_search",
                "description": "Search the local skill framework by skill name, summary, owner, gate, or trigger phrase.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "Query string matched against local skills."},
                        "limit": {"type": "integer", "minimum": 1, "description": "Maximum returned matches."},
                    },
                    "required": ["query"],
                },
            },
            "framework_runtime_snapshot": {
                "name": "framework_runtime_snapshot",
                "description": "Read the current supervisor and artifact snapshot for this workspace.",
                "inputSchema": {"type": "object", "properties": {}},
            },
            "framework_contract_summary": {
                "name": "framework_contract_summary",
                "description": "Summarize the current execution contract, blockers, evidence, and next actions.",
                "inputSchema": {"type": "object", "properties": {}},
            },
        }

    def _build_resource_definitions(self) -> dict[str, dict[str, Any]]:
        return {
            "framework://memory/project": {
                "uri": "framework://memory/project",
                "name": "Project Memory",
                "description": "Checked-in long-term framework memory for this repository.",
                "mimeType": "text/markdown",
            },
            "framework://routing/runtime": {
                "uri": "framework://routing/runtime",
                "name": "Routing Runtime",
                "description": "Machine-readable skill routing runtime map.",
                "mimeType": "application/json",
            },
            "framework://bootstrap/default": {
                "uri": "framework://bootstrap/default",
                "name": "Default Bootstrap",
                "description": "Current Hermes bootstrap payload for this workspace.",
                "mimeType": "application/json",
            },
            "framework://supervisor/state": {
                "uri": "framework://supervisor/state",
                "name": "Supervisor State",
                "description": "Latest persisted supervisor state for the active workspace.",
                "mimeType": "application/json",
            },
            "framework://artifacts/index": {
                "uri": "framework://artifacts/index",
                "name": "Artifact Index",
                "description": "Compact index of current execution artifacts, evidence, and next actions.",
                "mimeType": "application/json",
            },
        }

    def _read_resource(self, *, uri: str) -> dict[str, Any]:
        if uri == "framework://memory/project":
            path = self._repo_root / "memory" / "MEMORY.md"
            text = self._read_text_file(path=path, missing_message="Project memory file not found.")
            return {"uri": uri, "mimeType": "text/markdown", "text": text}
        if uri == "framework://routing/runtime":
            path = self._repo_root / "skills" / "SKILL_ROUTING_RUNTIME.json"
            text = self._read_text_file(path=path, missing_message="Routing runtime file not found.")
            return {"uri": uri, "mimeType": "application/json", "text": text}
        if uri == "framework://bootstrap/default":
            path = self._output_dir / "hermes_default_bootstrap.json"
            if not path.is_file():
                self._bootstrap_refresh(query="", top=8)
            text = self._read_text_file(path=path, missing_message="Bootstrap payload not found after refresh.")
            return {"uri": uri, "mimeType": "application/json", "text": text}
        if uri == "framework://supervisor/state":
            path = self._repo_root / ".supervisor_state.json"
            text = self._read_text_file(path=path, missing_message="Supervisor state file not found.")
            return {"uri": uri, "mimeType": "application/json", "text": text}
        if uri == "framework://artifacts/index":
            snapshot = load_runtime_snapshot(self._repo_root)
            payload = {
                "workspace": self._workspace,
                "collected_at": snapshot.collected_at,
                "current_root": str(snapshot.current_root),
                "session_summary": parse_session_summary(snapshot.session_summary_text),
                "next_actions": normalize_next_actions(snapshot.next_actions),
                "trace_skills": normalize_trace_skills(snapshot.trace_metadata),
                "evidence": normalize_evidence_index(snapshot.evidence_index),
                "snapshots": [str(path) for path in snapshot.snapshots],
            }
            return {"uri": uri, "mimeType": "application/json", "text": json.dumps(payload, ensure_ascii=False, indent=2)}
        raise FrameworkServerError(
            code="INVALID_INPUT",
            message=f"Unknown resource URI: {uri}",
            suggested_next_actions=["call resources/list to inspect available resources"],
        )

    def _read_text_file(self, *, path: Path, missing_message: str) -> str:
        if not path.is_file():
            raise FrameworkServerError(
                code="MISSING_RESOURCE",
                message=missing_message,
                suggested_next_actions=["refresh the bootstrap bundle", "verify the repository artifacts exist"],
            )
        return path.read_text(encoding="utf-8")

    def _optional_int(self, *, arguments: JSONDict, key: str, default: int, minimum: int = 0) -> int:
        value = arguments.get(key, default)
        if not isinstance(value, int):
            raise FrameworkServerError(
                code="INVALID_INPUT",
                message=f"Expected integer for '{key}', got {type(value).__name__}",
                suggested_next_actions=[f"pass '{key}' as an integer >= {minimum}"],
            )
        if value < minimum:
            raise FrameworkServerError(
                code="INVALID_INPUT",
                message=f"Expected '{key}' >= {minimum}, got {value}",
                suggested_next_actions=[f"pass '{key}' as an integer >= {minimum}"],
            )
        return value

    def _optional_str(self, *, arguments: JSONDict, key: str, default: str) -> str:
        value = arguments.get(key, default)
        if not isinstance(value, str):
            raise FrameworkServerError(
                code="INVALID_INPUT",
                message=f"Expected string for '{key}', got {type(value).__name__}",
                suggested_next_actions=[f"pass '{key}' as a string"],
            )
        return value

    def _require_str(self, *, arguments: JSONDict, key: str) -> str:
        value = arguments.get(key)
        if not isinstance(value, str) or not value.strip():
            raise FrameworkServerError(
                code="INVALID_INPUT",
                message=f"Missing required string field '{key}'",
                suggested_next_actions=[f"provide a non-empty string for '{key}'"],
            )
        return value

    def _success_response(self, *, request_id: Any, result: JSONDict) -> JSONDict:
        return {"jsonrpc": "2.0", "id": request_id, "result": result}

    def _error_response(self, *, request_id: Any, error: FrameworkServerError) -> JSONDict:
        return {
            "jsonrpc": "2.0",
            "id": request_id,
            "error": {"code": -32000, "message": error.message, "data": error.to_payload()},
        }
