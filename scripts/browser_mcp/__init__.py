"""Minimal browser MCP skeleton package."""

from .runtime import InMemoryBrowserRuntime
from .server import BrowserMcpServer

__all__ = ["BrowserMcpServer", "InMemoryBrowserRuntime"]
