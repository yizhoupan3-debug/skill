"""Minimal MCP-compatible stdio server for browser tool experiments."""

from __future__ import annotations

import json
import sys
from typing import Any, TextIO

from .errors import BrowserServerError
from .models import WaitCondition
from .runtime import BrowserRuntime, InMemoryBrowserRuntime
from .schema import ToolDefinition, build_tool_definitions


JSONDict = dict[str, Any]


class BrowserMcpServer:
    """Implement a tiny MCP-style JSON-RPC server for browser tools.

    Parameters:
        runtime: Browser runtime backend used for tool execution.
        server_name: Public server name reported during initialize.
        server_version: Public server version reported during initialize.

    Returns:
        BrowserMcpServer: Server instance ready for request handling.
    """

    def __init__(
        self,
        runtime: BrowserRuntime | None = None,
        server_name: str = "browser-mcp-skeleton",
        server_version: str = "0.1.0",
    ) -> None:
        """Initialize the server and register the tool surface.

        Parameters:
            runtime: Optional runtime backend. Defaults to the in-memory backend.
            server_name: Server name reported to clients.
            server_version: Server version reported to clients.

        Returns:
            None.
        """

        self._runtime = runtime or InMemoryBrowserRuntime()
        self._server_name = server_name
        self._server_version = server_version
        self._tools = {tool.name: tool for tool in build_tool_definitions()}

    def handle_request(self, request: JSONDict) -> JSONDict | None:
        """Handle a single JSON-RPC request dictionary.

        Parameters:
            request: Decoded JSON-RPC request payload.

        Returns:
            dict[str, Any] | None: JSON-RPC response or None for notifications.
        """

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
                result = self._handle_tools_list()
            elif method == "tools/call":
                result = self._handle_tools_call(params=params)
            else:
                raise BrowserServerError(
                    code="UNSUPPORTED_OPERATION",
                    message=f"Unsupported JSON-RPC method: {method}",
                    suggested_next_actions=["call initialize", "call tools/list", "call tools/call"],
                )
            return self._success_response(request_id=request_id, result=result)
        except BrowserServerError as error:
            return self._error_response(request_id=request_id, error=error)
        except Exception as error:  # pragma: no cover - defensive protocol guard
            return self._error_response(
                request_id=request_id,
                error=BrowserServerError(
                    code="INTERNAL_ERROR",
                    message=f"Unhandled server error: {error}",
                    recoverable=False,
                    suggested_next_actions=["inspect server logs", "retry with narrower inputs"],
                ),
            )

    def run_stdio_loop(self, stdin: TextIO | None = None, stdout: TextIO | None = None) -> int:
        """Run the line-delimited stdio request loop.

        Parameters:
            stdin: Optional input stream. Defaults to sys.stdin.
            stdout: Optional output stream. Defaults to sys.stdout.

        Returns:
            int: Process exit code.
        """

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
                    error=BrowserServerError(
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
        """Return the server initialize response payload.

        Parameters:
            None.

        Returns:
            dict[str, Any]: MCP initialize result payload.
        """

        return {
            "protocolVersion": "2024-11-05",
            "serverInfo": {"name": self._server_name, "version": self._server_version},
            "capabilities": {"tools": {"listChanged": False}},
        }

    def _handle_tools_list(self) -> JSONDict:
        """Return the registered browser tool definitions.

        Parameters:
            None.

        Returns:
            dict[str, Any]: Tool listing payload.
        """

        return {"tools": [tool.to_dict() for tool in self._tools.values()]}

    def _handle_tools_call(self, params: JSONDict) -> JSONDict:
        """Dispatch a tool call to the selected runtime method.

        Parameters:
            params: tools/call parameter object with name and arguments.

        Returns:
            dict[str, Any]: MCP tool result payload.
        """

        tool_name = params.get("name")
        if tool_name not in self._tools:
            raise BrowserServerError(
                code="INVALID_INPUT",
                message=f"Unknown tool name: {tool_name}",
                suggested_next_actions=["call tools/list to inspect available tools"],
            )
        arguments = params.get("arguments", {})
        try:
            structured = self._call_tool(tool_name=tool_name, arguments=arguments)
            return {
                "structuredContent": structured,
                "content": [{"type": "text", "text": json.dumps(structured, ensure_ascii=False)}],
                "isError": False,
            }
        except BrowserServerError as error:
            structured = {"ok": False, "error": error.to_payload()}
            return {
                "structuredContent": structured,
                "content": [{"type": "text", "text": json.dumps(structured, ensure_ascii=False)}],
                "isError": True,
            }

    def _call_tool(self, tool_name: str, arguments: JSONDict) -> JSONDict:
        """Execute one browser tool against the runtime backend.

        Parameters:
            tool_name: Registered tool name.
            arguments: Decoded tool arguments.

        Returns:
            dict[str, Any]: Tool-specific result payload.
        """

        if tool_name == "browser_open":
            return self._runtime.open_page(
                url=self._require_str(arguments=arguments, key="url"),
                new_tab=self._optional_bool(arguments=arguments, key="new_tab", default=False),
            )
        if tool_name == "browser_tabs":
            return self._runtime.tabs(
                action=self._require_str(arguments=arguments, key="action"),
                tab_id=arguments.get("tab_id"),
            )
        if tool_name == "browser_close":
            return self._runtime.close(
                target=self._require_str(arguments=arguments, key="target"),
                tab_id=arguments.get("tab_id"),
            )
        if tool_name == "browser_get_state":
            return self._runtime.get_state(
                tab_id=arguments.get("tab_id"),
                include=self._optional_list(
                    arguments=arguments,
                    key="include",
                    default=["summary", "interactive_elements"],
                ),
                since_revision=arguments.get("since_revision"),
                max_elements=self._optional_int(arguments=arguments, key="max_elements", default=20, minimum=1),
                text_budget=self._optional_int(arguments=arguments, key="text_budget", default=1200, minimum=0),
            )
        if tool_name == "browser_get_elements":
            return self._runtime.get_elements(
                tab_id=arguments.get("tab_id"),
                role=arguments.get("role"),
                query=arguments.get("query"),
                scope_ref=arguments.get("scope_ref"),
                limit=self._optional_int(arguments=arguments, key="limit", default=10, minimum=1),
            )
        if tool_name == "browser_click":
            return self._runtime.click(
                tab_id=arguments.get("tab_id"),
                ref=self._require_str(arguments=arguments, key="ref"),
                timeout_ms=self._optional_int(arguments=arguments, key="timeout_ms", default=5000, minimum=0),
            )
        if tool_name == "browser_fill":
            return self._runtime.fill(
                tab_id=arguments.get("tab_id"),
                ref=self._require_str(arguments=arguments, key="ref"),
                value=self._require_str(arguments=arguments, key="value"),
                submit=self._optional_bool(arguments=arguments, key="submit", default=False),
            )
        if tool_name == "browser_wait_for":
            return self._runtime.wait_for(
                tab_id=arguments.get("tab_id"),
                condition=self._build_wait_condition(arguments=arguments),
                timeout_ms=self._optional_int(arguments=arguments, key="timeout_ms", default=5000, minimum=0),
            )
        raise BrowserServerError(
            code="UNSUPPORTED_OPERATION",
            message=f"Tool is registered but not implemented: {tool_name}",
            suggested_next_actions=["call tools/list to inspect supported tools"],
        )

    def _require_str(self, arguments: JSONDict, key: str) -> str:
        """Read a required string argument with validation.

        Parameters:
            arguments: Raw tool arguments.
            key: Required argument key.

        Returns:
            str: Validated string value.
        """

        value = arguments.get(key)
        if not isinstance(value, str) or not value:
            raise BrowserServerError(
                code="INVALID_INPUT",
                message=f"Argument '{key}' must be a non-empty string.",
                suggested_next_actions=[f"retry tools/call with a valid '{key}' string"],
            )
        return value

    def _require_object(self, arguments: JSONDict, key: str) -> JSONDict:
        """Read a required object argument with validation.

        Parameters:
            arguments: Raw tool arguments.
            key: Required object key.

        Returns:
            dict[str, Any]: Validated nested object.
        """

        value = arguments.get(key)
        if not isinstance(value, dict):
            raise BrowserServerError(
                code="INVALID_INPUT",
                message=f"Argument '{key}' must be an object.",
                suggested_next_actions=[f"retry tools/call with an object for '{key}'"],
            )
        return value

    def _optional_int(self, arguments: JSONDict, key: str, default: int, minimum: int) -> int:
        """Read an optional integer argument with lower-bound validation.

        Parameters:
            arguments: Raw tool arguments.
            key: Integer argument key.
            default: Fallback value when the argument is absent.
            minimum: Inclusive minimum accepted value.

        Returns:
            int: Validated integer value.
        """

        value = arguments.get(key, default)
        if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
            raise BrowserServerError(
                code="INVALID_INPUT",
                message=f"Argument '{key}' must be an integer >= {minimum}.",
                suggested_next_actions=[f"retry tools/call with a valid '{key}' integer"],
            )
        return value

    def _optional_bool(self, arguments: JSONDict, key: str, default: bool) -> bool:
        """Read an optional boolean argument with validation.

        Parameters:
            arguments: Raw tool arguments.
            key: Boolean argument key.
            default: Fallback value when the argument is absent.

        Returns:
            bool: Validated boolean value.
        """

        value = arguments.get(key, default)
        if not isinstance(value, bool):
            raise BrowserServerError(
                code="INVALID_INPUT",
                message=f"Argument '{key}' must be a boolean.",
                suggested_next_actions=[f"retry tools/call with a boolean for '{key}'"],
            )
        return value

    def _optional_list(self, arguments: JSONDict, key: str, default: list[str]) -> list[str]:
        """Read an optional string list argument with validation.

        Parameters:
            arguments: Raw tool arguments.
            key: List argument key.
            default: Fallback list when the argument is absent.

        Returns:
            list[str]: Validated string list.
        """

        value = arguments.get(key, default)
        if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
            raise BrowserServerError(
                code="INVALID_INPUT",
                message=f"Argument '{key}' must be a list of strings.",
                suggested_next_actions=[f"retry tools/call with a string list for '{key}'"],
            )
        return value

    def _build_wait_condition(self, arguments: JSONDict) -> WaitCondition:
        """Validate and build the wait condition object.

        Parameters:
            arguments: Raw tool arguments.

        Returns:
            WaitCondition: Validated wait condition.
        """

        condition = self._require_object(arguments=arguments, key="condition")
        condition_type = condition.get("type")
        condition_value = condition.get("value")
        if not isinstance(condition_type, str) or not condition_type:
            raise BrowserServerError(
                code="INVALID_INPUT",
                message="Wait condition 'type' must be a non-empty string.",
                suggested_next_actions=["retry browser_wait_for with a valid condition.type"],
            )
        if not isinstance(condition_value, str):
            raise BrowserServerError(
                code="INVALID_INPUT",
                message="Wait condition 'value' must be a string.",
                suggested_next_actions=["retry browser_wait_for with a string condition.value"],
            )
        return WaitCondition.from_dict({"type": condition_type, "value": condition_value})

    def _success_response(self, request_id: Any, result: JSONDict) -> JSONDict:
        """Wrap a successful payload in JSON-RPC response format.

        Parameters:
            request_id: Original JSON-RPC request identifier.
            result: Successful method result payload.

        Returns:
            dict[str, Any]: JSON-RPC success response.
        """

        return {"jsonrpc": "2.0", "id": request_id, "result": result}

    def _error_response(self, request_id: Any, error: BrowserServerError) -> JSONDict:
        """Wrap a structured server error in JSON-RPC response format.

        Parameters:
            request_id: Original JSON-RPC request identifier.
            error: Structured browser server error.

        Returns:
            dict[str, Any]: JSON-RPC error response.
        """

        return {
            "jsonrpc": "2.0",
            "id": request_id,
            "error": {
                "code": -32000,
                "message": error.message,
                "data": error.to_payload(),
            },
        }
