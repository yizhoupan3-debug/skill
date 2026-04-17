"""Tool schema definitions for the browser MCP skeleton."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any


JSONDict = dict[str, Any]


@dataclass(frozen=True, slots=True)
class ToolDefinition:
    """Describe an MCP tool exposed by the browser server.

    Parameters:
        name: Stable tool name.
        description: Agent-facing usage guidance.
        input_schema: JSON Schema for tool arguments.

    Returns:
        ToolDefinition: Tool definition instance.
    """

    name: str
    description: str
    input_schema: JSONDict

    def to_dict(self) -> JSONDict:
        """Serialize the tool definition into the MCP list format.

        Parameters:
            None.

        Returns:
            dict[str, Any]: Tool list payload entry.
        """

        return {
            "name": self.name,
            "description": self.description,
            "inputSchema": self.input_schema,
        }


def build_tool_definitions() -> list[ToolDefinition]:
    """Build the v1 browser tool surface for the skeleton server.

    Parameters:
        None.

    Returns:
        list[ToolDefinition]: Core browser tool definitions.
    """

    return [
        ToolDefinition(
            name="browser_open",
            description="Open a page in the current tab or a new tab and return session state.",
            input_schema={
                "type": "object",
                "properties": {
                    "url": {"type": "string", "minLength": 1},
                    "new_tab": {"type": "boolean", "default": False},
                },
                "required": ["url"],
                "additionalProperties": False,
            },
        ),
        ToolDefinition(
            name="browser_tabs",
            description="List open tabs or select the active tab.",
            input_schema={
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["list", "select"]},
                    "tab_id": {"type": "string"},
                },
                "required": ["action"],
                "additionalProperties": False,
            },
        ),
        ToolDefinition(
            name="browser_close",
            description="Close a single tab or the full browser session.",
            input_schema={
                "type": "object",
                "properties": {
                    "target": {"type": "string", "enum": ["tab", "session"]},
                    "tab_id": {"type": "string"},
                },
                "required": ["target"],
                "additionalProperties": False,
            },
        ),
        ToolDefinition(
            name="browser_get_state",
            description="Return a compressed page snapshot with optional delta reporting.",
            input_schema={
                "type": "object",
                "properties": {
                    "tab_id": {"type": "string"},
                    "include": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": ["summary", "interactive_elements", "diff"],
                        },
                        "default": ["summary", "interactive_elements"],
                    },
                    "since_revision": {"type": "integer", "minimum": 0},
                    "max_elements": {"type": "integer", "minimum": 1, "default": 20},
                    "text_budget": {"type": "integer", "minimum": 0, "default": 1200},
                },
                "additionalProperties": False,
            },
        ),
        ToolDefinition(
            name="browser_get_elements",
            description="Filter interactive elements by role and query string.",
            input_schema={
                "type": "object",
                "properties": {
                    "tab_id": {"type": "string"},
                    "role": {"type": "string"},
                    "query": {"type": "string"},
                    "scope_ref": {"type": "string"},
                    "limit": {"type": "integer", "minimum": 1, "default": 10},
                },
                "additionalProperties": False,
            },
        ),
        ToolDefinition(
            name="browser_click",
            description="Click an element reference and return the resulting page delta.",
            input_schema={
                "type": "object",
                "properties": {
                    "tab_id": {"type": "string"},
                    "ref": {"type": "string", "minLength": 1},
                    "timeout_ms": {"type": "integer", "minimum": 0, "default": 5000},
                },
                "required": ["ref"],
                "additionalProperties": False,
            },
        ),
        ToolDefinition(
            name="browser_fill",
            description="Fill an element reference with a value and optionally submit.",
            input_schema={
                "type": "object",
                "properties": {
                    "tab_id": {"type": "string"},
                    "ref": {"type": "string", "minLength": 1},
                    "value": {"type": "string"},
                    "submit": {"type": "boolean", "default": False},
                },
                "required": ["ref", "value"],
                "additionalProperties": False,
            },
        ),
        ToolDefinition(
            name="browser_wait_for",
            description="Wait for url, text, element, or network-idle conditions.",
            input_schema={
                "type": "object",
                "properties": {
                    "tab_id": {"type": "string"},
                    "condition": {
                        "type": "object",
                        "properties": {
                            "type": {
                                "type": "string",
                                "enum": [
                                    "url_contains",
                                    "text_appears",
                                    "element_appears",
                                    "network_idle",
                                ],
                            },
                            "value": {"type": "string"},
                        },
                        "required": ["type", "value"],
                        "additionalProperties": False,
                    },
                    "timeout_ms": {"type": "integer", "minimum": 0, "default": 5000},
                },
                "required": ["condition"],
                "additionalProperties": False,
            },
        ),
    ]
