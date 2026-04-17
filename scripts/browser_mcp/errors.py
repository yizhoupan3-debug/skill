"""Structured error types for the browser MCP skeleton."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


@dataclass(slots=True)
class BrowserServerError(Exception):
    """Represent a structured, agent-actionable browser server error.

    Parameters:
        code: Stable machine-readable error code.
        message: Human-readable error summary.
        recoverable: Whether the caller can continue with another action.
        suggested_next_actions: Follow-up calls the agent should consider.
        data: Optional extra structured context for debugging.

    Returns:
        BrowserServerError: An exception instance that can be serialized.
    """

    code: str
    message: str
    recoverable: bool = True
    suggested_next_actions: list[str] = field(default_factory=list)
    data: dict[str, Any] | None = None

    def __post_init__(self) -> None:
        """Initialize the base Exception with the structured message.

        Parameters:
            None.

        Returns:
            None.
        """

        Exception.__init__(self, self.message)

    def to_payload(self) -> dict[str, Any]:
        """Serialize the error into an MCP-friendly payload.

        Parameters:
            None.

        Returns:
            dict[str, Any]: Structured error data for JSON responses.
        """

        return {
            "code": self.code,
            "message": self.message,
            "recoverable": self.recoverable,
            "suggested_next_actions": self.suggested_next_actions,
            "data": self.data or {},
        }
