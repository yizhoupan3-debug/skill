from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))


def _run_hook(command: str, payload: dict[str, object]) -> subprocess.CompletedProcess[bytes]:
    script = PROJECT_ROOT / "scripts" / "claude_hook_automation.py"
    return subprocess.run(
        ["python3", str(script), command, "--repo-root", str(PROJECT_ROOT)],
        input=json.dumps(payload, ensure_ascii=False).encode("utf-8"),
        capture_output=True,
        check=False,
    )


def test_user_prompt_submit_injects_coding_context_for_code_requests() -> None:
    result = _run_hook(
        "user-prompt-submit",
        {"hook_event_name": "UserPromptSubmit", "prompt": "继续优化 runtime，去掉补丁式保底并顺手看内存和速度"},
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout.decode("utf-8"))
    output = payload["hookSpecificOutput"]
    assert output["hookEventName"] == "UserPromptSubmit"
    assert "热路径" in output["additionalContext"]
    assert "fallback" in output["additionalContext"]
    assert "简化优先" not in output["additionalContext"]


def test_user_prompt_submit_stays_silent_for_non_coding_prompts() -> None:
    result = _run_hook(
        "user-prompt-submit",
        {"hook_event_name": "UserPromptSubmit", "prompt": "把这个结论改得更像人话一点"},
    )

    assert result.returncode == 0
    assert result.stdout == b""
    assert result.stderr == b""


def test_post_tool_audit_reports_patchy_rust_runtime_edits(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    target = repo_root / "scripts" / "router-rs" / "src" / "claude_hooks.rs"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "\n".join(
            [
                'let a = foo.clone();',
                'let b = bar.clone();',
                'let c = baz.clone();',
                'let d = q.clone();',
                'let e = w.clone();',
                'let f = e.clone();',
                'let g = serde_json::to_string(&x)?;',
                'let h = serde_json::to_string(&y)?;',
                'let i = serde_json::to_string(&z)?;',
                '// legacy fallback compatibility patch',
            ]
        )
        + "\n",
        encoding="utf-8",
    )

    script = PROJECT_ROOT / "scripts" / "claude_hook_automation.py"
    result = subprocess.run(
        ["python3", str(script), "post-tool-audit", "--repo-root", str(repo_root)],
        input=json.dumps(
            {
                "tool_name": "Edit",
                "tool_input": {
                    "file_path": str(target),
                    "old_string": "let old = 1;",
                    "new_string": "\n".join(
                        [
                            "let a = foo.clone();",
                            "let b = bar.clone();",
                            "let g = serde_json::to_string(&x)?;",
                            "// legacy fallback compatibility patch",
                        ]
                    ),
                },
            },
            ensure_ascii=False,
        ).encode("utf-8"),
        capture_output=True,
        check=False,
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout.decode("utf-8"))
    assert payload["hookSpecificOutput"]["hookEventName"] == "PostToolUse"
    assert "异步实现复查" in payload["additionalContext"]
    assert "增量来源=edit_new_string" in payload["additionalContext"]
    assert "clone=" in payload["additionalContext"]
    assert "新增兼容分支或中转层" in payload["additionalContext"]


def test_post_tool_audit_stays_silent_for_clean_non_target_edits(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    target = repo_root / "notes" / "todo.py"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text("print('ok')\n", encoding="utf-8")

    script = PROJECT_ROOT / "scripts" / "claude_hook_automation.py"
    result = subprocess.run(
        ["python3", str(script), "post-tool-audit", "--repo-root", str(repo_root)],
        input=json.dumps(
            {"tool_name": "Edit", "tool_input": {"file_path": str(target)}},
            ensure_ascii=False,
        ).encode("utf-8"),
        capture_output=True,
        check=False,
    )

    assert result.returncode == 0
    assert result.stdout == b""
    assert result.stderr == b""


def test_post_tool_audit_ignores_old_file_noise_when_new_edit_is_clean(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    target = repo_root / "scripts" / "claude_hook_automation.py"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "\n".join(
            [
                "legacy fallback compatibility patch",
                "json.dumps(x)",
                "json.loads(y)",
                "json.dumps(z)",
                "json.loads(w)",
            ]
        )
        + "\n",
        encoding="utf-8",
    )

    script = PROJECT_ROOT / "scripts" / "claude_hook_automation.py"
    result = subprocess.run(
        ["python3", str(script), "post-tool-audit", "--repo-root", str(repo_root)],
        input=json.dumps(
            {
                "tool_name": "Edit",
                "tool_input": {
                    "file_path": str(target),
                    "old_string": "old helper",
                    "new_string": "new_helper = build_context(payload)",
                },
            },
            ensure_ascii=False,
        ).encode("utf-8"),
        capture_output=True,
        check=False,
    )

    assert result.returncode == 0
    assert result.stdout == b""
    assert result.stderr == b""


def test_post_tool_audit_uses_pre_tool_snapshot_for_true_delta_review(tmp_path: Path) -> None:
    repo_root = tmp_path / "repo"
    target = repo_root / "scripts" / "claude_hook_automation.py"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        "\n".join(
            [
                "legacy fallback compatibility patch",
                "json.dumps(x)",
                "json.loads(y)",
                "json.dumps(z)",
                "json.loads(w)",
            ]
        )
        + "\n",
        encoding="utf-8",
    )

    script = PROJECT_ROOT / "scripts" / "claude_hook_automation.py"
    pre_result = subprocess.run(
        ["python3", str(script), "pre-tool-use-quality", "--repo-root", str(repo_root)],
        input=json.dumps(
            {"tool_name": "Edit", "tool_input": {"file_path": str(target)}},
            ensure_ascii=False,
        ).encode("utf-8"),
        capture_output=True,
        check=False,
    )
    assert pre_result.returncode == 0

    target.write_text(
        target.read_text(encoding="utf-8") + "helper = build_context(payload)\n",
        encoding="utf-8",
    )

    post_result = subprocess.run(
        ["python3", str(script), "post-tool-audit", "--repo-root", str(repo_root)],
        input=json.dumps(
            {"tool_name": "Edit", "tool_input": {"file_path": str(target)}},
            ensure_ascii=False,
        ).encode("utf-8"),
        capture_output=True,
        check=False,
    )

    assert post_result.returncode == 0
    assert post_result.stdout == b""
    assert post_result.stderr == b""


def test_user_prompt_submit_stays_silent_for_non_code_wording_edits() -> None:
    result = _run_hook(
        "user-prompt-submit",
        {"hook_event_name": "UserPromptSubmit", "prompt": "把这个结论改得更像人话一点，顺手润色一下措辞"},
    )

    assert result.returncode == 0
    assert result.stdout == b""
    assert result.stderr == b""


def test_user_prompt_submit_stays_silent_for_readme_doc_edits() -> None:
    result = _run_hook(
        "user-prompt-submit",
        {"hook_event_name": "UserPromptSubmit", "prompt": "改一下 README.md 的文档措辞，让说明更清楚"},
    )

    assert result.returncode == 0
    assert result.stdout == b""
    assert result.stderr == b""


def test_user_prompt_submit_stays_silent_for_hook_readme_doc_edits() -> None:
    result = _run_hook(
        "user-prompt-submit",
        {"hook_event_name": "UserPromptSubmit", "prompt": "优化 .claude/hooks/README.md，把说明写得更清楚"},
    )

    assert result.returncode == 0
    assert result.stdout == b""
    assert result.stderr == b""


def test_user_prompt_submit_stays_silent_for_agent_policy_docs() -> None:
    result = _run_hook(
        "user-prompt-submit",
        {"hook_event_name": "UserPromptSubmit", "prompt": "继续优化 AGENT.md，把 simplify 原则再收紧一点"},
    )

    assert result.returncode == 0
    assert result.stdout == b""
    assert result.stderr == b""


def test_user_prompt_submit_uses_path_mentions_to_raise_precision() -> None:
    result = _run_hook(
        "user-prompt-submit",
        {"hook_event_name": "UserPromptSubmit", "prompt": "继续改 scripts/router-rs/src/claude_hooks.rs，把 hook 自动化做准一点"},
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout.decode("utf-8"))
    output = payload["hookSpecificOutput"]
    assert "增加自动化" in output["additionalContext"]
    assert "matcher/if" in output["additionalContext"]


def test_user_prompt_submit_deduplicates_repeated_hook_context() -> None:
    result = _run_hook(
        "user-prompt-submit",
        {
            "hook_event_name": "UserPromptSubmit",
            "prompt": "继续改 scripts/router-rs/src/claude_hooks.rs，把 hook 自动化做准一点，hook 触发要更窄",
        },
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout.decode("utf-8"))
    output = payload["hookSpecificOutput"]
    assert output["additionalContext"].count("Hook 额外检查") == 1


def test_pre_tool_use_quality_injects_additional_context_for_runtime_code() -> None:
    result = _run_hook(
        "pre-tool-use-quality",
        {
            "tool_name": "Edit",
            "tool_input": {"file_path": str(PROJECT_ROOT / "scripts" / "router-rs" / "src" / "claude_hooks.rs")},
        },
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout.decode("utf-8"))
    output = payload["hookSpecificOutput"]
    assert output["permissionDecision"] == "allow"
    assert "热路径" in output["additionalContext"]
    assert "Rust 额外检查" in output["additionalContext"]
    assert "增加自动化" in output["additionalContext"]


def test_pre_tool_use_quality_injects_contract_context_for_tests() -> None:
    result = _run_hook(
        "pre-tool-use-quality",
        {
            "tool_name": "Edit",
            "tool_input": {"file_path": str(PROJECT_ROOT / "tests" / "test_claude_hook_automation.py")},
        },
    )

    assert result.returncode == 0
    payload = json.loads(result.stdout.decode("utf-8"))
    output = payload["hookSpecificOutput"]
    assert "测试额外检查" in output["additionalContext"]
    assert "补丁式旧行为" in output["additionalContext"]


def test_pre_tool_use_quality_stays_silent_for_non_code_files() -> None:
    result = _run_hook(
        "pre-tool-use-quality",
        {
            "tool_name": "Edit",
            "tool_input": {"file_path": str(PROJECT_ROOT / "README.md")},
        },
    )

    assert result.returncode == 0
    assert result.stdout == b""
    assert result.stderr == b""
