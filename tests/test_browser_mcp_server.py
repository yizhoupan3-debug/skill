"""High-signal tests for the browser MCP skeleton."""

from __future__ import annotations

import io
import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.browser_mcp import BrowserMcpServer, InMemoryBrowserRuntime


def _call(server: BrowserMcpServer, request_id: int, method: str, params: dict) -> dict:
    """Send one in-process JSON-RPC request to the browser server.

    Parameters:
        server: Target server instance.
        request_id: JSON-RPC request identifier.
        method: JSON-RPC method name.
        params: Method parameters.

    Returns:
        dict: Decoded JSON-RPC response.
    """

    response = server.handle_request(
        {"jsonrpc": "2.0", "id": request_id, "method": method, "params": params}
    )
    assert response is not None
    return response


def _tool_call(server: BrowserMcpServer, request_id: int, name: str, arguments: dict) -> dict:
    """Call one MCP tool and return its structured content.

    Parameters:
        server: Target server instance.
        request_id: JSON-RPC request identifier.
        name: MCP tool name.
        arguments: Tool arguments.

    Returns:
        dict: Structured tool result.
    """

    response = _call(
        server=server,
        request_id=request_id,
        method="tools/call",
        params={"name": name, "arguments": arguments},
    )
    return response["result"]["structuredContent"]


def test_tools_list_includes_core_browser_surface() -> None:
    """Verify the server advertises the intended MVP browser tools.

    Parameters:
        None.

    Returns:
        None.
    """

    server = BrowserMcpServer(runtime=InMemoryBrowserRuntime())
    response = _call(server=server, request_id=1, method="tools/list", params={})
    tool_names = {tool["name"] for tool in response["result"]["tools"]}
    assert {
        "browser_open",
        "browser_tabs",
        "browser_close",
        "browser_get_state",
        "browser_get_elements",
        "browser_click",
        "browser_fill",
        "browser_wait_for",
    }.issubset(tool_names)


def test_login_flow_navigates_to_dashboard_with_delta() -> None:
    """Verify the deterministic login path returns actionable deltas.

    Parameters:
        None.

    Returns:
        None.
    """

    server = BrowserMcpServer(runtime=InMemoryBrowserRuntime())
    opened = _tool_call(
        server=server,
        request_id=2,
        name="browser_open",
        arguments={"url": "https://example.com/login"},
    )
    tab_id = opened["tab"]["tab_id"]
    state = _tool_call(
        server=server,
        request_id=3,
        name="browser_get_state",
        arguments={"tab_id": tab_id, "include": ["summary", "interactive_elements"]},
    )
    refs = {element["name"]: element["ref"] for element in state["interactive_elements"]}
    _tool_call(
        server=server,
        request_id=4,
        name="browser_fill",
        arguments={"tab_id": tab_id, "ref": refs["Email"], "value": "user@example.com"},
    )
    _tool_call(
        server=server,
        request_id=5,
        name="browser_fill",
        arguments={"tab_id": tab_id, "ref": refs["Password"], "value": "secret"},
    )
    clicked = _tool_call(
        server=server,
        request_id=6,
        name="browser_click",
        arguments={"tab_id": tab_id, "ref": refs["Sign in"]},
    )
    waited = _tool_call(
        server=server,
        request_id=7,
        name="browser_wait_for",
        arguments={
            "tab_id": tab_id,
            "condition": {"type": "url_contains", "value": "/dashboard"},
        },
    )
    assert clicked["delta"]["url_changed"] is True
    assert clicked["tab"]["url"].endswith("/dashboard")
    assert waited["ok"] is True


def test_stale_element_reference_returns_actionable_error() -> None:
    """Verify stale refs emit structured recovery guidance.

    Parameters:
        None.

    Returns:
        None.
    """

    server = BrowserMcpServer(runtime=InMemoryBrowserRuntime())
    opened = _tool_call(
        server=server,
        request_id=8,
        name="browser_open",
        arguments={"url": "https://example.com/login"},
    )
    tab_id = opened["tab"]["tab_id"]
    state = _tool_call(
        server=server,
        request_id=9,
        name="browser_get_state",
        arguments={"tab_id": tab_id, "include": ["interactive_elements"]},
    )
    sign_in_ref = next(
        element["ref"] for element in state["interactive_elements"] if element["name"] == "Sign in"
    )
    _call(
        server=server,
        request_id=10,
        method="tools/call",
        params={"name": "browser_click", "arguments": {"tab_id": tab_id, "ref": sign_in_ref}},
    )
    stale = _call(
        server=server,
        request_id=11,
        method="tools/call",
        params={"name": "browser_click", "arguments": {"tab_id": tab_id, "ref": sign_in_ref}},
    )
    assert stale["result"]["isError"] is True
    assert stale["result"]["structuredContent"]["error"]["code"] == "STALE_ELEMENT_REF"
    assert "browser_get_state" in stale["result"]["structuredContent"]["error"]["suggested_next_actions"][0]


def test_state_reports_unchanged_when_revision_matches() -> None:
    """Verify repeated state pulls can short-circuit unchanged pages.

    Parameters:
        None.

    Returns:
        None.
    """

    server = BrowserMcpServer(runtime=InMemoryBrowserRuntime())
    opened = _tool_call(
        server=server,
        request_id=12,
        name="browser_open",
        arguments={"url": "https://example.com/login"},
    )
    tab_id = opened["tab"]["tab_id"]
    revision = opened["tab"]["page_revision"]
    state = _tool_call(
        server=server,
        request_id=13,
        name="browser_get_state",
        arguments={"tab_id": tab_id, "since_revision": revision, "include": ["diff"]},
    )
    assert state["unchanged"] is True
    assert state["page_revision"] == revision


def test_stdio_loop_handles_initialize_and_tool_call() -> None:
    """Verify the stdio loop can process newline-delimited JSON-RPC traffic.

    Parameters:
        None.

    Returns:
        None.
    """

    server = BrowserMcpServer(runtime=InMemoryBrowserRuntime())
    stdin = io.StringIO(
        "\n".join(
            [
                json.dumps({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
                json.dumps({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
                "",
            ]
        )
    )
    stdout = io.StringIO()
    exit_code = server.run_stdio_loop(stdin=stdin, stdout=stdout)
    lines = [json.loads(line) for line in stdout.getvalue().strip().splitlines()]
    assert exit_code == 0
    assert lines[0]["result"]["serverInfo"]["name"] == "browser-mcp-skeleton"
    assert any(tool["name"] == "browser_open" for tool in lines[1]["result"]["tools"])


def test_invalid_arguments_return_recoverable_tool_errors() -> None:
    """Verify invalid tool inputs return tool-level recoverable errors.

    Parameters:
        None.

    Returns:
        None.
    """

    server = BrowserMcpServer(runtime=InMemoryBrowserRuntime())
    invalid = _call(
        server=server,
        request_id=14,
        method="tools/call",
        params={"name": "browser_open", "arguments": {}},
    )
    assert invalid["result"]["isError"] is True
    assert invalid["result"]["structuredContent"]["error"]["code"] == "INVALID_INPUT"


def test_invalid_wait_condition_and_bool_timeout_are_rejected() -> None:
    """Verify edge-case invalid inputs stay recoverable instead of crashing.

    Parameters:
        None.

    Returns:
        None.
    """

    server = BrowserMcpServer(runtime=InMemoryBrowserRuntime())
    missing_value = _call(
        server=server,
        request_id=15,
        method="tools/call",
        params={
            "name": "browser_wait_for",
            "arguments": {"condition": {"type": "url_contains"}},
        },
    )
    bad_timeout = _call(
        server=server,
        request_id=16,
        method="tools/call",
        params={
            "name": "browser_click",
            "arguments": {"ref": "el_continue", "timeout_ms": True},
        },
    )
    assert missing_value["result"]["isError"] is True
    assert missing_value["result"]["structuredContent"]["error"]["code"] == "INVALID_INPUT"
    assert bad_timeout["result"]["isError"] is True
    assert bad_timeout["result"]["structuredContent"]["error"]["code"] == "INVALID_INPUT"
