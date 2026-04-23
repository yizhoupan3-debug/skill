from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.host_integration_rs import export_runtime_registry
from scripts.claude_statusline import render_statusline
from scripts.materialize_cli_host_entrypoints import (
    CODEX_PROJECT_HOOKS,
    CLAUDE_AUTOPILOT_COMMAND,
    CLAUDE_BACKGROUND_BATCH_COMMAND,
    CLAUDE_DEEPINTERVIEW_COMMAND,
    CLAUDE_LATEX_COMPILE_ACCELERATION_COMMAND,
    CLAUDE_PROJECT_DIR_SNIPPET,
    CLAUDE_REFRESH_COMMAND,
    CLAUDE_ROUTER_RS_MANIFEST_PATH,
    CLAUDE_ROUTER_RS_DEBUG_BINARY,
    CLAUDE_ROUTER_RS_RELEASE_BINARY,
    CLAUDE_TEAM_COMMAND,
    materialize_repo_host_entrypoints,
    sync_repo_host_entrypoints,
)
from scripts.rust_binary_runner import ensure_rust_binary
from scripts.sync_skills import write_generated_files


def _framework_native_aliases() -> dict[str, object]:
    payload = export_runtime_registry(PROJECT_ROOT)
    aliases = payload.get("framework_native_aliases")
    assert isinstance(aliases, dict)
    return aliases


def _write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def _write_json(path: Path, payload: dict[str, object]) -> None:
    _write_text(path, json.dumps(payload, ensure_ascii=False, indent=2) + "\n")


def _seed_runtime_artifacts(repo_root: Path) -> None:
    task_id = "validate-claude-hooks-20260423010101"
    _write_text(
        repo_root / "artifacts" / "current" / task_id / "SESSION_SUMMARY.md",
        "\n".join(
            [
                "- task: Validate Claude hooks",
                "- phase: integration",
                "- status: in_progress",
            ]
        )
        + "\n",
    )
    _write_json(
        repo_root / "artifacts" / "current" / task_id / "NEXT_ACTIONS.json",
        {"next_actions": ["Wire hooks"]},
    )
    _write_json(repo_root / "artifacts" / "current" / task_id / "EVIDENCE_INDEX.json", {"artifacts": []})
    _write_json(
        repo_root / "artifacts" / "current" / task_id / "TRACE_METADATA.json",
        {"matched_skills": ["checklist-fixer"]},
    )
    _write_text(
        repo_root / "artifacts" / "current" / "SESSION_SUMMARY.md",
        "\n".join(
            [
                "- task: Validate Claude hooks",
                "- phase: integration",
                "- status: in_progress",
            ]
        )
        + "\n",
    )
    _write_json(repo_root / "artifacts" / "current" / "NEXT_ACTIONS.json", {"next_actions": ["Wire hooks"]})
    _write_json(repo_root / "artifacts" / "current" / "EVIDENCE_INDEX.json", {"artifacts": []})
    _write_json(repo_root / "artifacts" / "current" / "TRACE_METADATA.json", {"matched_skills": ["checklist-fixer"]})
    _write_json(
        repo_root / "artifacts" / "current" / "active_task.json",
        {"task_id": task_id, "task": "Validate Claude hooks"},
    )
    _write_json(
        repo_root / "artifacts" / "current" / "focus_task.json",
        {"task_id": task_id, "task": "Validate Claude hooks"},
    )
    _write_json(
        repo_root / "artifacts" / "current" / "task_registry.json",
        {
            "schema_version": "task-registry-v1",
            "focus_task_id": task_id,
            "tasks": [
                {
                    "task_id": task_id,
                    "task": "Validate Claude hooks",
                    "phase": "integration",
                    "status": "in_progress",
                    "resume_allowed": True,
                }
            ],
        },
    )
    _write_json(
        repo_root / ".supervisor_state.json",
        {
            "task_id": task_id,
            "task_summary": "Validate Claude hooks",
            "active_phase": "integration",
            "verification": {"verification_status": "in_progress"},
            "continuity": {"story_state": "active", "resume_allowed": True},
            "execution_contract": {
                "scope": ["Claude hooks"],
                "acceptance_criteria": ["Hooks refresh projection"],
            },
        },
    )


def _seed_shared_memory(repo_root: Path) -> None:
    _write_text(
        repo_root / ".codex" / "memory" / "MEMORY.md",
        "\n".join(
            [
                "# 项目长期记忆",
                "",
                "## Active Patterns",
                "",
                "- AP-1: Externalize complex task state into artifacts",
                "",
                "## 稳定决策",
                "",
                "- SD-1: Generated host files should not drift from source",
                "",
            ]
        )
        + "\n",
    )


def _init_git_repo(repo_root: Path) -> None:
    subprocess.run(["git", "init"], cwd=repo_root, check=True)
    subprocess.run(["git", "config", "user.name", "Codex Test"], cwd=repo_root, check=True)
    subprocess.run(["git", "config", "user.email", "codex@example.com"], cwd=repo_root, check=True)
    (repo_root / "README.md").write_text("seed\n", encoding="utf-8")
    subprocess.run(["git", "add", "README.md"], cwd=repo_root, check=True)
    subprocess.run(["git", "commit", "-m", "init"], cwd=repo_root, check=True)


def _ensure_router_rs_binaries() -> None:
    crate_root = PROJECT_ROOT / "scripts" / "router-rs"
    ensure_rust_binary(
        crate_root=crate_root,
        binary_name="router-rs",
        release=False,
        allow_stale_fallback=False,
        allow_cross_profile_fallback=False,
        cwd=PROJECT_ROOT,
    )
    ensure_rust_binary(
        crate_root=crate_root,
        binary_name="router-rs",
        release=True,
        allow_stale_fallback=False,
        allow_cross_profile_fallback=False,
        cwd=PROJECT_ROOT,
    )


def _run_router_rs_hook_manifest() -> dict[str, object]:
    crate_root = PROJECT_ROOT / "scripts" / "router-rs"
    binary_path = ensure_rust_binary(
        crate_root=crate_root,
        binary_name="router-rs",
        release=True,
        allow_stale_fallback=False,
        allow_cross_profile_fallback=False,
        cwd=PROJECT_ROOT,
    )
    completed = subprocess.run(
        [str(binary_path), "--claude-hook-manifest-json"],
        cwd=PROJECT_ROOT,
        check=True,
        text=True,
        capture_output=True,
    )
    return json.loads(completed.stdout)


def _run_router_rs_claude_project_settings(repo_root: Path) -> dict[str, object]:
    crate_root = PROJECT_ROOT / "scripts" / "router-rs"
    binary_path = ensure_rust_binary(
        crate_root=crate_root,
        binary_name="router-rs",
        release=True,
        allow_stale_fallback=False,
        allow_cross_profile_fallback=False,
        cwd=PROJECT_ROOT,
    )
    completed = subprocess.run(
        [
            str(binary_path),
            "--claude-project-settings-json",
            "--repo-root",
            str(repo_root),
        ],
        cwd=PROJECT_ROOT,
        check=True,
        text=True,
        capture_output=True,
    )
    return json.loads(completed.stdout)


def _run_router_rs_claude_audit(
    command: str,
    repo_root: Path,
    payload: dict[str, object],
) -> subprocess.CompletedProcess[str]:
    _ensure_router_rs_binaries()
    debug_bin = PROJECT_ROOT / "scripts" / "router-rs" / "target" / "debug" / "router-rs"
    return subprocess.run(
        [str(debug_bin), "--claude-hook-audit-command", command, "--repo-root", str(repo_root)],
        input=json.dumps(payload, ensure_ascii=False),
        text=True,
        capture_output=True,
        check=False,
    )


def _relay_router_rs_claude_audit(command: str, repo_root: Path, payload: dict[str, object]) -> int:
    result = _run_router_rs_claude_audit(command, repo_root, payload)
    if result.stdout:
        sys.stdout.write(result.stdout)
    if result.stderr:
        sys.stderr.write(result.stderr)
    return result.returncode


def run_config_change(repo_root: Path, payload: dict[str, object]) -> int:
    return _relay_router_rs_claude_audit("config-change", repo_root, payload)


def run_pre_tool_use(repo_root: Path, payload: dict[str, object]) -> int:
    return _relay_router_rs_claude_audit("pre-tool-use", repo_root, payload)


def run_stop_failure(repo_root: Path, payload: dict[str, object]) -> int:
    return _relay_router_rs_claude_audit("stop-failure", repo_root, payload)


def test_router_rs_exports_claude_hook_manifest() -> None:
    manifest = _run_router_rs_hook_manifest()

    assert manifest["schema_version"] == "router-rs-claude-hook-manifest-v1"
    assert "/.claude/settings.json" in manifest["protected_paths"]["edit_write"]
    assert "*.claude/settings.json*" in manifest["protected_paths"]["bash"]
    assert "/scripts/router-rs/src/**" in manifest["protected_paths"]["quality"]
    assert "/scripts/materialize_cli_host_entrypoints.py" in manifest["protected_paths"]["quality"]
    assert "/tests/**" in manifest["protected_paths"]["quality"]
    assert "/.claude/hooks/**" in manifest["protected_paths"]["quality"]
    assert set(manifest["settings_hooks"]) == {
        "PreToolUse",
        "PostToolUse",
        "SessionEnd",
        "ConfigChange",
        "StopFailure",
        "UserPromptSubmit",
    }


def test_materialized_claude_settings_hooks_match_rust_manifest(tmp_path: Path) -> None:
    manifest = _run_router_rs_hook_manifest()
    expected_settings = _run_router_rs_claude_project_settings(tmp_path)
    materialize_repo_host_entrypoints(tmp_path)

    settings = json.loads((tmp_path / ".claude" / "settings.json").read_text(encoding="utf-8"))

    assert settings == expected_settings
    assert settings["hooks"] == manifest["settings_hooks"]


def test_router_rs_hook_manifest_resolution_stays_release_strict(
    tmp_path: Path, monkeypatch
) -> None:
    captured: dict[str, object] = {}

    def fake_ensure_rust_binary(**kwargs):
        captured.update(kwargs)
        return tmp_path / "router-rs-release"

    monkeypatch.setattr(sys.modules[__name__], "PROJECT_ROOT", tmp_path)
    monkeypatch.setattr(sys.modules[__name__], "ensure_rust_binary", fake_ensure_rust_binary)

    def fake_run(cmd, **kwargs):
        assert cmd == [str(tmp_path / "router-rs-release"), "--claude-hook-manifest-json"]
        return subprocess.CompletedProcess(
            cmd,
            0,
            stdout='{"schema_version":"router-rs-claude-hook-manifest-v1"}',
            stderr="",
        )

    monkeypatch.setattr(subprocess, "run", fake_run)

    manifest = _run_router_rs_hook_manifest()

    assert manifest["schema_version"] == "router-rs-claude-hook-manifest-v1"
    assert captured["crate_root"] == tmp_path / "scripts" / "router-rs"
    assert captured["binary_name"] == "router-rs"
    assert captured["release"] is True
    assert captured["allow_stale_fallback"] is False
    assert captured["allow_cross_profile_fallback"] is False
    assert captured["cwd"] == tmp_path


def test_materialize_repo_host_entrypoints_creates_shared_policy_and_host_proxies(
    tmp_path: Path,
) -> None:
    aliases = _framework_native_aliases()
    result = materialize_repo_host_entrypoints(tmp_path)

    assert "AGENT.md" in result["written"]
    assert "AGENTS.md" in result["written"]
    assert "CLAUDE.md" in result["written"]
    assert "GEMINI.md" in result["written"]
    assert ".claude/settings.json" in result["written"]
    assert ".gemini/settings.json" in result["written"]

    assert (tmp_path / "AGENT.md").is_file()
    agent_policy = (tmp_path / "AGENT.md").read_text(encoding="utf-8")
    assert "Shared Agent Policy" in agent_policy
    assert "RTK.md" in agent_policy
    assert "## Repo Landmarks" in agent_policy
    assert "## Communication Style" in agent_policy
    assert "## Verification Defaults" in agent_policy
    assert "## Task Closeout" in agent_policy
    assert "changed-file inventories" in agent_policy
    assert "evidence lists" in agent_policy
    assert "what was done, what effect was achieved" in agent_policy
    assert "plain and natural" in agent_policy
    assert "task artifact, audit log, or status machine" in agent_policy
    assert "Explain things in plain language first" in agent_policy
    assert "Avoid internal runtime, routing, framework, or tool jargon" in agent_policy
    assert "Do not force personality" in agent_policy
    assert "user-visible effect over implementation narration" in agent_policy
    assert "Do not silently choose an ambiguous interpretation" in agent_policy
    assert "Prefer the smallest solution that fully solves the stated problem" in agent_policy
    assert "## Simplify First" in agent_policy
    assert "Prefer simplification before expansion" in agent_policy
    assert "If two approaches both solve the task" in agent_policy
    assert "Prefer removing obsolete compatibility" in agent_policy
    assert "For non-trivial execution, state the minimum success criteria" in agent_policy
    assert "Keep this file compact and factual" in agent_policy
    assert "configs/framework/FRAMEWORK_SURFACE_POLICY.json" in agent_policy
    assert "AGENT.md" in (tmp_path / "AGENTS.md").read_text(encoding="utf-8")
    assert not (tmp_path / ".claude" / "CLAUDE.md").exists()
    assert not (tmp_path / ".codex" / "model_instructions.md").exists()
    assert not (tmp_path / ".mcp.json").exists()
    claude_entry = (tmp_path / "CLAUDE.md").read_text(encoding="utf-8")
    assert "@.codex/memory/CLAUDE_MEMORY.md" not in claude_entry
    assert "Keep startup lean." in claude_entry
    assert "host-shell glue" in claude_entry
    assert "manual resume" in claude_entry
    assert "Generated-first maintenance rule" in claude_entry
    assert "AGENT.md" in (tmp_path / "GEMINI.md").read_text(encoding="utf-8")
    settings = json.loads((tmp_path / ".claude" / "settings.json").read_text(encoding="utf-8"))
    expected_settings = _run_router_rs_claude_project_settings(tmp_path)
    codex_hooks = json.loads((tmp_path / ".codex" / "hooks.json").read_text(encoding="utf-8"))
    assert settings == expected_settings
    assert settings["$schema"] == "https://json.schemastore.org/claude-code-settings.json"
    assert settings["allowedMcpServers"] == [
        {"serverName": "browser-mcp"},
        {"serverName": "framework-mcp"},
        {"serverName": "openaiDeveloperDocs"},
    ]
    assert "statusLine" not in settings
    assert codex_hooks == CODEX_PROJECT_HOOKS
    assert codex_hooks["hooks"] == {}
    assert set(settings["hooks"]) == {
        "PreToolUse",
        "PostToolUse",
        "SessionEnd",
        "ConfigChange",
        "StopFailure",
        "UserPromptSubmit",
    }
    assert (
        settings["hooks"]["UserPromptSubmit"][0]["hooks"][0]["command"]
        == 'sh "$CLAUDE_PROJECT_DIR"/.claude/hooks/run.sh user-prompt-submit'
    )
    assert settings["hooks"]["PreToolUse"][0]["matcher"] == "Edit|MultiEdit|Write"
    assert settings["hooks"]["PreToolUse"][1]["matcher"] == "Bash"
    assert settings["hooks"]["PreToolUse"][2]["matcher"] == "Edit|MultiEdit|Write"
    pre_tool_hooks = settings["hooks"]["PreToolUse"][0]["hooks"]
    assert any(item["if"] == "Edit(/.claude/settings.json)" for item in pre_tool_hooks)
    assert any(item["if"] == "Edit(/.claude/agents/README.md)" for item in pre_tool_hooks)
    assert any(item["if"] == "Edit(/.claude/hooks/README.md)" for item in pre_tool_hooks)
    assert any(item["if"] == "Edit(/.claude/commands/**)" for item in pre_tool_hooks)
    assert any(item["if"] == "Write(/.codex/memory/CLAUDE_MEMORY.md)" for item in pre_tool_hooks)
    assert not any(item["if"] == "Edit(/.claude/**)" for item in pre_tool_hooks)
    assert not any("settings.local.json" in item["if"] for item in pre_tool_hooks)
    bash_hooks = settings["hooks"]["PreToolUse"][1]["hooks"]
    assert any(item["if"] == "Bash(*.claude/settings.json*)" for item in bash_hooks)
    assert any(item["if"] == "Bash(*.claude/agents/README.md*)" for item in bash_hooks)
    assert any(item["if"] == "Bash(*.claude/commands/*)" for item in bash_hooks)
    assert not any(item["if"] == "Bash(*.claude/*)" for item in bash_hooks)
    quality_hooks = settings["hooks"]["PreToolUse"][2]["hooks"]
    assert any(item["if"] == "Edit(/scripts/router-rs/src/**)" for item in quality_hooks)
    assert any(item["if"] == "Write(/framework_runtime/src/**)" for item in quality_hooks)
    assert any(item["if"] == "Edit(/scripts/materialize_cli_host_entrypoints.py)" for item in quality_hooks)
    assert any(item["if"] == "Edit(/tests/**)" for item in quality_hooks)
    assert any(item["if"] == "Edit(/.claude/hooks/**)" for item in quality_hooks)
    assert not any(item["if"] == "Edit(/scripts/**)" for item in quality_hooks)
    post_tool_hooks = settings["hooks"]["PostToolUse"][0]["hooks"]
    assert settings["hooks"]["PostToolUse"][0]["matcher"] == "Edit|MultiEdit|Write"
    assert any(item["if"] == "Edit(/scripts/router-rs/src/**)" for item in post_tool_hooks)
    assert any(item["if"] == "Write(/framework_runtime/src/**)" for item in post_tool_hooks)
    assert any(item["if"] == "Edit(/scripts/materialize_cli_host_entrypoints.py)" for item in post_tool_hooks)
    assert any(item["if"] == "Edit(/tests/**)" for item in post_tool_hooks)
    assert any(item["if"] == "Edit(/.claude/hooks/**)" for item in post_tool_hooks)
    assert not any(item["if"] == "Edit(/scripts/**)" for item in post_tool_hooks)
    assert all(item["command"].endswith('/.claude/hooks/run.sh post-tool-audit') for item in post_tool_hooks)
    assert all(item["async"] is True for item in post_tool_hooks)
    assert all(item["timeout"] == 8 for item in post_tool_hooks)
    assert settings["hooks"]["ConfigChange"][0]["matcher"] == "project_settings"
    assert settings["hooks"]["StopFailure"][0]["matcher"] == (
        "invalid_request|server_error|max_output_tokens|rate_limit|authentication_failed|billing_error|unknown"
    )
    assert json.loads((tmp_path / ".gemini" / "settings.json").read_text(encoding="utf-8")) == {}
    assert (tmp_path / ".claude" / "agents" / "README.md").is_file()
    assert (tmp_path / ".claude" / "commands" / "refresh.md").is_file()
    assert (tmp_path / ".claude" / "commands" / "background_batch.md").is_file()
    assert (tmp_path / ".claude" / "commands" / "autopilot.md").is_file()
    assert (tmp_path / ".claude" / "commands" / "deepinterview.md").is_file()
    assert (tmp_path / ".claude" / "commands" / "team.md").is_file()
    assert (tmp_path / ".claude" / "commands" / "latex-compile-acceleration.md").is_file()
    assert (tmp_path / ".claude" / "hooks" / "run.sh").is_file()
    hook_runner_script = (tmp_path / ".claude" / "hooks" / "run.sh").read_text(encoding="utf-8")
    assert "run_router_rs" in hook_runner_script
    assert "python3 \"$PROJECT_DIR/scripts/claude_hook" not in hook_runner_script
    assert "cargo build --manifest-path \"$ROUTER_RS_CRATE_ROOT/Cargo.toml\"" not in hook_runner_script
    assert "case \"$command_name\" in" in hook_runner_script
    assert "--claude-hook-audit-command pre-tool-use" in hook_runner_script
    assert "--claude-hook-audit-command \"$command_name\"" in hook_runner_script
    assert "--claude-hook-command session-end" in hook_runner_script
    assert "shared hook returned no hookSpecificOutput; continuing with degraded context" in hook_runner_script
    assert 'config-change|stop-failure)\n    run_router_rs --claude-hook-audit-command "$command_name" --repo-root "$PROJECT_DIR" >/dev/null' in hook_runner_script
    assert not (tmp_path / ".claude" / "commands" / "deepreview.md").exists()
    refresh_command = (tmp_path / ".claude" / "commands" / "refresh.md").read_text(encoding="utf-8")
    background_batch_command = (
        tmp_path / ".claude" / "commands" / "background_batch.md"
    ).read_text(encoding="utf-8")
    autopilot_command = (
        tmp_path / ".claude" / "commands" / "autopilot.md"
    ).read_text(encoding="utf-8")
    deepinterview_command = (
        tmp_path / ".claude" / "commands" / "deepinterview.md"
    ).read_text(encoding="utf-8")
    team_command = (
        tmp_path / ".claude" / "commands" / "team.md"
    ).read_text(encoding="utf-8")
    latex_compile_acceleration_command = (
        tmp_path / ".claude" / "commands" / "latex-compile-acceleration.md"
    ).read_text(encoding="utf-8")
    assert refresh_command == CLAUDE_REFRESH_COMMAND
    assert background_batch_command == CLAUDE_BACKGROUND_BATCH_COMMAND
    assert autopilot_command == CLAUDE_AUTOPILOT_COMMAND
    assert deepinterview_command == CLAUDE_DEEPINTERVIEW_COMMAND
    assert team_command == CLAUDE_TEAM_COMMAND
    assert latex_compile_acceleration_command == CLAUDE_LATEX_COMPILE_ACCELERATION_COMMAND
    assert CLAUDE_PROJECT_DIR_SNIPPET in refresh_command
    assert "使用 Rust refresh 命令继续当前活跃任务，并复制下一轮执行提示。" in refresh_command
    assert "唯一显式的 continue / next 入口" in refresh_command
    assert "读取现有 continuity 真源" in refresh_command
    assert (
        '"$PROJECT_DIR"/scripts/router-rs/target/release/router-rs --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"'
        in refresh_command
    )
    assert (
        '"$PROJECT_DIR"/scripts/router-rs/target/debug/router-rs --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"'
        in refresh_command
    )
    assert (
        'cargo run --manifest-path "$PROJECT_DIR"/scripts/router-rs/Cargo.toml --release -- --framework-refresh-json --claude-hook-max-lines 4 --repo-root "$PROJECT_DIR"'
        in refresh_command
    )
    assert "然后严格回复" in refresh_command
    assert "下一轮执行提示已准备好，并且已经复制到剪贴板。" in refresh_command
    assert "summary" not in refresh_command.lower()
    assert "clear" not in refresh_command.lower()
    assert "- Bash(git rev-parse *)" in refresh_command
    assert f"- Bash({CLAUDE_ROUTER_RS_RELEASE_BINARY} *)" in refresh_command
    assert f"- Bash({CLAUDE_ROUTER_RS_DEBUG_BINARY} *)" in refresh_command
    assert "- Bash(*scripts/router-rs/target/release/router-rs *)" in refresh_command
    assert "- Bash(*scripts/router-rs/target/debug/router-rs *)" in refresh_command
    assert (
        "- Bash(cargo run --manifest-path *scripts/router-rs/Cargo.toml --release -- *)"
        in refresh_command
    )
    assert "python3 scripts/router_rs_runner.py" not in refresh_command
    assert "copy `recap.workflow_prompt`" not in refresh_command
    assert "runtime_background_cli.py" in background_batch_command
    assert "enqueue-batch" in background_batch_command
    assert "group-summary" in background_batch_command
    assert "list-groups" in background_batch_command
    assert "allowed-tools: Bash(python3 scripts/runtime_background_cli.py *)" in background_batch_command
    assert "thin Rust-first alias" in autopilot_command
    assert aliases["autopilot"]["host_entrypoints"]["claude-code"] in autopilot_command
    assert "--framework-alias-json" in autopilot_command
    assert "--framework-alias autopilot" in autopilot_command
    assert "--compact-output" in autopilot_command
    assert "--claude-hook-max-lines 3" in autopilot_command
    assert "resident Rust binary directly" in autopilot_command
    assert "alias.state_machine" in autopilot_command
    assert "alias.entry_contract" in autopilot_command
    assert "explicit entrypoints: `/autopilot`, `$autopilot`" in autopilot_command
    assert "Implicit routing policy: `never`" in autopilot_command
    assert CLAUDE_PROJECT_DIR_SNIPPET in autopilot_command
    assert '"$PROJECT_DIR"/scripts/router-rs/target/release/router-rs' in autopilot_command
    assert '"$PROJECT_DIR"/scripts/router-rs/target/debug/router-rs' in autopilot_command
    assert (
        'cargo run --manifest-path "$PROJECT_DIR"/scripts/router-rs/Cargo.toml --release -- --framework-alias-json --framework-alias autopilot --compact-output --claude-hook-max-lines 3 --repo-root "$PROJECT_DIR"'
        in autopilot_command
    )
    assert "python3 scripts/router_rs_runner.py" not in autopilot_command
    assert aliases["autopilot"]["upstream_source"]["official_skill_path"] in autopilot_command
    assert "Only open" in autopilot_command
    assert "thin Rust-first alias" in deepinterview_command
    assert aliases["deepinterview"]["host_entrypoints"]["claude-code"] in deepinterview_command
    assert "--framework-alias-json" in deepinterview_command
    assert "--framework-alias deepinterview" in deepinterview_command
    assert "--compact-output" in deepinterview_command
    assert "--claude-hook-max-lines 3" in deepinterview_command
    assert "resident Rust binary directly" in deepinterview_command
    assert "alias.state_machine" in deepinterview_command
    assert "alias.entry_contract" in deepinterview_command
    assert "explicit entrypoints: `/deepinterview`, `$deepinterview`" in deepinterview_command
    assert "Implicit routing policy: `never`" in deepinterview_command
    assert CLAUDE_PROJECT_DIR_SNIPPET in deepinterview_command
    assert '"$PROJECT_DIR"/scripts/router-rs/target/release/router-rs' in deepinterview_command
    assert '"$PROJECT_DIR"/scripts/router-rs/target/debug/router-rs' in deepinterview_command
    assert (
        'cargo run --manifest-path "$PROJECT_DIR"/scripts/router-rs/Cargo.toml --release -- --framework-alias-json --framework-alias deepinterview --compact-output --claude-hook-max-lines 3 --repo-root "$PROJECT_DIR"'
        in deepinterview_command
    )
    assert "python3 scripts/router_rs_runner.py" not in deepinterview_command
    assert aliases["deepinterview"]["upstream_source"]["official_skill_path"] in deepinterview_command
    assert "Only open" in deepinterview_command
    assert "thin Rust-first alias" in team_command
    assert aliases["team"]["host_entrypoints"]["claude-code"] in team_command
    assert "--framework-alias-json" in team_command
    assert "--framework-alias team" in team_command
    assert "--compact-output" in team_command
    assert "--claude-hook-max-lines 3" in team_command
    assert "resident Rust binary directly" in team_command
    assert "alias.state_machine" in team_command
    assert "alias.entry_contract" in team_command
    assert "explicit entrypoints: `/team`, `$team`" in team_command
    assert "Implicit routing policy: `strong-orchestration-only`" in team_command
    assert CLAUDE_PROJECT_DIR_SNIPPET in team_command
    assert '"$PROJECT_DIR"/scripts/router-rs/target/release/router-rs' in team_command
    assert '"$PROJECT_DIR"/scripts/router-rs/target/debug/router-rs' in team_command
    assert (
        'cargo run --manifest-path "$PROJECT_DIR"/scripts/router-rs/Cargo.toml --release -- --framework-alias-json --framework-alias team --compact-output --claude-hook-max-lines 3 --repo-root "$PROJECT_DIR"'
        in team_command
    )
    assert "python3 scripts/router_rs_runner.py" not in team_command
    assert aliases["team"]["upstream_source"]["official_skill_path"] in team_command
    assert "Only open" in team_command
    assert "thin Rust-first alias" in latex_compile_acceleration_command
    assert aliases["latex-compile-acceleration"]["host_entrypoints"]["claude-code"] in latex_compile_acceleration_command
    assert "--framework-alias-json" in latex_compile_acceleration_command
    assert "--framework-alias latex-compile-acceleration" in latex_compile_acceleration_command
    assert "--compact-output" in latex_compile_acceleration_command
    assert "--claude-hook-max-lines 3" in latex_compile_acceleration_command
    assert "resident Rust binary directly" in latex_compile_acceleration_command
    assert "alias.state_machine" in latex_compile_acceleration_command
    assert "alias.entry_contract" in latex_compile_acceleration_command
    assert "explicit entrypoints: `/latex-compile-acceleration`, `$latex-compile-acceleration`" in latex_compile_acceleration_command
    assert "Implicit routing policy: `measurement-only`" in latex_compile_acceleration_command
    assert CLAUDE_PROJECT_DIR_SNIPPET in latex_compile_acceleration_command
    assert '"$PROJECT_DIR"/scripts/router-rs/target/release/router-rs' in latex_compile_acceleration_command
    assert '"$PROJECT_DIR"/scripts/router-rs/target/debug/router-rs' in latex_compile_acceleration_command
    assert (
        'cargo run --manifest-path "$PROJECT_DIR"/scripts/router-rs/Cargo.toml --release -- --framework-alias-json --framework-alias latex-compile-acceleration --compact-output --claude-hook-max-lines 3 --repo-root "$PROJECT_DIR"'
        in latex_compile_acceleration_command
    )
    assert "python3 scripts/router_rs_runner.py" not in latex_compile_acceleration_command
    assert aliases["latex-compile-acceleration"]["upstream_source"]["official_skill_path"] in latex_compile_acceleration_command
    assert "Only open" in latex_compile_acceleration_command
    assert "Otherwise run" not in latex_compile_acceleration_command
    assert "Otherwise run" not in autopilot_command
    assert "Otherwise run" not in deepinterview_command
    assert "Otherwise run" not in team_command
    assert (tmp_path / ".claude" / "hooks" / "README.md").is_file()
    hooks_readme = (tmp_path / ".claude" / "hooks" / "README.md").read_text(encoding="utf-8")
    assert "Generated-first maintenance" in hooks_readme
    assert "update `scripts/router-rs/` first for Claude hook rules and contracts" in hooks_readme
    assert "Event-level lifecycle decisions live in `.claude/hooks/README.md`." in claude_entry
    for marker in (
        "`PreToolUse` | `run.sh pre-tool-use`",
        "`SessionEnd` | `run.sh session-end`",
        "`ConfigChange` | `run.sh config-change`",
        "`StopFailure` | `run.sh stop-failure`",
        "generated-surface guard",
        "intentionally uninstalled",
        "broad implementation philosophy still live in",
        "repo-specific invariants only",
        "Use `matcher` first and `if` to narrow further",
        "`UserPromptSubmit` is installed here on purpose",
        "only add a one-line closeout reminder",
        "permissionDecision: deny",
    ):
        assert marker in hooks_readme
    assert (tmp_path / ".claude" / "hooks" / "run.sh").is_file()
    assert not (tmp_path / ".claude" / "hooks" / "session_start.sh").exists()
    assert not (tmp_path / ".claude" / "hooks" / "stop.sh").exists()
    assert not (tmp_path / ".claude" / "hooks" / "pre_compact.sh").exists()
    assert not (tmp_path / ".claude" / "hooks" / "subagent_stop.sh").exists()


def test_materialized_claude_hook_runner_covers_current_rust_commands(tmp_path: Path) -> None:
    materialize_repo_host_entrypoints(tmp_path)
    hook_runner_script = (tmp_path / ".claude" / "hooks" / "run.sh").read_text(encoding="utf-8")

    for command_name, expected_fragment in (
        ("session-end", 'run_router_rs --claude-hook-command session-end --repo-root "$PROJECT_DIR" >/dev/null'),
        ("config-change", 'config-change|stop-failure)'),
        ("stop-failure", 'config-change|stop-failure)'),
        ("pre-tool-use", 'run_router_rs --claude-hook-audit-command pre-tool-use --repo-root "$PROJECT_DIR"'),
        ("user-prompt-submit", 'run_router_rs --claude-hook-audit-command "$command_name" --repo-root "$PROJECT_DIR"'),
        ("pre-tool-use-quality", 'pre-tool-use-quality|post-tool-audit)'),
        ("post-tool-audit", 'pre-tool-use-quality|post-tool-audit)'),
    ):
        assert command_name in hook_runner_script
        assert expected_fragment in hook_runner_script

    for retired_command in ("session-start", "stop", "pre-compact", "subagent-stop"):
        assert f'  {retired_command})' not in hook_runner_script


def test_materialize_repo_host_entrypoints_is_idempotent(tmp_path: Path) -> None:
    materialize_repo_host_entrypoints(tmp_path)
    result = materialize_repo_host_entrypoints(tmp_path)

    assert result["written"] == []
    assert "AGENT.md" in result["unchanged"]
    assert ".codex/hooks.json" in result["unchanged"]
    assert ".claude/settings.json" in result["unchanged"]
    assert ".claude/CLAUDE.md" not in result["unchanged"]
    assert "configs/codex/AGENTS.md" not in result["unchanged"]
    assert "configs/claude/CLAUDE.md" not in result["unchanged"]
    assert "configs/gemini/GEMINI.md" not in result["unchanged"]


def test_sync_repo_host_entrypoints_reports_drift_without_writing(tmp_path: Path) -> None:
    result = sync_repo_host_entrypoints(tmp_path, apply=False)

    assert "AGENT.md" in result["written"]
    assert not (tmp_path / "AGENT.md").exists()


def test_materialize_repo_host_entrypoints_retires_redundant_claude_and_config_proxies(tmp_path: Path) -> None:
    for relative in (
        ".claude/CLAUDE.md",
        "configs/codex/AGENTS.md",
        "configs/claude/CLAUDE.md",
        "configs/gemini/GEMINI.md",
    ):
        path = tmp_path / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("legacy proxy\n", encoding="utf-8")

    result = materialize_repo_host_entrypoints(tmp_path)

    assert ".claude/CLAUDE.md" in result["written"]
    assert "configs/codex/AGENTS.md" in result["written"]
    assert "configs/claude/CLAUDE.md" in result["written"]
    assert "configs/gemini/GEMINI.md" in result["written"]
    assert not (tmp_path / ".claude" / "CLAUDE.md").exists()
    assert not (tmp_path / "configs/codex/AGENTS.md").exists()
    assert not (tmp_path / "configs/claude/CLAUDE.md").exists()
    assert not (tmp_path / "configs/gemini/GEMINI.md").exists()


def test_materialize_repo_host_entrypoints_syncs_matching_worktrees(tmp_path: Path) -> None:
    _init_git_repo(tmp_path)
    peer_worktree = tmp_path / ".claude" / "worktrees" / "agent-peer"
    peer_worktree.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(["git", "worktree", "add", str(peer_worktree), "--detach"], cwd=tmp_path, check=True)
    _write_text(peer_worktree / ".claude" / "hooks" / "session_start.sh", "#!/bin/sh\n")
    _write_text(peer_worktree / ".claude" / "hooks" / "subagent_stop.sh", "#!/bin/sh\n")
    _write_text(peer_worktree / ".claude" / "settings.json", '{"legacy": true}\n')

    result = materialize_repo_host_entrypoints(tmp_path)

    assert str(peer_worktree.resolve()) in result["synced_worktrees"]
    assert (peer_worktree / ".claude" / "commands" / "refresh.md").is_file()
    assert (peer_worktree / ".claude" / "commands" / "background_batch.md").is_file()
    assert (peer_worktree / ".claude" / "hooks" / "run.sh").is_file()
    assert json.loads((peer_worktree / ".claude" / "settings.json").read_text(encoding="utf-8"))["$schema"] == (
        "https://json.schemastore.org/claude-code-settings.json"
    )
    assert not (peer_worktree / ".claude" / "hooks" / "session_start.sh").exists()
    assert not (peer_worktree / ".claude" / "hooks" / "subagent_stop.sh").exists()
    assert (
        peer_worktree / ".claude" / "commands" / "refresh.md"
    ).read_text(encoding="utf-8") == CLAUDE_REFRESH_COMMAND
    assert (
        peer_worktree / ".claude" / "commands" / "background_batch.md"
    ).read_text(encoding="utf-8") == CLAUDE_BACKGROUND_BATCH_COMMAND


def test_write_generated_files_includes_shared_cli_entrypoints_when_repo_is_dirty(tmp_path: Path) -> None:
    materialize_repo_host_entrypoints(tmp_path)

    target = tmp_path / "CLAUDE.md"
    original = target.read_text(encoding="utf-8")
    target.write_text(original + "\nDRIFT\n", encoding="utf-8")

    result = sync_repo_host_entrypoints(tmp_path, apply=False)
    assert "CLAUDE.md" in result["written"]


def test_materialized_claude_hooks_execute_without_error(tmp_path: Path) -> None:
    _ensure_router_rs_binaries()
    materialize_repo_host_entrypoints(tmp_path)
    (tmp_path / "scripts").symlink_to(PROJECT_ROOT / "scripts", target_is_directory=True)
    _seed_runtime_artifacts(tmp_path)
    _seed_shared_memory(tmp_path)

    env = os.environ.copy()
    env["CLAUDE_PROJECT_DIR"] = str(tmp_path)

    blocked = subprocess.run(
        ["sh", str(tmp_path / ".claude" / "hooks" / "run.sh"), "pre-tool-use"],
        cwd=tmp_path,
        env=env,
        input='{"tool_name":"MultiEdit","tool_input":{"file_path":".claude/settings.json"}}\n',
        text=True,
        capture_output=True,
    )
    assert blocked.returncode == 0
    blocked_payload = json.loads(blocked.stdout)
    assert blocked_payload["hookSpecificOutput"]["permissionDecision"] == "deny"
    assert ".claude/settings.json" in blocked_payload["hookSpecificOutput"]["permissionDecisionReason"]

    user_prompt = subprocess.run(
        ["sh", str(tmp_path / ".claude" / "hooks" / "run.sh"), "user-prompt-submit"],
        cwd=tmp_path,
        env=env,
        input='{"hook_event_name":"UserPromptSubmit","prompt":"继续优化 runtime，去掉补丁式保底并顺手看内存和速度"}\n',
        text=True,
        capture_output=True,
    )
    assert user_prompt.returncode == 0
    user_prompt_payload = json.loads(user_prompt.stdout)
    assert user_prompt_payload["hookSpecificOutput"]["hookEventName"] == "UserPromptSubmit"
    context = user_prompt_payload["hookSpecificOutput"]["additionalContext"]
    telemetry = user_prompt_payload["contextTelemetry"]
    assert "repo-local shared memory" in context
    assert "当前状态：" in context
    assert "收尾提醒：" in context
    assert "热路径" in context
    assert "Task Snapshot" not in context
    assert len(context) <= 420
    assert telemetry["budget_chars"] == 420
    assert telemetry["state_budget_chars"] == 120
    assert telemetry["trimmed"] is False
    assert "memory-truth" in telemetry["lanes"]
    assert "continuity-truth" in telemetry["lanes"]
    assert "state-compact" in telemetry["lanes"]

    quality_context = subprocess.run(
        ["sh", str(tmp_path / ".claude" / "hooks" / "run.sh"), "pre-tool-use-quality"],
        cwd=tmp_path,
        env=env,
        input='{"tool_name":"Edit","tool_input":{"file_path":"tests/test_cli_host_entrypoints.py"}}\n',
        text=True,
        capture_output=True,
    )
    assert quality_context.returncode == 0
    quality_payload = json.loads(quality_context.stdout)
    assert quality_payload["hookSpecificOutput"]["permissionDecision"] == "allow"
    assert "测试额外检查" in quality_payload["hookSpecificOutput"]["additionalContext"]

    materializer_quality_context = subprocess.run(
        ["sh", str(tmp_path / ".claude" / "hooks" / "run.sh"), "pre-tool-use-quality"],
        cwd=tmp_path,
        env=env,
        input='{"tool_name":"Edit","tool_input":{"file_path":"scripts/materialize_cli_host_entrypoints.py"}}\n',
        text=True,
        capture_output=True,
    )
    assert materializer_quality_context.returncode == 0
    materializer_quality_payload = json.loads(materializer_quality_context.stdout)
    assert materializer_quality_payload["hookSpecificOutput"]["permissionDecision"] == "allow"
    assert "Python 额外检查" in materializer_quality_payload["hookSpecificOutput"]["additionalContext"]

    hook_quality_context = subprocess.run(
        ["sh", str(tmp_path / ".claude" / "hooks" / "run.sh"), "pre-tool-use-quality"],
        cwd=tmp_path,
        env=env,
        input='{"tool_name":"Edit","tool_input":{"file_path":".claude/hooks/run.sh"}}\n',
        text=True,
        capture_output=True,
    )
    assert hook_quality_context.returncode == 0
    hook_quality_payload = json.loads(hook_quality_context.stdout)
    assert hook_quality_payload["hookSpecificOutput"]["permissionDecision"] == "allow"
    assert "Hook 额外检查" in hook_quality_payload["hookSpecificOutput"]["additionalContext"]

    allowed = subprocess.run(
        ["sh", str(tmp_path / ".claude" / "hooks" / "run.sh"), "pre-tool-use"],
        cwd=tmp_path,
        env=env,
        input='{"tool_name":"Edit","tool_input":{"file_path":"notes/todo.md"}}\n',
        text=True,
        capture_output=True,
    )
    assert allowed.returncode == 0
    assert allowed.stdout == ""
    assert allowed.stderr == ""

    bash_blocked = subprocess.run(
        ["sh", str(tmp_path / ".claude" / "hooks" / "run.sh"), "pre-tool-use"],
        cwd=tmp_path,
        env=env,
        input='{"tool_name":"Bash","tool_input":{"command":"cp tmp .claude/settings.json"}}\n',
        text=True,
        capture_output=True,
    )
    assert bash_blocked.returncode == 0
    bash_payload = json.loads(bash_blocked.stdout)
    assert bash_payload["hookSpecificOutput"]["permissionDecision"] == "deny"
    assert ".claude/settings.json" in bash_payload["hookSpecificOutput"]["permissionDecisionReason"]

    patchy_edit = subprocess.run(
        ["sh", str(tmp_path / ".claude" / "hooks" / "run.sh"), "post-tool-audit"],
        cwd=tmp_path,
        env=env,
        input='{"tool_name":"Edit","tool_input":{"file_path":"scripts/router-rs/src/claude_hooks.rs","new_string":"let a = foo.clone();\\nlet b = bar.clone();\\nlet g = serde_json::to_string(&x)?;\\n// legacy fallback compatibility patch"}}\n',
        text=True,
        capture_output=True,
    )
    assert patchy_edit.returncode == 0
    patchy_payload = json.loads(patchy_edit.stdout)
    assert patchy_payload["hookSpecificOutput"]["hookEventName"] == "PostToolUse"
    assert "异步实现复查" in patchy_payload["additionalContext"]

    for command_name in ("config-change", "stop-failure"):
        payload = None
        if command_name == "config-change":
            payload = '{"hook_event_name":"ConfigChange","source":"project_settings","file_path":".claude/settings.json"}\n'
        elif command_name == "stop-failure":
            payload = '{"hook_event_name":"StopFailure","error":"server_error"}\n'
        subprocess.run(
            ["sh", str(tmp_path / ".claude" / "hooks" / "run.sh"), command_name],
            cwd=tmp_path,
            check=True,
            env=env,
            input=payload,
            text=True,
        )
        assert not (tmp_path / ".codex" / "memory" / "MEMORY_AUTO.md").exists()

    subprocess.run(
        ["sh", str(tmp_path / ".claude" / "hooks" / "run.sh"), "session-end"],
        cwd=tmp_path,
        check=True,
        env=env,
    )
    assert (tmp_path / ".codex" / "memory" / "CLAUDE_MEMORY.md").is_file()


def test_materialized_codex_hooks_are_disabled(tmp_path: Path) -> None:
    materialize_repo_host_entrypoints(tmp_path)

    hooks = json.loads((tmp_path / ".codex" / "hooks.json").read_text(encoding="utf-8"))
    assert hooks == {"hooks": {}}


def test_pre_tool_use_hook_uses_repo_local_audit_without_router_rs_bootstrap(tmp_path: Path) -> None:
    materialize_repo_host_entrypoints(tmp_path)
    (tmp_path / "scripts").symlink_to(PROJECT_ROOT / "scripts", target_is_directory=True)
    cargo_bin_dir = tmp_path / "fake-bin"
    cargo_log = tmp_path / "cargo-args.txt"
    cargo_bin_dir.mkdir(parents=True)
    _write_text(
        cargo_bin_dir / "cargo",
        "\n".join(
            [
                "#!/bin/sh",
                "set -eu",
                f"printf '%s\\n' \"$@\" > '{cargo_log}'",
            ]
        )
        + "\n",
    )
    os.chmod(cargo_bin_dir / "cargo", 0o755)

    env = os.environ.copy()
    env["CLAUDE_PROJECT_DIR"] = str(tmp_path)
    env["PATH"] = f"{cargo_bin_dir}{os.pathsep}{env.get('PATH', '')}"

    allowed = subprocess.run(
        ["sh", str(tmp_path / ".claude" / "hooks" / "run.sh"), "pre-tool-use"],
        cwd=tmp_path,
        env=env,
        input='{"tool_name":"Edit","tool_input":{"file_path":"notes/todo.md"}}\n',
        text=True,
        capture_output=True,
    )

    assert allowed.returncode == 0
    assert allowed.stdout == ""
    assert allowed.stderr == ""
    assert not cargo_log.exists()


def test_pre_tool_use_hook_blocks_without_router_rs_binary(tmp_path: Path) -> None:
    materialize_repo_host_entrypoints(tmp_path)
    (tmp_path / "scripts").symlink_to(PROJECT_ROOT / "scripts", target_is_directory=True)

    env = os.environ.copy()
    env["CLAUDE_PROJECT_DIR"] = str(tmp_path)
    env["PATH"] = "/usr/bin:/bin"

    blocked = subprocess.run(
        ["sh", str(tmp_path / ".claude" / "hooks" / "run.sh"), "pre-tool-use"],
        cwd=tmp_path,
        env=env,
        input='{"tool_name":"Edit","tool_input":{"file_path":".claude/settings.json"}}\n',
        text=True,
        capture_output=True,
    )

    assert blocked.returncode == 0
    blocked_payload = json.loads(blocked.stdout)
    assert blocked_payload["hookSpecificOutput"]["permissionDecision"] == "deny"
    assert ".claude/settings.json" in blocked_payload["hookSpecificOutput"]["permissionDecisionReason"]
    assert blocked.stderr == ""


def test_user_prompt_submit_hook_avoids_cargo_bootstrap_on_hot_path(tmp_path: Path) -> None:
    materialize_repo_host_entrypoints(tmp_path)
    (tmp_path / "scripts").symlink_to(PROJECT_ROOT / "scripts", target_is_directory=True)
    cargo_bin_dir = tmp_path / "fake-bin"
    cargo_log = tmp_path / "cargo-args.txt"
    cargo_bin_dir.mkdir(parents=True)
    _write_text(
        cargo_bin_dir / "cargo",
        "\n".join(
            [
                "#!/bin/sh",
                "set -eu",
                f"printf '%s\\n' \"$@\" > '{cargo_log}'",
            ]
        )
        + "\n",
    )
    os.chmod(cargo_bin_dir / "cargo", 0o755)

    env = os.environ.copy()
    env["CLAUDE_PROJECT_DIR"] = str(tmp_path)
    env["PATH"] = f"{cargo_bin_dir}{os.pathsep}{env.get('PATH', '')}"

    result = subprocess.run(
        ["sh", str(tmp_path / ".claude" / "hooks" / "run.sh"), "user-prompt-submit"],
        cwd=tmp_path,
        env=env,
        input='{"hook_event_name":"UserPromptSubmit","prompt":"继续优化 runtime"}\n',
        text=True,
        capture_output=True,
    )

    assert result.returncode == 0
    assert "repo-local shared memory" in result.stdout
    assert not cargo_log.exists()


def test_user_prompt_submit_hook_emits_stderr_notice_when_router_response_has_no_hook_specific_output(
    tmp_path: Path,
) -> None:
    materialize_repo_host_entrypoints(tmp_path)
    scripts_root = tmp_path / "scripts"
    (scripts_root / "router-rs").mkdir(parents=True)
    _write_text(
        scripts_root / "router-rs" / "Cargo.toml",
        "[package]\nname = \"router-rs\"\nversion = \"0.0.0\"\n",
    )
    _write_text(
        scripts_root / "router-rs" / "target" / "debug" / "router-rs",
        "#!/bin/sh\nprintf '%s\\n' '{\"command\":\"user-prompt-submit\"}'\n",
    )
    os.chmod(scripts_root / "router-rs" / "target" / "debug" / "router-rs", 0o755)

    env = os.environ.copy()
    env["CLAUDE_PROJECT_DIR"] = str(tmp_path)
    env["PATH"] = "/usr/bin:/bin"

    result = subprocess.run(
        ["sh", str(tmp_path / ".claude" / "hooks" / "run.sh"), "user-prompt-submit"],
        cwd=tmp_path,
        env=env,
        input='{"hook_event_name":"UserPromptSubmit","prompt":"继续优化 runtime"}\n',
        text=True,
        capture_output=True,
    )

    assert result.returncode == 0
    assert result.stdout == ""
    assert "shared hook returned no hookSpecificOutput; continuing with degraded context" in result.stderr


def test_session_end_projection_includes_preferences(tmp_path: Path) -> None:
    materialize_repo_host_entrypoints(tmp_path)
    scripts_path = tmp_path / "scripts"
    if scripts_path.exists() or scripts_path.is_symlink():
        shutil.rmtree(scripts_path)
    (tmp_path / "scripts").symlink_to(PROJECT_ROOT / "scripts", target_is_directory=True)
    _seed_runtime_artifacts(tmp_path)
    _seed_shared_memory(tmp_path)
    _write_text(
        tmp_path / ".codex" / "memory" / "preferences.md",
        "# preferences\n\n## 处理偏好\n\n- Prefer direct answers\n",
    )

    env = os.environ.copy()
    env["CLAUDE_PROJECT_DIR"] = str(tmp_path)
    subprocess.run(
        ["sh", str(tmp_path / ".claude" / "hooks" / "run.sh"), "session-end"],
        cwd=tmp_path,
        check=True,
        env=env,
    )

    projection = (tmp_path / ".codex" / "memory" / "CLAUDE_MEMORY.md").read_text(encoding="utf-8")
    assert "Prefer direct answers" in projection
    assert "artifacts/current/active_task.json" in projection
    assert "->" not in projection
    assert len(projection.splitlines()) <= 24


def test_claude_statusline_renders_runtime_summary(tmp_path: Path) -> None:
    focus_task_id = "validate-status-line-20260423010101"
    task_root = tmp_path / "artifacts" / "current" / focus_task_id
    _write_json(
        tmp_path / "artifacts" / "current" / "active_task.json",
        {"task_id": focus_task_id, "task": "Validate status line"},
    )
    _write_json(
        tmp_path / "artifacts" / "current" / "focus_task.json",
        {"task_id": focus_task_id, "task": "Validate status line"},
    )
    _write_json(
        tmp_path / "artifacts" / "current" / "task_registry.json",
        {
            "schema_version": "task-registry-v1",
            "focus_task_id": focus_task_id,
            "tasks": [
                {
                    "task_id": focus_task_id,
                    "task": "Validate status line",
                    "phase": "integration",
                    "status": "in_progress",
                    "resume_allowed": True,
                }
            ],
        },
    )
    _write_text(
        task_root / "SESSION_SUMMARY.md",
        "\n".join([
            "# SESSION_SUMMARY",
            "",
            "- task: Validate status line",
            "- phase: integration",
            "- status: in_progress",
        ])
        + "\n",
    )
    _write_json(
        task_root / "TRACE_METADATA.json",
        {
            "matched_skills": ["execution-controller-coding", "checklist-fixer"],
            "verification_status": "completed",
        },
    )
    _write_json(task_root / "NEXT_ACTIONS.json", {"next_actions": ["Ship it"]})
    _write_json(task_root / "EVIDENCE_INDEX.json", {"artifacts": []})
    _write_json(
        tmp_path / ".supervisor_state.json",
        {
            "task_id": focus_task_id,
            "task_summary": "Validate status line",
            "active_phase": "integration",
            "verification": {"verification_status": "in_progress"},
            "continuity": {"story_state": "active", "resume_allowed": True},
        },
    )

    statusline = render_statusline(tmp_path)

    assert "task=Validate status line" in statusline
    assert "next=/refresh" in statusline
    assert "integration/in_progress" in statusline
    assert "route=execution-controller-coding+1" in statusline
    assert "others=0" in statusline
    assert "resumable=0" in statusline
    assert "git=nogit" in statusline


def test_claude_statusline_prefers_task_scoped_runtime_over_stale_root_mirrors(tmp_path: Path) -> None:
    task_root = tmp_path / "artifacts" / "current" / "fresh-task-20260419013000"
    _write_json(
        tmp_path / "artifacts" / "current" / "active_task.json",
        {"task_id": "stale-active-task-20260419012000", "task": "Stale active task"},
    )
    _write_json(
        tmp_path / "artifacts" / "current" / "focus_task.json",
        {"task_id": "fresh-task-20260419013000", "task": "Fresh current task"},
    )
    _write_json(
        tmp_path / "artifacts" / "current" / "task_registry.json",
        {
            "schema_version": "task-registry-v1",
            "focus_task_id": "fresh-task-20260419013000",
            "tasks": [
                {
                    "task_id": "fresh-task-20260419013000",
                    "task": "Fresh current task",
                    "phase": "integration",
                    "status": "in_progress",
                    "resume_allowed": True,
                },
                {
                    "task_id": "background-task-20260419014000",
                    "task": "Background follow-up",
                    "phase": "implementation",
                    "status": "in_progress",
                    "resume_allowed": True,
                },
            ],
        },
    )
    _write_text(
        task_root / "SESSION_SUMMARY.md",
        "\n".join([
            "# SESSION_SUMMARY",
            "",
            "- task: Fresh current task",
            "- phase: integration",
            "- status: in_progress",
        ])
        + "\n",
    )
    _write_json(task_root / "NEXT_ACTIONS.json", {"next_actions": ["Ship the fix"]})
    _write_json(task_root / "EVIDENCE_INDEX.json", {"artifacts": []})
    _write_json(
        task_root / "TRACE_METADATA.json",
        {
            "matched_skills": ["execution-controller-coding", "agent-memory"],
            "verification_status": "in_progress",
        },
    )
    _write_text(
        tmp_path / "SESSION_SUMMARY.md",
        "\n".join([
            "# SESSION_SUMMARY",
            "",
            "- task: Stale root task",
            "- phase: finalized",
            "- status: completed",
        ])
        + "\n",
    )
    _write_json(
        tmp_path / "TRACE_METADATA.json",
        {
            "matched_skills": ["checklist-fixer"],
            "verification_status": "completed",
        },
    )
    _write_json(tmp_path / "NEXT_ACTIONS.json", {"next_actions": ["Ignore me"]})
    _write_json(
        tmp_path / ".supervisor_state.json",
        {
            "task_id": "fresh-task-20260419013000",
            "task_summary": "Fresh current task",
            "active_phase": "integration",
            "verification": {"verification_status": "in_progress"},
            "continuity": {"story_state": "active", "resume_allowed": True},
        },
    )

    statusline = render_statusline(tmp_path)

    assert "task=Fresh current task" in statusline
    assert "integration/in_progress" in statusline
    assert "route=execution-controller-coding+1" in statusline
    assert "others=1" in statusline
    assert "resumable=1" in statusline
    assert "Stale root task" not in statusline


def test_claude_statusline_prefers_supervisor_owned_actions_over_stale_sidecars(tmp_path: Path) -> None:
    task_root = tmp_path / "artifacts" / "current" / "fresh-task-20260419013000"
    _write_json(
        tmp_path / "artifacts" / "current" / "active_task.json",
        {"task_id": "fresh-task-20260419013000", "task": "Fresh current task"},
    )
    _write_text(
        task_root / "SESSION_SUMMARY.md",
        "\n".join([
            "# SESSION_SUMMARY",
            "",
            "- task: Fresh current task",
            "- phase: verification",
            "- status: completed",
        ])
        + "\n",
    )
    _write_json(task_root / "NEXT_ACTIONS.json", {"next_actions": ["stale sidecar action"]})
    _write_json(task_root / "EVIDENCE_INDEX.json", {"artifacts": []})
    _write_json(
        task_root / "TRACE_METADATA.json",
        {
            "task": "Fresh current task",
            "matched_skills": ["legacy-skill"],
            "verification_status": "completed",
            "routing_runtime_version": 0,
        },
    )
    _write_json(
        tmp_path / ".supervisor_state.json",
        {
            "task_id": "fresh-task-20260419013000",
            "task_summary": "Fresh current task",
            "active_phase": "verification",
            "verification": {"verification_status": "completed"},
            "continuity": {"story_state": "completed", "resume_allowed": False},
            "next_actions": ["Ship the real follow-up"],
            "controller": {
                "primary_owner": "execution-controller-coding",
                "gate": "subagent-delegation",
            },
        },
    )

    statusline = render_statusline(tmp_path)

    assert "next=Ship the real follow-up" in statusline
    assert "route=subagent-delegation+1" in statusline
    assert "legacy-skill" not in statusline


def test_claude_hook_audit_reports_generated_surface_drift(tmp_path: Path) -> None:
    result = _run_router_rs_claude_audit(
        "config-change",
        tmp_path,
        {
            "source": "project_settings",
            "file_path": str(tmp_path / ".claude" / "settings.json"),
        },
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout)
    assert "generated Claude host surfaces" in result.stderr
    assert "scripts/materialize_cli_host_entrypoints.py" in result.stderr
    assert any("generated Claude host surfaces" in notice for notice in payload["notices"])


def test_claude_pre_tool_use_blocks_generated_host_surfaces(tmp_path: Path) -> None:
    result = _run_router_rs_claude_audit(
        "pre-tool-use",
        tmp_path,
        {
            "tool_name": "MultiEdit",
            "tool_input": {"file_path": str(tmp_path / ".claude" / "settings.json")},
        },
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout)
    assert payload["hookSpecificOutput"]["permissionDecision"] == "deny"
    assert "generated host surface" in payload["hookSpecificOutput"]["permissionDecisionReason"]
    assert "materialize_cli_host_entrypoints.py" in payload["hookSpecificOutput"]["permissionDecisionReason"]


def test_claude_pre_tool_use_blocks_generated_codex_hook_manifest(tmp_path: Path) -> None:
    result = _run_router_rs_claude_audit(
        "pre-tool-use",
        tmp_path,
        {
            "tool_name": "Edit",
            "tool_input": {
                "file_path": str(tmp_path / ".codex" / "host_entrypoints_sync_manifest.json")
            },
        },
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout)
    assert payload["hookSpecificOutput"]["permissionDecision"] == "deny"
    assert ".codex/host_entrypoints_sync_manifest.json" in payload["hookSpecificOutput"]["permissionDecisionReason"]


def test_claude_pre_tool_use_allows_normal_workspace_files(tmp_path: Path) -> None:
    result = _run_router_rs_claude_audit(
        "pre-tool-use",
        tmp_path,
        {
            "tool_name": "Edit",
            "tool_input": {"file_path": str(tmp_path / "notes" / "todo.md")},
        },
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout)
    assert payload["decision"] == "allow"
    assert payload.get("hookSpecificOutput") is None
    assert result.stderr == ""


def test_claude_pre_tool_use_allows_local_settings_overlay(tmp_path: Path) -> None:
    result = _run_router_rs_claude_audit(
        "pre-tool-use",
        tmp_path,
        {
            "tool_name": "Edit",
            "tool_input": {"file_path": str(tmp_path / ".claude" / "settings.local.json")},
        },
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout)
    assert payload["decision"] == "allow"
    assert payload.get("hookSpecificOutput") is None
    assert result.stderr == ""


def test_claude_pre_tool_use_allows_manual_claude_agent_docs(tmp_path: Path) -> None:
    result = _run_router_rs_claude_audit(
        "pre-tool-use",
        tmp_path,
        {
            "tool_name": "Edit",
            "tool_input": {"file_path": str(tmp_path / ".claude" / "agents" / "custom.md")},
        },
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout)
    assert payload["decision"] == "allow"
    assert payload.get("hookSpecificOutput") is None
    assert result.stderr == ""


def test_claude_pre_tool_use_blocks_targeted_bash_writes(tmp_path: Path) -> None:
    result = _run_router_rs_claude_audit(
        "pre-tool-use",
        tmp_path,
        {
            "tool_name": "Bash",
            "tool_input": {"command": "cp tmp .claude/settings.json"},
        },
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout)
    assert payload["hookSpecificOutput"]["permissionDecision"] == "deny"
    assert ".claude/settings.json" in payload["hookSpecificOutput"]["permissionDecisionReason"]


def test_claude_pre_tool_use_blocks_shell_redirection_into_generated_files(
    tmp_path: Path,
) -> None:
    result = _run_router_rs_claude_audit(
        "pre-tool-use",
        tmp_path,
        {
            "tool_name": "Bash",
            "tool_input": {"command": "printf '{}' > .claude/settings.json"},
        },
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout)
    assert payload["hookSpecificOutput"]["permissionDecision"] == "deny"
    assert ".claude/settings.json" in payload["hookSpecificOutput"]["permissionDecisionReason"]


def test_claude_pre_tool_use_allows_reading_generated_files_after_unrelated_write(
    tmp_path: Path,
) -> None:
    result = _run_router_rs_claude_audit(
        "pre-tool-use",
        tmp_path,
        {
            "tool_name": "Bash",
            "tool_input": {"command": "cp tmp ./tmp.out && cat .claude/settings.json"},
        },
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout)
    assert payload["decision"] == "allow"
    assert payload.get("hookSpecificOutput") is None
    assert result.stderr == ""


def test_claude_pre_tool_use_allows_bash_write_to_local_settings_overlay(tmp_path: Path) -> None:
    result = _run_router_rs_claude_audit(
        "pre-tool-use",
        tmp_path,
        {
            "tool_name": "Bash",
            "tool_input": {"command": "printf '{}' > .claude/settings.local.json"},
        },
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout)
    assert payload["decision"] == "allow"
    assert payload.get("hookSpecificOutput") is None
    assert result.stderr == ""


def test_claude_hook_audit_reports_stop_failure_without_mutation(tmp_path: Path) -> None:
    result = _run_router_rs_claude_audit(
        "stop-failure",
        tmp_path,
        {
            "error": "server_error",
            "context": "host projection",
        },
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout)
    assert payload["failure_type"] == "server_error"
    assert "server_error" in result.stderr
    assert "host-private projection drift" in result.stderr
