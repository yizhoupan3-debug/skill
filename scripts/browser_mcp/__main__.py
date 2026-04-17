"""CLI entrypoint for the browser MCP skeleton server."""

from __future__ import annotations

from .server import BrowserMcpServer


def main() -> int:
    """Run the browser MCP skeleton over stdio.

    Parameters:
        None.

    Returns:
        int: Process exit code.
    """

    server = BrowserMcpServer()
    return server.run_stdio_loop()


if __name__ == "__main__":
    raise SystemExit(main())
