"""Regression coverage for repository-local Git safety helpers."""

from __future__ import annotations

import json
import subprocess
import sys
import tarfile
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.git_safety import collect_repo_snapshot, render_snapshot, start_topic_branch, write_checkpoint


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

    assert snapshot.branch.head_name == "main"
    assert snapshot.changes.tracked_paths == 1
    assert snapshot.changes.worktree_paths == 1
    assert snapshot.changes.index_paths == 0
    assert snapshot.changes.untracked_paths == 1
    assert snapshot.changes.ignored_paths == 1
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
