#!/usr/bin/env python3
"""Install the local browser MCP server into the user's Codex config."""

from __future__ import annotations

from pathlib import Path

CONFIG_PATH = Path.home() / ".codex" / "config.toml"
REPO_ROOT = Path(__file__).resolve().parents[1]


def build_server_block(repo_root: Path) -> str:
    """Build the browser MCP config block for the current repository.

    Parameters:
        repo_root: Repository root containing the browser-mcp script.

    Returns:
        str: TOML block ready to append to the Codex config.
    """

    command_path = repo_root / "tools" / "browser-mcp" / "scripts" / "start_browser_mcp.sh"
    return "\n".join(
        [
            "[mcp_servers.browser-mcp]",
            f'command = "{command_path}"',
        ]
    )


def install_server(config_path: Path, repo_root: Path) -> bool:
    """Install the browser MCP entry into a Codex config file.

    Parameters:
        config_path: Target Codex config path.
        repo_root: Repository root used to resolve the start script path.

    Returns:
        bool: True when a new entry was written, False when already present.
    """

    if not config_path.exists():
        raise SystemExit(f"Codex config not found: {config_path}")

    content = config_path.read_text(encoding="utf-8")
    if "[mcp_servers.browser-mcp]" in content:
        return False

    updated = content.rstrip() + "\n\n" + build_server_block(repo_root) + "\n"
    config_path.write_text(updated, encoding="utf-8")
    return True


def main() -> None:
    """Patch the Codex config so the local browser MCP server is available."""
    changed = install_server(config_path=CONFIG_PATH, repo_root=REPO_ROOT)
    if not changed:
        print("browser-mcp entry already exists in Codex config")
        return

    print(f"Installed browser-mcp into {CONFIG_PATH}")


if __name__ == "__main__":
    main()
