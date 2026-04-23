"""Regression coverage for repository-local Git safety helpers."""

from __future__ import annotations

import json
import subprocess
import sys
import tarfile
from unittest.mock import patch
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.git_safety import (
    _expand_verify_commands,
    _resolve_verification_command,
    build_verify_preset_suggestion,
    build_doctor_report,
    build_publish_plan,
    collect_repo_snapshot,
    render_auto_closeout,
    render_doctor_report,
    render_publish_plan,
    render_snapshot,
    render_verification_batch,
    run_auto_closeout,
    run_verification_batch,
    start_topic_branch,
    write_checkpoint,
)


def _init_repo(repo_root: Path) -> None:
    subprocess.run(["git", "init", "-b", "main"], cwd=repo_root, check=True)
    subprocess.run(["git", "config", "user.name", "Codex Test"], cwd=repo_root, check=True)
    subprocess.run(["git", "config", "user.email", "codex@example.com"], cwd=repo_root, check=True)
    (repo_root / ".gitignore").write_text("ignored.txt\nartifacts/\n", encoding="utf-8")
    (repo_root / "README.md").write_text("seed\n", encoding="utf-8")
    (repo_root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
    subprocess.run(["git", "add", ".gitignore", "README.md", "tracked.txt"], cwd=repo_root, check=True)
    subprocess.run(["git", "commit", "-m", "init"], cwd=repo_root, check=True)


def test_collect_repo_snapshot_reports_dirty_counts(tmp_path: Path) -> None:
    _init_repo(tmp_path)
    (tmp_path / "README.md").write_text("seed\nworktree change\n", encoding="utf-8")
    (tmp_path / "untracked.txt").write_text("draft\n", encoding="utf-8")
    (tmp_path / "ignored.txt").write_text("ignored\n", encoding="utf-8")

    snapshot = collect_repo_snapshot(tmp_path)
    rendered = render_snapshot(snapshot)

    assert snapshot.collection_mode == "parallel-probes"
    assert "status_porcelain" in snapshot.probe_timings_ms
    assert snapshot.branch.head_name == "main"
    assert snapshot.changes.tracked_paths == 1
    assert snapshot.changes.worktree_paths == 1
    assert snapshot.changes.index_paths == 0
    assert snapshot.changes.untracked_paths == 1
    assert snapshot.changes.ignored_paths == 1
    assert "audit_mode: parallel-probes" in rendered
    assert "tracked 1" in rendered
    assert "untracked 1" in rendered
    assert "当前脏改动直接堆在 main 上" in rendered


def test_write_checkpoint_captures_tracked_staged_and_untracked_state(tmp_path: Path) -> None:
    _init_repo(tmp_path)
    (tmp_path / "README.md").write_text("seed\nworktree change\n", encoding="utf-8")
    (tmp_path / "tracked.txt").write_text("tracked\nstaged change\n", encoding="utf-8")
    subprocess.run(["git", "add", "tracked.txt"], cwd=tmp_path, check=True)
    (tmp_path / "untracked.txt").write_text("draft\n", encoding="utf-8")

    snapshot = collect_repo_snapshot(tmp_path)
    checkpoint_dir = write_checkpoint(snapshot, label="smoke")

    metadata = json.loads((checkpoint_dir / "metadata.json").read_text(encoding="utf-8"))
    tracked_patch = (checkpoint_dir / "tracked.patch").read_text(encoding="utf-8")
    staged_patch = (checkpoint_dir / "staged.patch").read_text(encoding="utf-8")
    restore_doc = (checkpoint_dir / "RESTORE.md").read_text(encoding="utf-8")

    assert metadata["branch"]["head_name"] == "main"
    assert (checkpoint_dir / "status.porcelain-v2").is_file()
    assert (checkpoint_dir / "worktrees.porcelain").is_file()
    assert (checkpoint_dir / "reflog.txt").is_file()
    assert "README.md" in tracked_patch
    assert "tracked.txt" in staged_patch
    assert "git apply --index staged.patch" in restore_doc
    assert (checkpoint_dir / "untracked_files.txt").read_text(encoding="utf-8").strip() == "untracked.txt"

    with tarfile.open(checkpoint_dir / "untracked.tar.gz", "r:gz") as archive:
        assert archive.getnames() == ["untracked.txt"]


def test_start_topic_branch_checkpoints_and_preserves_dirty_worktree(tmp_path: Path) -> None:
    _init_repo(tmp_path)
    (tmp_path / "README.md").write_text("seed\nworktree change\n", encoding="utf-8")
    (tmp_path / "draft.txt").write_text("draft\n", encoding="utf-8")

    checkpoint_dir, snapshot = start_topic_branch("topic/git-hygiene", repo_root=tmp_path)

    current_branch = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=tmp_path,
        check=True,
        text=True,
        capture_output=True,
    ).stdout.strip()

    assert snapshot.branch.head_name == "main"
    assert current_branch == "topic/git-hygiene"
    assert (checkpoint_dir / "metadata.json").is_file()
    assert (tmp_path / "README.md").read_text(encoding="utf-8").endswith("worktree change\n")
    assert (tmp_path / "draft.txt").read_text(encoding="utf-8") == "draft\n"


def test_doctor_report_flags_dirty_main_and_suggests_topic_branch(tmp_path: Path) -> None:
    _init_repo(tmp_path)
    (tmp_path / "README.md").write_text("seed\nworktree change\n", encoding="utf-8")
    (tmp_path / "draft.txt").write_text("draft\n", encoding="utf-8")

    snapshot = collect_repo_snapshot(tmp_path)
    report = build_doctor_report(snapshot)
    rendered = render_doctor_report(report)

    assert report.risk_level == "high"
    assert report.suggested_topic_branch is not None
    assert "脏改动直接堆在 main 上" in rendered
    assert "start-topic" in rendered


def test_publish_plan_for_topic_branch_prefers_ff_only_merge_back_to_main(tmp_path: Path) -> None:
    _init_repo(tmp_path)
    subprocess.run(["git", "switch", "-c", "topic/gitx-smoke"], cwd=tmp_path, check=True)
    (tmp_path / "README.md").write_text("seed\ntopic change\n", encoding="utf-8")

    snapshot = collect_repo_snapshot(tmp_path)
    plan = build_publish_plan(snapshot, target_branch="main")
    rendered = render_publish_plan(plan)

    assert plan.blocked is False
    assert "git add <scoped-paths>" in rendered
    assert "git merge --ff-only topic/gitx-smoke" in rendered
    assert "git push origin main" in rendered


def test_auto_closeout_creates_topic_commit_and_merges_back_to_main(tmp_path: Path) -> None:
    _init_repo(tmp_path)
    (tmp_path / "README.md").write_text("seed\nauto closeout\n", encoding="utf-8")
    (tmp_path / "draft.txt").write_text("draft\n", encoding="utf-8")

    result = run_auto_closeout(repo_root=tmp_path, target_branch="main")

    current_branch = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=tmp_path,
        check=True,
        text=True,
        capture_output=True,
    ).stdout.strip()
    history = subprocess.run(
        ["git", "log", "--oneline", "-n", "2"],
        cwd=tmp_path,
        check=True,
        text=True,
        capture_output=True,
    ).stdout

    assert result.blocked is False
    assert result.created_topic_branch is not None
    assert result.commit_created is True
    assert result.merged_to_target is True
    assert current_branch == "main"
    assert "gitx auto closeout" in history


def test_auto_closeout_blocks_when_branch_has_unmerged_paths(tmp_path: Path) -> None:
    _init_repo(tmp_path)
    subprocess.run(["git", "switch", "-c", "topic/left"], cwd=tmp_path, check=True)
    (tmp_path / "README.md").write_text("left\n", encoding="utf-8")
    subprocess.run(["git", "commit", "-am", "left"], cwd=tmp_path, check=True)
    subprocess.run(["git", "switch", "main"], cwd=tmp_path, check=True)
    (tmp_path / "README.md").write_text("main\n", encoding="utf-8")
    subprocess.run(["git", "commit", "-am", "main"], cwd=tmp_path, check=True)
    merge_proc = subprocess.run(["git", "merge", "topic/left"], cwd=tmp_path, text=True, capture_output=True)

    assert merge_proc.returncode != 0

    result = run_auto_closeout(repo_root=tmp_path, target_branch="main")

    assert result.blocked is True
    assert "未解决冲突" in result.warnings[0]


def test_resolve_verification_command_wraps_rtk_for_noisy_commands() -> None:
    with patch("scripts.git_safety._rtk_available", return_value=True):
        resolved, used_rtk = _resolve_verification_command("pytest -q tests/test_git_safety.py", prefer_rtk=True)

    assert used_rtk is True
    assert resolved[:2] == ["rtk", "pytest"]


def test_resolve_verification_command_wraps_rtk_for_python_module_pytest() -> None:
    with patch("scripts.git_safety._rtk_available", return_value=True):
        resolved, used_rtk = _resolve_verification_command(
            "python3 -m pytest -q tests/test_git_safety.py",
            prefer_rtk=True,
        )

    assert used_rtk is True
    assert resolved[:4] == ["rtk", "python3", "-m", "pytest"]


def test_expand_verify_commands_supports_presets() -> None:
    expanded = _expand_verify_commands(
        commands=["python3 -c \"print('extra')\""],
        presets=["gitx-smoke"],
    )

    assert expanded[0]["command"].startswith("python3 -m pytest")
    assert expanded[0]["preset"] == "gitx-smoke"
    assert expanded[1]["command"] == "python3 -m py_compile scripts/git_safety.py"
    assert expanded[2]["command"] == "python3 -c \"print('extra')\""
    assert expanded[2]["preset"] is None


def test_expand_verify_commands_deduplicates_same_command_across_presets() -> None:
    with patch(
        "scripts.git_safety.VERIFY_PRESET_CONFIGS",
        {
            "high": {
                "label": "High",
                "priority": 90,
                "commands": [{"command": "python3 -c \"print('same')\""}],
                "path_rules": (),
            },
            "low": {
                "label": "Low",
                "priority": 10,
                "commands": [{"command": "python3 -c \"print('same')\""}],
                "path_rules": (),
            },
        },
    ):
        expanded = _expand_verify_commands(commands=[], presets=["low", "high"])

    assert len(expanded) == 1
    assert expanded[0]["preset"] == "high"
    assert expanded[0]["merged_from"] == ["high", "low"]


def test_expand_verify_commands_keeps_same_command_when_cwd_differs() -> None:
    with patch(
        "scripts.git_safety.VERIFY_PRESET_CONFIGS",
        {
            "a": {
                "label": "A",
                "priority": 20,
                "commands": [{"command": "python3 -c \"print('same')\"", "cwd": "a"}],
                "path_rules": (),
            },
            "b": {
                "label": "B",
                "priority": 10,
                "commands": [{"command": "python3 -c \"print('same')\"", "cwd": "b"}],
                "path_rules": (),
            },
        },
    ):
        expanded = _expand_verify_commands(commands=[], presets=["a", "b"])

    assert len(expanded) == 2
    assert {item["cwd"] for item in expanded} == {"a", "b"}


def test_build_verify_preset_suggestion_matches_changed_paths(tmp_path: Path) -> None:
    _init_repo(tmp_path)
    (tmp_path / "scripts").mkdir()
    (tmp_path / "scripts" / "git_safety.py").write_text("print('x')\n", encoding="utf-8")
    (tmp_path / "skills").mkdir()
    (tmp_path / "skills" / "gitx.S").write_text("x\n", encoding="utf-8")

    with patch("scripts.git_safety._candidate_paths", return_value=["scripts/git_safety.py", "skills/gitx/SKILL.md"]):
        result = build_verify_preset_suggestion(tmp_path)

    assert "gitx-smoke" in result.suggested_presets
    assert result.matched_paths_by_preset["gitx-smoke"] == ["scripts/git_safety.py", "skills/gitx/SKILL.md"]
    assert result.preset_details[0].label == "Gitx Smoke"


def test_build_verify_preset_suggestion_includes_rust_routing_and_browser_matches(tmp_path: Path) -> None:
    _init_repo(tmp_path)

    with patch(
        "scripts.git_safety._candidate_paths",
        return_value=[
            "scripts/router-rs/src/main.rs",
            "scripts/evaluate_routing.py",
            "tools/browser-mcp/src/runtime.ts",
        ],
    ):
        result = build_verify_preset_suggestion(tmp_path)

    assert "rust-router" in result.suggested_presets
    assert "routing-eval" in result.suggested_presets
    assert "browser-runtime" in result.suggested_presets
    assert result.matched_paths_by_preset["rust-router"] == ["scripts/router-rs/src/main.rs"]
    assert result.matched_paths_by_preset["routing-eval"] == ["scripts/evaluate_routing.py"]
    assert result.matched_paths_by_preset["browser-runtime"] == ["tools/browser-mcp/src/runtime.ts"]
    assert result.suggested_presets == ["rust-router", "routing-eval", "browser-runtime"]


def test_verification_batch_reports_parallel_results_and_rendering(tmp_path: Path) -> None:
    _init_repo(tmp_path)

    result = run_verification_batch(
        repo_root=tmp_path,
        commands=[
            "python3 -c \"print('ok-a')\"",
            "python3 -c \"print('ok-b')\"",
        ],
        max_parallel=2,
        prefer_rtk=False,
    )
    rendered = render_verification_batch(result)

    assert result.parallel_mode == "parallel"
    assert result.total_commands == 2
    assert result.failed == 0
    assert result.passed == 2
    assert "parallel_mode: parallel" in rendered
    assert "verify-1" in rendered
    assert "verify-2" in rendered
    assert "preset=custom" in rendered


def test_verification_batch_render_includes_merged_from(tmp_path: Path) -> None:
    _init_repo(tmp_path)

    with patch(
        "scripts.git_safety.VERIFY_PRESET_CONFIGS",
        {
            "high": {
                "label": "High",
                "priority": 90,
                "commands": [{"command": "python3 -c \"print('same')\""}],
                "path_rules": (),
            },
            "low": {
                "label": "Low",
                "priority": 10,
                "commands": [{"command": "python3 -c \"print('same')\""}],
                "path_rules": (),
            },
        },
    ):
        result = run_verification_batch(
            repo_root=tmp_path,
            commands=[],
            presets=["low", "high"],
            max_parallel=1,
            prefer_rtk=False,
        )

    rendered = render_verification_batch(result)
    assert result.total_commands == 1
    assert "merged_from=high|low" in rendered


def test_verification_batch_runs_preset_without_explicit_commands(tmp_path: Path) -> None:
    _init_repo(tmp_path)

    with patch(
        "scripts.git_safety.VERIFY_PRESET_CONFIGS",
        {
            "local-pass": {
                "label": "Local Pass",
                "priority": 1,
                "commands": [{"command": "python3 -c \"print('preset-ok')\""}],
                "path_rules": (),
            }
        },
    ):
        result = run_verification_batch(
            repo_root=tmp_path,
            commands=[],
            presets=["local-pass"],
            max_parallel=1,
            prefer_rtk=False,
        )

    assert result.failed == 0
    assert result.passed == 1
    assert result.results[0].command == "python3 -c \"print('preset-ok')\""
    assert result.results[0].preset == "local-pass"


def test_auto_closeout_blocks_when_verification_batch_fails(tmp_path: Path) -> None:
    _init_repo(tmp_path)
    (tmp_path / "README.md").write_text("seed\nauto closeout\n", encoding="utf-8")

    result = run_auto_closeout(
        repo_root=tmp_path,
        target_branch="main",
        verify_commands=[
            "python3 -c \"print('pass')\"",
            "python3 -c \"import sys; sys.exit(3)\"",
        ],
        verify_jobs=2,
        prefer_rtk_for_verification=False,
    )
    rendered = render_auto_closeout(result)

    assert result.blocked is True
    assert result.commit_created is False
    assert result.verification_batch is not None
    assert result.verification_batch.failed == 1
    assert "验证命令失败" in result.warnings[-1]
    assert "verification:" in rendered


def test_auto_closeout_accepts_verify_preset(tmp_path: Path) -> None:
    _init_repo(tmp_path)
    (tmp_path / "README.md").write_text("seed\npreset closeout\n", encoding="utf-8")

    with patch(
        "scripts.git_safety.VERIFY_PRESET_CONFIGS",
        {
            "local-pass": {
                "label": "Local Pass",
                "priority": 1,
                "commands": [{"command": "python3 -c \"print('preset-ok')\""}],
                "path_rules": (),
            }
        },
    ):
        result = run_auto_closeout(
            repo_root=tmp_path,
            target_branch="main",
            verify_commands=[],
            verify_presets=["local-pass"],
            verify_jobs=1,
            prefer_rtk_for_verification=False,
        )

    assert result.blocked is False
    assert result.verification_batch is not None
    assert result.verification_batch.passed == 1


def test_auto_closeout_can_use_suggested_verify_presets(tmp_path: Path) -> None:
    _init_repo(tmp_path)
    (tmp_path / "README.md").write_text("seed\nsuggested closeout\n", encoding="utf-8")

    with patch("scripts.git_safety._candidate_paths", return_value=["scripts/git_safety.py"]):
        with patch(
            "scripts.git_safety.VERIFY_PRESET_CONFIGS",
            {
                "gitx-smoke": {
                    "label": "Gitx Smoke",
                    "priority": 40,
                    "commands": [{"command": "python3 -c \"print('preset-ok')\""}],
                    "path_rules": ("scripts/git_safety.py",),
                }
            },
        ):
            result = run_auto_closeout(
                repo_root=tmp_path,
                target_branch="main",
                verify_commands=[],
                verify_presets=[],
                use_suggested_verify_presets=True,
                verify_jobs=1,
                prefer_rtk_for_verification=False,
            )

    assert result.blocked is False
    assert result.inferred_verify_presets == ["gitx-smoke"]
    assert result.verification_batch is not None
    assert result.verification_batch.passed == 1


def test_verification_batch_uses_preset_workdir(tmp_path: Path) -> None:
    _init_repo(tmp_path)
    workdir = tmp_path / "subdir"
    workdir.mkdir()

    with patch(
        "scripts.git_safety.VERIFY_PRESET_CONFIGS",
        {
            "cwd-pass": {
                "label": "Cwd Pass",
                "priority": 5,
                "commands": [{"command": "python3 -c \"print('cwd-ok')\"", "cwd": "subdir"}],
                "path_rules": (),
            }
        },
    ):
        result = run_verification_batch(
            repo_root=tmp_path,
            commands=[],
            presets=["cwd-pass"],
            max_parallel=1,
            prefer_rtk=False,
        )

    assert result.failed == 0
    assert result.results[0].working_directory == str(workdir.resolve())
