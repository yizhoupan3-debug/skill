#!/usr/bin/env python3
"""Install the local browser MCP server into the user's Codex config."""

from __future__ import annotations

import re
from pathlib import Path

CONFIG_PATH = Path.home() / ".codex" / "config.toml"
REPO_ROOT = Path(__file__).resolve().parents[1]
CONFIG_SCHEMA_HEADER = "#:schema https://developers.openai.com/codex/config-schema.json\n"
BROWSER_SERVER_PATTERN = re.compile(r"(?ms)^\[mcp_servers\.browser-mcp\]\n.*?(?=^\[|\Z)")


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


def ensure_config_file(config_path: Path) -> bool:
    """Ensure the target Codex config file exists with a schema header."""

    config_path.parent.mkdir(parents=True, exist_ok=True)
    if config_path.exists():
        return False
    config_path.write_text(CONFIG_SCHEMA_HEADER, encoding="utf-8")
    return True


def _upsert_browser_server_block(content: str, block: str) -> str:
    """Replace or append the browser-mcp block without disturbing other tables."""

    match = BROWSER_SERVER_PATTERN.search(content)
    if match:
        existing = match.group(0).rstrip("\n")
        if existing == block:
            return content
        return content[: match.start()] + block + "\n" + content[match.end() :]
    return content.rstrip() + ("\n\n" if content.strip() else "") + block + "\n"


def install_server(config_path: Path, repo_root: Path) -> bool:
    """Install the browser MCP entry into a Codex config file.

    Parameters:
        config_path: Target Codex config path.
        repo_root: Repository root used to resolve the start script path.

    Returns:
        bool: True when a new entry was written, False when already present.
    """

    ensure_config_file(config_path)
    content = config_path.read_text(encoding="utf-8")
    updated = _upsert_browser_server_block(content, build_server_block(repo_root))
    config_path.write_text(updated, encoding="utf-8")
    return updated != content


def main() -> None:
    """Patch the Codex config so the local browser MCP server is available."""
    changed = install_server(config_path=CONFIG_PATH, repo_root=REPO_ROOT)
    if not changed:
        print("browser-mcp entry already exists in Codex config")
        return

    print(f"Installed browser-mcp into {CONFIG_PATH}")


if __name__ == "__main__":
    main()
