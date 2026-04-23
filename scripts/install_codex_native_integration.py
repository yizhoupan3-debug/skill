#!/usr/bin/env python3
"""Install the repo's native Codex integration pieces."""

from __future__ import annotations

import argparse
import json
import re
import shutil
import sys
from pathlib import Path
from tempfile import TemporaryDirectory
from typing import Any

if __package__ in {None, ""}:
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.default_bootstrap import resolve_bootstrap_path, run_default_bootstrap
from scripts.materialize_cli_host_entrypoints import (
    CLAUDE_REFRESH_COMMAND,
    write_host_entrypoint_template,
)
from scripts.host_integration_rs import export_runtime_registry, run_host_integration_rs
from scripts.memory_support import (
    bootstrap_artifact_root,
    get_repo_root,
    read_json_if_exists,
    write_json_if_changed,
    write_text_if_changed,
)

HOME_CONFIG_PATH = Path.home() / ".codex" / "config.toml"
HOME_PLUGIN_ROOT = Path.home() / ".codex" / "plugins" / "skill-framework-native"
HOME_MARKETPLACE_PATH = Path.home() / ".agents" / "plugins" / "marketplace.json"
HOME_CODEX_SKILLS_PATH = Path.home() / ".codex" / "skills"
HOME_CLAUDE_SKILLS_PATH = Path.home() / ".claude" / "skills"
HOME_CLAUDE_REFRESH_PATH = Path.home() / ".claude" / "commands" / "refresh.md"
HOME_CLAUDE_MCP_CONFIG_PATH = Path.home() / ".claude.json"
PROJECT_INSTRUCTIONS_PATH = Path(".codex") / "model_instructions.md"
REPO_MARKETPLACE_PATH = Path(".agents") / "plugins" / "marketplace.json"
PLUGIN_NAME = "skill-framework-native"
DEFAULT_PLUGIN_CATEGORY = "Developer Tools"
CONFIG_SCHEMA_HEADER = "#:schema https://developers.openai.com/codex/config-schema.json\n"
OPENAI_DEVELOPER_DOCS_MCP_URL = "https://developers.openai.com/mcp"
DEFAULT_TUI_STATUS_ITEMS = (
    "model-with-reasoning",
    "git-branch",
    "context-used",
    "fast-mode",
)
PERSONAL_PLUGIN_LIVE_PROJECTION_EXCLUDES = frozenset({"skills", ".mcp.json"})
FRAMEWORK_SERVER_PATTERN = re.compile(r"(?ms)^\[mcp_servers\.framework-mcp\]\n.*?(?=^\[|\Z)")
OPENAI_DEVELOPER_DOCS_SERVER_PATTERN = re.compile(r"(?ms)^\[mcp_servers\.openaiDeveloperDocs\]\n.*?(?=^\[|\Z)")
FEATURES_BLOCK_PATTERN = re.compile(r"(?ms)^\[features\]\n.*?(?=^\[|\Z)")


def _bootstrap_payload_matches_contract(payload: dict[str, Any], repo_root: Path) -> bool:
    """Return whether one bootstrap payload still matches the current repo contract."""

    bootstrap = payload.get("bootstrap")
    memory = payload.get("memory-bootstrap")
    skills = payload.get("skills-export")
    proposals = payload.get("evolution-proposals")
    return (
        isinstance(bootstrap, dict)
        and bootstrap.get("repo_root") == str(repo_root.resolve())
        and isinstance(memory, dict)
        and isinstance(skills, dict)
        and skills.get("source") == "skills/SKILL_ROUTING_RUNTIME.json"
        and isinstance(proposals, dict)
    )


def _bootstrap_file_matches_contract(path: Path, repo_root: Path) -> bool:
    """Return whether an existing bootstrap file is usable as-is."""

    payload = read_json_if_exists(path)
    return bool(payload) and _bootstrap_payload_matches_contract(payload, repo_root)


def _upsert_named_toml_block(content: str, *, block: str, pattern: re.Pattern[str]) -> str:
    """Replace or append one named TOML block while preserving surrounding content."""

    match = pattern.search(content)
    if match:
        existing = match.group(0).rstrip("\n")
        if existing == block:
            return content
        replacement = block + "\n"
        return content[: match.start()] + replacement + content[match.end() :]
    return content.rstrip() + ("\n\n" if content.strip() else "") + block + "\n"


def _runtime_registry_payload(repo_root: Path | None = None) -> dict[str, Any]:
    payload = export_runtime_registry(repo_root)
    if not isinstance(payload, dict):
        raise ValueError("Rust runtime registry export must be a JSON object.")
    return payload


def _primary_plugin_record(repo_root: Path | None = None) -> dict[str, Any]:
    records = _runtime_registry_payload(repo_root).get("plugins")
    if not isinstance(records, list) or not records:
        raise ValueError("Runtime registry must define at least one plugin record.")
    record = records[0]
    if not isinstance(record, dict):
        raise ValueError("Runtime registry plugin record must be an object.")
    return record


def _workspace_bootstrap_defaults(repo_root: Path | None = None) -> dict[str, Any]:
    defaults = _runtime_registry_payload(repo_root).get("workspace_bootstrap_defaults")
    if not isinstance(defaults, dict):
        raise ValueError("Runtime registry workspace_bootstrap_defaults must be an object.")
    return defaults


def ensure_default_bootstrap_bundle(
    repo_root: Path,
    *,
    output_dir: Path | None = None,
) -> dict[str, Any]:
    """Ensure the canonical default bootstrap bundle exists for this repository."""

    resolved_repo_root = repo_root.resolve()
    resolved_output_dir = (output_dir or bootstrap_artifact_root(resolved_repo_root)).resolve()
    resolved_output_dir.mkdir(parents=True, exist_ok=True)
    mirror_bootstrap_path = resolve_bootstrap_path(resolved_output_dir)
    had_existing_file = mirror_bootstrap_path.exists()
    if _bootstrap_file_matches_contract(mirror_bootstrap_path, resolved_repo_root):
        return {
            "success": True,
            "changed": False,
            "status": "already-present",
            "output_dir": str(resolved_output_dir),
            "bootstrap_path": str(mirror_bootstrap_path),
            "mirror_bootstrap_path": str(mirror_bootstrap_path),
        }

    result = run_default_bootstrap(repo_root=resolved_repo_root, output_dir=resolved_output_dir)
    return {
        "success": True,
        "changed": True,
        "status": "repaired-stale" if had_existing_file else "materialized",
        "output_dir": result["paths"]["output_dir"],
        "task_output_dir": result["paths"]["task_output_dir"],
        "bootstrap_path": result["bootstrap_path"],
        "mirror_bootstrap_path": result["paths"]["mirror_bootstrap_path"],
        "task_id": result["payload"]["bootstrap"]["task_id"],
        "memory_items": result["memory_items"],
        "proposal_count": result["proposal_count"],
    }

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
    updated = _upsert_named_toml_block(
        content,
        block=build_framework_server_block(repo_root),
        pattern=FRAMEWORK_SERVER_PATTERN,
    )
    return write_text_if_changed(config_path, updated)


def build_openai_developer_docs_server_block() -> str:
    """Build the OpenAI Developer Docs MCP config block."""

    return "\n".join(
        [
            "[mcp_servers.openaiDeveloperDocs]",
            f'url = "{OPENAI_DEVELOPER_DOCS_MCP_URL}"',
        ]
    )


def build_codex_hooks_feature_block() -> str:
    """Build the Codex experimental hooks feature block."""

    return "\n".join(
        [
            "[features]",
            "codex_hooks = true",
        ]
    )


def install_codex_hooks_feature(config_path: Path) -> bool:
    """Ensure Codex experimental hooks are enabled in a config file."""

    content = config_path.read_text(encoding="utf-8") if config_path.exists() else ""
    block = build_codex_hooks_feature_block()
    match = FEATURES_BLOCK_PATTERN.search(content)
    if not match:
        updated = content.rstrip()
        if updated:
            updated += "\n\n"
        updated += block + "\n"
        return write_text_if_changed(config_path, updated)

    lines = match.group(0).rstrip("\n").splitlines()
    saw_key = False
    needs_change = False
    for line in lines:
        if not re.match(r"^\s*codex_hooks\s*=", line):
            continue
        saw_key = True
        if line.strip() != "codex_hooks = true":
            needs_change = True
    if not saw_key:
        needs_change = True
    if not needs_change:
        return False

    replaced = False
    updated_lines: list[str] = []
    for line in lines:
        if re.match(r"^\s*codex_hooks\s*=", line):
            updated_lines.append("codex_hooks = true")
            replaced = True
            continue
        updated_lines.append(line)
    if not replaced:
        updated_lines.append("codex_hooks = true")
    new_block = "\n".join(updated_lines) + "\n"
    updated = content[: match.start()] + new_block + content[match.end() :]
    return write_text_if_changed(config_path, updated)


def install_openai_developer_docs_server(config_path: Path) -> bool:
    """Install the OpenAI Developer Docs MCP entry into a Codex config file."""

    content = config_path.read_text(encoding="utf-8") if config_path.exists() else ""
    updated = _upsert_named_toml_block(
        content,
        block=build_openai_developer_docs_server_block(),
        pattern=OPENAI_DEVELOPER_DOCS_SERVER_PATTERN,
    )
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


def skill_bridge_source_path(repo_root: Path) -> Path:
    """Return the canonical shared skill source for this repository."""

    skill_bridge = _workspace_bootstrap_defaults(repo_root).get("skill_bridge", {})
    source_rel = str(skill_bridge.get("source_rel", "skills"))
    return (repo_root / source_rel).resolve()


def sync_directory(
    source: Path,
    destination: Path,
    *,
    skip_names: set[str] | frozenset[str] | None = None,
) -> bool:
    """Mirror one directory tree into another."""

    if not source.is_dir():
        raise FileNotFoundError(f"Plugin source directory not found: {source}")

    changed = False
    skipped = skip_names or set()
    destination.mkdir(parents=True, exist_ok=True)

    source_children = {item.name: item for item in source.iterdir()}
    destination_children = {item.name: item for item in destination.iterdir()}

    for stale_name, stale_path in destination_children.items():
        if stale_name in skipped:
            continue
        if stale_name in source_children:
            continue
        changed = True
        if stale_path.is_dir():
            shutil.rmtree(stale_path)
        else:
            stale_path.unlink()

    for name, source_path in source_children.items():
        if name in skipped:
            continue
        destination_path = destination / name
        if source_path.is_dir():
            changed = sync_directory(source_path, destination_path, skip_names=skip_names) or changed
            continue
        source_bytes = source_path.read_bytes()
        destination_bytes = destination_path.read_bytes() if destination_path.is_file() else None
        if destination_bytes == source_bytes:
            continue
        destination_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source_path, destination_path)
        changed = True

    return changed


def _ensure_directory_symlink(
    source: Path,
    *,
    target_path: Path,
) -> bool:
    """Ensure one directory path is a symlink to the shared source tree."""

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


def _ensure_home_skills_link(
    repo_root: Path,
    *,
    target_path: Path,
) -> bool:
    """Ensure one host skill directory points at the repository skill library."""

    return _ensure_directory_symlink(skill_bridge_source_path(repo_root), target_path=target_path)


def ensure_home_codex_skills_link(
    repo_root: Path,
    *,
    target_path: Path = HOME_CODEX_SKILLS_PATH,
) -> bool:
    """Ensure ~/.codex/skills points at the repository skill library."""

    return _ensure_home_skills_link(repo_root, target_path=target_path)


def ensure_home_claude_skills_link(
    repo_root: Path,
    *,
    target_path: Path = HOME_CLAUDE_SKILLS_PATH,
) -> bool:
    """Ensure ~/.claude/skills points at the repository skill library."""

    return _ensure_home_skills_link(repo_root, target_path=target_path)


def ensure_home_claude_refresh_command(
    command_path: Path = HOME_CLAUDE_REFRESH_PATH,
) -> bool:
    """Ensure the global Claude refresh command matches the repo canonical text."""

    return write_text_if_changed(command_path, CLAUDE_REFRESH_COMMAND)


def build_personal_plugin_mcp_payload(repo_root: Path) -> dict[str, Any]:
    """Build the home-plugin MCP payload with stable absolute repo pointers."""

    resolved_repo_root = repo_root.resolve()
    browser_script = (resolved_repo_root / "tools" / "browser-mcp" / "scripts" / "start_browser_mcp.sh").resolve()
    return {
        "mcpServers": {
            "framework-mcp": {
                "command": "python3",
                "args": ["-m", "scripts.framework_mcp"],
                "cwd": str(resolved_repo_root),
            },
            "browser-mcp": {
                "command": "bash",
                "args": [str(browser_script)],
                "cwd": str(resolved_repo_root),
            },
            "openaiDeveloperDocs": {
                "type": "http",
                "url": OPENAI_DEVELOPER_DOCS_MCP_URL,
            },
        }
    }


def ensure_personal_plugin_live_projection(
    repo_root: Path,
    plugin_root: Path = HOME_PLUGIN_ROOT,
) -> bool:
    """Install the home plugin as a thin live projection onto repo-owned skills/runtime."""

    source_rel = str(_primary_plugin_record(repo_root).get("source_rel", f"plugins/{PLUGIN_NAME}"))
    source = repo_root / source_rel
    changed = sync_directory(source, plugin_root, skip_names=PERSONAL_PLUGIN_LIVE_PROJECTION_EXCLUDES)
    changed = _ensure_directory_symlink(skill_bridge_source_path(repo_root), target_path=plugin_root / "skills") or changed
    changed = write_json_if_changed(plugin_root / ".mcp.json", build_personal_plugin_mcp_payload(repo_root)) or changed
    return changed


def build_personal_marketplace_payload(
    plugin_root: Path = HOME_PLUGIN_ROOT,
    *,
    marketplace_root: Path | None = None,
    existing_marketplace: dict[str, Any] | None = None,
    plugin_name: str | None = None,
    plugin_category: str | None = None,
) -> dict[str, Any]:
    """Build a personal marketplace payload that exposes the framework plugin."""

    resolved_plugin_name = plugin_name or PLUGIN_NAME
    resolved_category = plugin_category or DEFAULT_PLUGIN_CATEGORY
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
        if row.get("name") != resolved_plugin_name:
            updated_plugins.append(row)
            continue
        replaced = True
        updated_plugins.append(
            {
                "name": resolved_plugin_name,
                "source": {"source": "local", "path": plugin_path},
                "policy": {"installation": "AVAILABLE", "authentication": "ON_INSTALL"},
                "category": row.get("category", resolved_category),
            }
        )

    if not replaced:
        updated_plugins.append(
            {
                "name": resolved_plugin_name,
                "source": {"source": "local", "path": plugin_path},
                "policy": {"installation": "AVAILABLE", "authentication": "ON_INSTALL"},
                "category": resolved_category,
            }
        )

    payload["plugins"] = updated_plugins
    return payload


def install_personal_marketplace(
    marketplace_path: Path = HOME_MARKETPLACE_PATH,
    *,
    plugin_root: Path = HOME_PLUGIN_ROOT,
    repo_root: Path | None = None,
) -> bool:
    """Ensure the user's personal plugin marketplace exposes the framework plugin."""

    existing = read_json_if_exists(marketplace_path)
    plugin_record = _primary_plugin_record(repo_root)
    payload = build_personal_marketplace_payload(
        plugin_root,
        marketplace_root=marketplace_path.resolve().parents[2],
        existing_marketplace=existing,
        plugin_name=str(plugin_record.get("marketplace_name", plugin_record.get("plugin_name", PLUGIN_NAME))),
        plugin_category=str(plugin_record.get("marketplace_category", DEFAULT_PLUGIN_CATEGORY)),
    )
    return write_json_if_changed(marketplace_path, payload)


def install_native_integration(
    *,
    home_config_path: Path = HOME_CONFIG_PATH,
    repo_root: Path | None = None,
    home_plugin_root: Path = HOME_PLUGIN_ROOT,
    home_marketplace_path: Path = HOME_MARKETPLACE_PATH,
    home_codex_skills_path: Path = HOME_CODEX_SKILLS_PATH,
    home_claude_skills_path: Path = HOME_CLAUDE_SKILLS_PATH,
    home_claude_refresh_path: Path = HOME_CLAUDE_REFRESH_PATH,
    home_claude_mcp_config_path: Path = HOME_CLAUDE_MCP_CONFIG_PATH,
    project_instructions_path: Path = PROJECT_INSTRUCTIONS_PATH,
    install_browser_mcp: bool = True,
    install_framework_mcp: bool = True,
    install_openai_developer_docs_mcp: bool = True,
    retire_framework_overlay_file: bool = True,
    install_personal_plugin: bool = True,
    install_personal_marketplace_entry: bool = True,
    install_home_codex_skills_link: bool = True,
    install_home_claude_skills_link: bool = False,
    install_home_claude_refresh_command: bool = False,
    install_home_claude_mcp_sync: bool = True,
    install_default_bootstrap: bool = True,
    bootstrap_output_dir: Path | None = None,
) -> dict[str, Any]:
    """Rust-owned installer wrapper kept behind the legacy Python API."""

    resolved_repo_root = (repo_root or get_repo_root()).resolve()
    with TemporaryDirectory() as temp_dir:
        template_root = Path(temp_dir)
        write_host_entrypoint_template(template_root)
        command = [
            "install-native-integration",
            "--template-root",
            str(template_root),
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
            "--home-claude-skills-path",
            str(home_claude_skills_path),
            "--home-claude-refresh-path",
            str(home_claude_refresh_path),
            "--home-claude-mcp-config-path",
            str(home_claude_mcp_config_path),
            "--project-instructions-path",
            str(project_instructions_path),
        ]
        if bootstrap_output_dir is not None:
            command.extend(["--bootstrap-output-dir", str(bootstrap_output_dir)])
        if not install_browser_mcp:
            command.append("--skip-browser-mcp")
        if not install_framework_mcp:
            command.append("--skip-framework-mcp")
        if not install_openai_developer_docs_mcp:
            command.append("--skip-openai-developer-docs-mcp")
        if not retire_framework_overlay_file:
            command.append("--skip-framework-overlay-retirement")
        if not install_personal_plugin:
            command.append("--skip-personal-plugin")
        if not install_personal_marketplace_entry:
            command.append("--skip-personal-marketplace")
        if not install_home_codex_skills_link:
            command.append("--skip-home-codex-skills-link")
        if not install_home_claude_skills_link:
            command.append("--skip-home-claude-skills-link")
        if not install_home_claude_refresh_command:
            command.append("--skip-home-claude-refresh")
        if not install_home_claude_mcp_sync:
            command.append("--skip-home-claude-mcp-sync")
        if not install_default_bootstrap:
            command.append("--skip-default-bootstrap")
        return run_host_integration_rs(*command)


def main() -> int:
    parser = argparse.ArgumentParser(description="Install the repo's native Codex integration.")
    parser.add_argument("--home-config-path", type=Path, default=HOME_CONFIG_PATH)
    parser.add_argument("--home-plugin-root", type=Path, default=HOME_PLUGIN_ROOT)
    parser.add_argument("--home-marketplace-path", type=Path, default=HOME_MARKETPLACE_PATH)
    parser.add_argument("--home-codex-skills-path", type=Path, default=HOME_CODEX_SKILLS_PATH)
    parser.add_argument("--home-claude-skills-path", type=Path, default=HOME_CLAUDE_SKILLS_PATH)
    parser.add_argument("--home-claude-refresh-path", type=Path, default=HOME_CLAUDE_REFRESH_PATH)
    parser.add_argument("--home-claude-mcp-config-path", type=Path, default=HOME_CLAUDE_MCP_CONFIG_PATH)
    parser.add_argument("--project-instructions-path", type=Path, default=PROJECT_INSTRUCTIONS_PATH)
    parser.add_argument("--bootstrap-output-dir", type=Path, default=None)
    parser.add_argument("--repo-root", type=Path, default=None)
    parser.add_argument("--skip-browser-mcp", action="store_true")
    parser.add_argument("--skip-framework-mcp", action="store_true")
    parser.add_argument("--skip-openai-developer-docs-mcp", action="store_true")
    parser.add_argument("--skip-framework-overlay-retirement", action="store_true")
    parser.add_argument("--skip-personal-plugin", action="store_true")
    parser.add_argument("--skip-personal-marketplace", action="store_true")
    parser.add_argument("--skip-home-codex-skills-link", action="store_true")
    parser.add_argument("--home-claude-skills", action="store_true")
    parser.add_argument("--home-claude-refresh", action="store_true")
    parser.add_argument("--skip-home-claude-mcp-sync", action="store_true")
    parser.add_argument("--skip-default-bootstrap", action="store_true")
    parser.add_argument("--json", action="store_true", dest="json_output")
    args = parser.parse_args()
    payload = install_native_integration(
        home_config_path=args.home_config_path,
        home_plugin_root=args.home_plugin_root,
        home_marketplace_path=args.home_marketplace_path,
        home_codex_skills_path=args.home_codex_skills_path,
        home_claude_skills_path=args.home_claude_skills_path,
        home_claude_refresh_path=args.home_claude_refresh_path,
        home_claude_mcp_config_path=args.home_claude_mcp_config_path,
        repo_root=args.repo_root,
        project_instructions_path=args.project_instructions_path,
        install_browser_mcp=not args.skip_browser_mcp,
        install_framework_mcp=not args.skip_framework_mcp,
        install_openai_developer_docs_mcp=not args.skip_openai_developer_docs_mcp,
        retire_framework_overlay_file=not args.skip_framework_overlay_retirement,
        install_personal_plugin=not args.skip_personal_plugin,
        install_personal_marketplace_entry=not args.skip_personal_marketplace,
        install_home_codex_skills_link=not args.skip_home_codex_skills_link,
        install_home_claude_skills_link=args.home_claude_skills,
        install_home_claude_refresh_command=args.home_claude_refresh,
        install_home_claude_mcp_sync=not args.skip_home_claude_mcp_sync,
        install_default_bootstrap=not args.skip_default_bootstrap,
        bootstrap_output_dir=args.bootstrap_output_dir,
    )
    if args.json_output:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    else:
        print(payload)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
