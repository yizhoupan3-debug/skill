"""CLI entrypoint for the framework MCP server."""

from __future__ import annotations

from .server import FrameworkMcpServer


def main() -> int:
    """Run the framework MCP server over stdio."""

    server = FrameworkMcpServer()
    return server.run_stdio_loop()


if __name__ == "__main__":
    raise SystemExit(main())
