"""Local MCP server exposing framework skills, memory, and runtime artifacts."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any, TextIO

from framework_runtime.rust_router import RustRouteAdapter

from scripts.default_bootstrap import resolve_bootstrap_path, run_default_bootstrap
from scripts.framework_bridge import export_framework_skills

JSONDict = dict[str, Any]
PROTOCOL_VERSION = "2024-11-05"
STABLE_MEMORY_FILENAMES = (
    "MEMORY.md",
    "preferences.md",
    "decisions.md",
    "lessons.md",
    "runbooks.md",
)


def _framework_root() -> Path:
    return Path(__file__).resolve().parents[2]


def _workspace_name_from_root(repo_root: Path) -> str:
    return repo_root.name


def _bootstrap_artifact_root(repo_root: Path) -> Path:
    return repo_root / "artifacts" / "bootstrap"


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
        self._repo_root = (repo_root or _framework_root()).resolve()
        self._framework_root = _framework_root()
        self._workspace = _workspace_name_from_root(self._repo_root)
        self._output_dir = (output_dir or _bootstrap_artifact_root(self._repo_root)).resolve()
        self._server_name = server_name
        self._server_version = server_version
        self._rust_adapter = RustRouteAdapter(self._framework_root)
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
                mode=self._optional_str(arguments=arguments, key="mode", default="stable"),
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
            "task_id": result["payload"]["bootstrap"]["task_id"],
            "paths": result["paths"],
            "memory_items": result["memory_items"],
            "proposal_count": result["proposal_count"],
        }

    def _memory_recall(self, *, query: str, top: int, mode: str) -> JSONDict:
        try:
            payload = self._rust_adapter.framework_memory_recall(
                repo_root=self._repo_root,
                query=query,
                top=top,
                mode=mode,
            )
            return self._compact_memory_recall_payload(payload)
        except RuntimeError as error:
            raise FrameworkServerError(
                code="RUST_FRAMEWORK_MEMORY_RECALL_FAILED",
                message=str(error),
                suggested_next_actions=[
                    "verify scripts/router-rs builds cleanly",
                    "inspect .supervisor_state.json, artifacts/current, and .codex/memory for drift",
                ],
            ) from error

    def _skill_search(self, *, query: str, limit: int) -> JSONDict:
        exported = export_framework_skills()
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
                        " ".join(
                            str(item)
                            for item in row.get("trigger_hints", row.get("triggers", []))
                        ),
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
        try:
            return self._rust_adapter.framework_runtime_snapshot(repo_root=self._repo_root)
        except RuntimeError as error:
            raise FrameworkServerError(
                code="RUST_RUNTIME_SNAPSHOT_FAILED",
                message=str(error),
                suggested_next_actions=[
                    "verify scripts/router-rs builds cleanly",
                    "inspect active continuity artifacts under artifacts/current",
                ],
            ) from error

    def _contract_summary(self) -> JSONDict:
        try:
            return self._rust_adapter.framework_contract_summary(repo_root=self._repo_root)
        except RuntimeError as error:
            raise FrameworkServerError(
                code="RUST_CONTRACT_SUMMARY_FAILED",
                message=str(error),
                suggested_next_actions=[
                    "verify scripts/router-rs builds cleanly",
                    "inspect .supervisor_state.json and artifacts/current for drift",
                ],
            ) from error

    def _compact_memory_recall_payload(self, payload: JSONDict) -> JSONDict:
        retrieval = payload.get("retrieval") if isinstance(payload.get("retrieval"), dict) else {}
        continuity = payload.get("continuity") if isinstance(payload.get("continuity"), dict) else {}
        compact_retrieval: JSONDict = {
            "workspace": retrieval.get("workspace"),
            "topic": retrieval.get("topic"),
            "mode": retrieval.get("mode"),
            "memory_root": retrieval.get("memory_root"),
            "sqlite_path": retrieval.get("sqlite_path"),
            "active_task_id": retrieval.get("active_task_id"),
            "active_task_included": retrieval.get("active_task_included", False),
            "freshness": retrieval.get("freshness", {}),
            "items": retrieval.get("items", []),
        }
        compact_payload = dict(payload)
        compact_payload["retrieval"] = compact_retrieval
        compact_payload["continuity"] = {
            "state": continuity.get("state"),
            "can_resume": continuity.get("can_resume", False),
            "task": continuity.get("task"),
            "phase": continuity.get("phase"),
            "status": continuity.get("status"),
            "next_actions": continuity.get("next_actions", []),
            "blockers": continuity.get("blockers", []),
            "recovery_hints": continuity.get("recovery_hints", []),
            "current_execution": continuity.get("current_execution"),
            "recent_completed_execution": continuity.get("recent_completed_execution"),
        }
        compact_payload.pop("prompt_payload", None)
        compact_payload.pop("active_task", None)
        compact_payload.pop("focused_task", None)
        return compact_payload

    def _build_tool_definitions(self) -> dict[str, dict[str, Any]]:
        return {
            "framework_bootstrap_refresh": {
                "name": "framework_bootstrap_refresh",
                "description": (
                    "Refresh the local framework bootstrap bundle that packages skill routing, "
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
                "description": "Recall stable framework memory, with optional active/history/debug expansion modes.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "Topic or keyword to retrieve."},
                        "top": {"type": "integer", "minimum": 1, "description": "Maximum retrieved items."},
                        "mode": {
                            "type": "string",
                            "enum": ["stable", "active", "history", "debug"],
                            "description": "Recall mode. Defaults to stable.",
                        },
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
                "description": "Current framework bootstrap payload for this workspace.",
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
            text = self._read_project_memory_bundle()
            if not text:
                raise FrameworkServerError(
                    code="MISSING_RESOURCE",
                    message="Project memory file not found.",
                    suggested_next_actions=["refresh the bootstrap bundle", "verify the repository artifacts exist"],
                )
            return {"uri": uri, "mimeType": "text/markdown", "text": text}
        if uri == "framework://routing/runtime":
            path = self._repo_root / "skills" / "SKILL_ROUTING_RUNTIME.json"
            text = self._read_text_file(path=path, missing_message="Routing runtime file not found.")
            return {"uri": uri, "mimeType": "application/json", "text": text}
        if uri == "framework://bootstrap/default":
            path = resolve_bootstrap_path(self._output_dir)
            if not path.is_file():
                self._bootstrap_refresh(query="", top=8)
            text = self._read_text_file(path=path, missing_message="Bootstrap payload not found after refresh.")
            return {"uri": uri, "mimeType": "application/json", "text": text}
        if uri == "framework://supervisor/state":
            path = self._repo_root / ".supervisor_state.json"
            text = self._read_text_file(path=path, missing_message="Supervisor state file not found.")
            return {"uri": uri, "mimeType": "application/json", "text": text}
        if uri == "framework://artifacts/index":
            snapshot = self._runtime_snapshot()
            contract = self._contract_summary()
            payload = {
                "workspace": self._workspace,
                "collected_at": snapshot.get("collected_at"),
                "current_root": snapshot.get("current_root"),
                "continuity": snapshot.get("continuity", {}),
                "next_actions": contract.get("next_actions", []),
                "trace_skills": contract.get("trace_skills", []),
                "evidence_count": snapshot.get("evidence_count", 0),
                "paths": snapshot.get("paths", {}),
            }
            return {"uri": uri, "mimeType": "application/json", "text": json.dumps(payload, ensure_ascii=False, indent=2)}
        raise FrameworkServerError(
            code="INVALID_INPUT",
            message=f"Unknown resource URI: {uri}",
            suggested_next_actions=["call resources/list to inspect available resources"],
        )

    def _read_project_memory_bundle(self) -> str:
        documents: list[tuple[str, str]] = []
        memory_root = self._repo_root / ".codex" / "memory"
        for file_name in STABLE_MEMORY_FILENAMES:
            path = memory_root / file_name
            if not path.is_file():
                continue
            text = path.read_text(encoding="utf-8").strip()
            if text:
                documents.append((file_name, text))
        if not documents:
            return ""
        if len(documents) == 1 and documents[0][0] == "MEMORY.md":
            return documents[0][1]
        lines = ["# Project Memory Bundle", ""]
        for file_name, text in documents:
            lines.extend([f"## {file_name}", "", text, ""])
        return "\n".join(lines).strip()

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
