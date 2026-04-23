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
    stdout = result.stdout.decode("utf-8")
    assert "默认直接实现" in stdout
    assert "速度、内存" in stdout


def test_user_prompt_submit_stays_silent_for_non_coding_prompts() -> None:
    result = _run_hook(
        "user-prompt-submit",
        {"hook_event_name": "UserPromptSubmit", "prompt": "把这个结论改得更像人话一点"},
    )

    assert result.returncode == 0
    assert result.stdout == b""
    assert result.stderr == b""


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
    assert "直接实现目标行为" in output["additionalContext"]
    assert "Rust 额外检查" in output["additionalContext"]


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
