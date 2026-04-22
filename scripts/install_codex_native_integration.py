#!/usr/bin/env python3
"""Install the repo's native Codex integration pieces."""

from __future__ import annotations

import argparse
import json
import re
import shutil
import sys
from pathlib import Path
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.install_browser_mcp_codex import install_server as install_browser_server
from scripts.install_codex_framework_default import retire_overlay as retire_framework_overlay
from scripts.materialize_cli_host_entrypoints import CLAUDE_REFRESH_COMMAND
from scripts.host_integration_rs import run_host_integration_rs
from scripts.memory_support import get_repo_root, read_json_if_exists, write_json_if_changed, write_text_if_changed

HOME_CONFIG_PATH = Path.home() / ".codex" / "config.toml"
HOME_PLUGIN_ROOT = Path.home() / ".codex" / "plugins" / "skill-framework-native"
HOME_MARKETPLACE_PATH = Path.home() / ".agents" / "plugins" / "marketplace.json"
HOME_CODEX_SKILLS_PATH = Path.home() / ".codex" / "skills"
HOME_CLAUDE_REFRESH_PATH = Path.home() / ".claude" / "commands" / "refresh.md"
PROJECT_INSTRUCTIONS_PATH = Path(".codex") / "model_instructions.md"
REPO_MARKETPLACE_PATH = Path(".agents") / "plugins" / "marketplace.json"
PLUGIN_NAME = "skill-framework-native"
CONFIG_SCHEMA_HEADER = "#:schema https://developers.openai.com/codex/config-schema.json\n"
DEFAULT_TUI_STATUS_ITEMS = (
    "model-with-reasoning",
    "git-branch",
    "context-used",
    "fast-mode",
)

def ensure_config_file(config_path: Path) -> bool:
    """Ensure the target Codex config file exists with a schema header."""

    config_path.parent.mkdir(parents=True, exist_ok=True)
    if config_path.exists():
        return False
    config_path.write_text(CONFIG_SCHEMA_HEADER, encoding="utf-8")
    return True


def build_framework_server_block(repo_root: Path) -> str:
    """Build the framework MCP config block for the current repository."""

    return "\n".join(
        [
            "[mcp_servers.framework-mcp]",
            'command = "python3"',
            'args = ["-m", "scripts.framework_mcp"]',
            f'cwd = "{repo_root}"',
        ]
    )


def install_framework_server(config_path: Path, repo_root: Path) -> bool:
    """Install the framework MCP entry into a Codex config file."""

    content = config_path.read_text(encoding="utf-8") if config_path.exists() else ""
    if "[mcp_servers.framework-mcp]" in content:
        return False
    updated = content.rstrip() + ("\n\n" if content.strip() else "") + build_framework_server_block(repo_root) + "\n"
    return write_text_if_changed(config_path, updated)


def _format_status_line(items: tuple[str, ...] = DEFAULT_TUI_STATUS_ITEMS) -> str:
    return "status_line = [" + ", ".join(f'"{item}"' for item in items) + "]"


def ensure_tui_status_line(
    config_path: Path,
    *,
    status_items: tuple[str, ...] = DEFAULT_TUI_STATUS_ITEMS,
) -> bool:
    """Ensure Codex config has a stable TUI status line without clobbering other [tui] keys."""

    content = config_path.read_text(encoding="utf-8") if config_path.exists() else ""
    status_line = _format_status_line(status_items)
    tui_pattern = re.compile(r"(?ms)^\[tui\]\n.*?(?=^\[|\Z)")
    match = tui_pattern.search(content)
    if not match:
        updated = content.rstrip()
        if updated:
            updated += "\n\n"
        updated += "[tui]\n" + status_line + "\n"
        return write_text_if_changed(config_path, updated)

    block = match.group(0).rstrip("\n")
    lines = block.splitlines()
    replaced = False
    updated_lines: list[str] = []
    for line in lines:
        if re.match(r"^\s*status_line\s*=", line):
            updated_lines.append(status_line)
            replaced = True
            continue
        updated_lines.append(line)
    if not replaced:
        updated_lines.append(status_line)

    new_block = "\n".join(updated_lines) + "\n"
    updated = content[: match.start()] + new_block + content[match.end() :]
    return write_text_if_changed(config_path, updated)


def sync_directory(source: Path, destination: Path) -> bool:
    """Mirror one directory tree into another."""

    if not source.is_dir():
        raise FileNotFoundError(f"Plugin source directory not found: {source}")

    changed = False
    destination.mkdir(parents=True, exist_ok=True)

    source_children = {item.name: item for item in source.iterdir()}
    destination_children = {item.name: item for item in destination.iterdir()}

    for stale_name, stale_path in destination_children.items():
        if stale_name in source_children:
            continue
        changed = True
        if stale_path.is_dir():
            shutil.rmtree(stale_path)
        else:
            stale_path.unlink()

    for name, source_path in source_children.items():
        destination_path = destination / name
        if source_path.is_dir():
            changed = sync_directory(source_path, destination_path) or changed
            continue
        source_bytes = source_path.read_bytes()
        destination_bytes = destination_path.read_bytes() if destination_path.is_file() else None
        if destination_bytes == source_bytes:
            continue
        destination_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source_path, destination_path)
        changed = True

    return changed


def sync_personal_plugin_bundle(repo_root: Path, plugin_root: Path = HOME_PLUGIN_ROOT) -> bool:
    """Copy the repo-local plugin bundle into the user's Codex plugin directory."""

    source = repo_root / "plugins" / PLUGIN_NAME
    return sync_directory(source, plugin_root)


def ensure_home_codex_skills_link(
    repo_root: Path,
    *,
    target_path: Path = HOME_CODEX_SKILLS_PATH,
) -> bool:
    """Ensure ~/.codex/skills points at the repository skill library."""

    source = (repo_root / "skills").resolve()
    target_path.parent.mkdir(parents=True, exist_ok=True)

    if target_path.is_symlink():
        current_target = target_path.resolve()
        if current_target == source:
            return False
        target_path.unlink()
    elif target_path.exists():
        backup_path = target_path.with_name(target_path.name + ".bak")
        if backup_path.exists() or backup_path.is_symlink():
            if backup_path.is_dir() and not backup_path.is_symlink():
                shutil.rmtree(backup_path)
            else:
                backup_path.unlink()
        shutil.move(str(target_path), str(backup_path))

    target_path.symlink_to(source, target_is_directory=True)
    return True


def ensure_home_claude_refresh_command(
    command_path: Path = HOME_CLAUDE_REFRESH_PATH,
) -> bool:
    """Ensure the global Claude refresh command matches the repo canonical text."""

    return write_text_if_changed(command_path, CLAUDE_REFRESH_COMMAND)


def build_personal_marketplace_payload(
    plugin_root: Path = HOME_PLUGIN_ROOT,
    *,
    marketplace_root: Path | None = None,
    existing_marketplace: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Build a personal marketplace payload that exposes the framework plugin."""

    payload = existing_marketplace.copy() if isinstance(existing_marketplace, dict) else {}
    payload["name"] = payload.get("name") or "skill-personal-marketplace"
    interface = payload.get("interface")
    payload["interface"] = interface if isinstance(interface, dict) else {}
    payload["interface"]["displayName"] = payload["interface"].get("displayName") or "Skill Personal Marketplace"

    relative_base = (marketplace_root or Path.home()).resolve()
    plugin_path = f"./{plugin_root.resolve().relative_to(relative_base)}"
    plugins = payload.get("plugins")
    plugin_rows = [row for row in plugins if isinstance(row, dict)] if isinstance(plugins, list) else []
    updated_plugins: list[dict[str, Any]] = []
    replaced = False

    for row in plugin_rows:
        if row.get("name") != PLUGIN_NAME:
            updated_plugins.append(row)
            continue
        replaced = True
        updated_plugins.append(
            {
                "name": PLUGIN_NAME,
                "source": {"source": "local", "path": plugin_path},
                "policy": {"installation": "AVAILABLE", "authentication": "ON_INSTALL"},
                "category": row.get("category", "Developer Tools"),
            }
        )

    if not replaced:
        updated_plugins.append(
            {
                "name": PLUGIN_NAME,
                "source": {"source": "local", "path": plugin_path},
                "policy": {"installation": "AVAILABLE", "authentication": "ON_INSTALL"},
                "category": "Developer Tools",
            }
        )

    payload["plugins"] = updated_plugins
    return payload


def install_personal_marketplace(
    marketplace_path: Path = HOME_MARKETPLACE_PATH,
    *,
    plugin_root: Path = HOME_PLUGIN_ROOT,
) -> bool:
    """Ensure the user's personal plugin marketplace exposes the framework plugin."""

    existing = read_json_if_exists(marketplace_path)
    payload = build_personal_marketplace_payload(
        plugin_root,
        marketplace_root=marketplace_path.resolve().parents[2],
        existing_marketplace=existing,
    )
    return write_json_if_changed(marketplace_path, payload)


def install_native_integration(
    *,
    home_config_path: Path = HOME_CONFIG_PATH,
    repo_root: Path | None = None,
    home_plugin_root: Path = HOME_PLUGIN_ROOT,
    home_marketplace_path: Path = HOME_MARKETPLACE_PATH,
    home_codex_skills_path: Path = HOME_CODEX_SKILLS_PATH,
    home_claude_refresh_path: Path = HOME_CLAUDE_REFRESH_PATH,
    project_instructions_path: Path = PROJECT_INSTRUCTIONS_PATH,
    install_browser_mcp: bool = True,
    install_framework_mcp: bool = True,
    retire_framework_overlay_file: bool = True,
    install_personal_plugin: bool = True,
    install_personal_marketplace_entry: bool = True,
    install_home_codex_skills_link: bool = True,
    install_home_claude_refresh_command: bool = True,
) -> dict[str, Any]:
    """Install the repo's Codex-native integration surfaces."""

    resolved_repo_root = (repo_root or get_repo_root()).resolve()
    created_config = ensure_config_file(home_config_path)
    browser_changed = False
    framework_changed = False
    personal_plugin_changed = False
    personal_marketplace_changed = False
    home_codex_skills_link_changed = False
    home_claude_refresh_changed = False
    framework_overlay_result: dict[str, Any] | None = None
    if install_browser_mcp:
        browser_changed = install_browser_server(config_path=home_config_path, repo_root=resolved_repo_root)
    if install_framework_mcp:
        framework_changed = install_framework_server(config_path=home_config_path, repo_root=resolved_repo_root)
    tui_changed = ensure_tui_status_line(home_config_path)
    if install_personal_plugin:
        personal_plugin_changed = sync_personal_plugin_bundle(resolved_repo_root, plugin_root=home_plugin_root)
    if install_personal_marketplace_entry:
        personal_marketplace_changed = install_personal_marketplace(
            marketplace_path=home_marketplace_path,
            plugin_root=home_plugin_root,
        )
    if install_home_codex_skills_link:
        home_codex_skills_link_changed = ensure_home_codex_skills_link(
            resolved_repo_root,
            target_path=home_codex_skills_path,
        )
    if install_home_claude_refresh_command:
        home_claude_refresh_changed = ensure_home_claude_refresh_command(home_claude_refresh_path)
    if retire_framework_overlay_file:
        framework_overlay_result = retire_framework_overlay(
            (resolved_repo_root / project_instructions_path).resolve()
        )
    return {
        "success": True,
        "repo_root": str(resolved_repo_root),
        "home_config_path": str(home_config_path),
        "home_plugin_root": str(home_plugin_root),
        "home_marketplace_path": str(home_marketplace_path),
        "home_codex_skills_path": str(home_codex_skills_path),
        "home_claude_refresh_path": str(home_claude_refresh_path),
        "repo_marketplace_path": str((resolved_repo_root / REPO_MARKETPLACE_PATH).resolve()),
        "created_config": created_config,
        "browser_mcp_changed": browser_changed,
        "framework_mcp_changed": framework_changed,
        "tui_status_line_changed": tui_changed,
        "personal_plugin_changed": personal_plugin_changed,
        "personal_marketplace_changed": personal_marketplace_changed,
        "home_codex_skills_link_changed": home_codex_skills_link_changed,
        "home_claude_refresh_changed": home_claude_refresh_changed,
        "framework_overlay_retirement": framework_overlay_result,
    }


def install_native_integration(
    *,
    home_config_path: Path = HOME_CONFIG_PATH,
    repo_root: Path | None = None,
    home_plugin_root: Path = HOME_PLUGIN_ROOT,
    home_marketplace_path: Path = HOME_MARKETPLACE_PATH,
    home_codex_skills_path: Path = HOME_CODEX_SKILLS_PATH,
    home_claude_refresh_path: Path = HOME_CLAUDE_REFRESH_PATH,
    project_instructions_path: Path = PROJECT_INSTRUCTIONS_PATH,
    install_browser_mcp: bool = True,
    install_framework_mcp: bool = True,
    retire_framework_overlay_file: bool = True,
    install_personal_plugin: bool = True,
    install_personal_marketplace_entry: bool = True,
    install_home_codex_skills_link: bool = True,
    install_home_claude_refresh_command: bool = True,
) -> dict[str, Any]:
    """Rust-owned installer wrapper kept behind the legacy Python API."""

    resolved_repo_root = (repo_root or get_repo_root()).resolve()
    command = [
        "install-native-integration",
        "--template-root",
        str(Path(__file__).resolve().parents[1]),
        "--repo-root",
        str(resolved_repo_root),
        "--home-config-path",
        str(home_config_path),
        "--home-plugin-root",
        str(home_plugin_root),
        "--home-marketplace-path",
        str(home_marketplace_path),
        "--home-codex-skills-path",
        str(home_codex_skills_path),
        "--home-claude-refresh-path",
        str(home_claude_refresh_path),
        "--project-instructions-path",
        str(project_instructions_path),
    ]
    if not install_browser_mcp:
        command.append("--skip-browser-mcp")
    if not install_framework_mcp:
        command.append("--skip-framework-mcp")
    if not retire_framework_overlay_file:
        command.append("--skip-framework-overlay-retirement")
    if not install_personal_plugin:
        command.append("--skip-personal-plugin")
    if not install_personal_marketplace_entry:
        command.append("--skip-personal-marketplace")
    if not install_home_codex_skills_link:
        command.append("--skip-home-codex-skills-link")
    if not install_home_claude_refresh_command:
        command.append("--skip-home-claude-refresh")
    return run_host_integration_rs(*command)


def main() -> int:
    parser = argparse.ArgumentParser(description="Install the repo's native Codex integration.")
    parser.add_argument("--home-config-path", type=Path, default=HOME_CONFIG_PATH)
    parser.add_argument("--home-plugin-root", type=Path, default=HOME_PLUGIN_ROOT)
    parser.add_argument("--home-marketplace-path", type=Path, default=HOME_MARKETPLACE_PATH)
    parser.add_argument("--home-codex-skills-path", type=Path, default=HOME_CODEX_SKILLS_PATH)
    parser.add_argument("--home-claude-refresh-path", type=Path, default=HOME_CLAUDE_REFRESH_PATH)
    parser.add_argument("--project-instructions-path", type=Path, default=PROJECT_INSTRUCTIONS_PATH)
    parser.add_argument("--repo-root", type=Path, default=None)
    parser.add_argument("--skip-browser-mcp", action="store_true")
    parser.add_argument("--skip-framework-mcp", action="store_true")
    parser.add_argument("--skip-framework-overlay-retirement", action="store_true")
    parser.add_argument("--skip-personal-plugin", action="store_true")
    parser.add_argument("--skip-personal-marketplace", action="store_true")
    parser.add_argument("--skip-home-codex-skills-link", action="store_true")
    parser.add_argument("--skip-home-claude-refresh", action="store_true")
    parser.add_argument("--json", action="store_true", dest="json_output")
    args = parser.parse_args()
    payload = install_native_integration(
        home_config_path=args.home_config_path,
        home_plugin_root=args.home_plugin_root,
        home_marketplace_path=args.home_marketplace_path,
        home_codex_skills_path=args.home_codex_skills_path,
        home_claude_refresh_path=args.home_claude_refresh_path,
        repo_root=args.repo_root,
        project_instructions_path=args.project_instructions_path,
        install_browser_mcp=not args.skip_browser_mcp,
        install_framework_mcp=not args.skip_framework_mcp,
        retire_framework_overlay_file=not args.skip_framework_overlay_retirement,
        install_personal_plugin=not args.skip_personal_plugin,
        install_personal_marketplace_entry=not args.skip_personal_marketplace,
        install_home_codex_skills_link=not args.skip_home_codex_skills_link,
        install_home_claude_refresh_command=not args.skip_home_claude_refresh,
    )
    if args.json_output:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    else:
        print(payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
