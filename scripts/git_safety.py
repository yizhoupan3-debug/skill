#!/usr/bin/env python3
"""Repository-local Git safety helpers for high-churn worktrees."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import tarfile
from dataclasses import asdict, dataclass
from datetime import datetime
from pathlib import Path
from typing import Any


@dataclass
class BranchStatus:
    head_oid: str
    head_name: str
    upstream: str | None
    ahead: int
    behind: int


@dataclass
class ChangeCounts:
    tracked_paths: int
    index_paths: int
    worktree_paths: int
    deleted_paths: int
    unmerged_paths: int
    untracked_paths: int
    ignored_paths: int


@dataclass
class WorktreeEntry:
    path: str
    head_oid: str
    branch: str | None
    is_current: bool
    head_matches_current: bool
    locked_reason: str | None


@dataclass
class RepoSnapshot:
    repo_root: str
    captured_at: str
    branch: BranchStatus
    changes: ChangeCounts
    hooks_path: str | None
    stash_entries: list[str]
    worktrees: list[WorktreeEntry]
    status_porcelain: str
    worktree_listing: str
    reflog_head: str


def _run_git(root: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=root,
        text=True,
        capture_output=True,
        check=check,
    )


def discover_repo_root(start: Path | None = None) -> Path:
    base = (start or Path.cwd()).resolve()
    proc = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        cwd=base,
        text=True,
        capture_output=True,
        check=False,
    )
    if proc.returncode != 0:
        raise SystemExit(proc.stderr.strip() or "not inside a git repository")
    return Path(proc.stdout.strip()).resolve()


def _parse_status_porcelain(output: str) -> tuple[BranchStatus, ChangeCounts]:
    head_oid = ""
    head_name = "(unknown)"
    upstream: str | None = None
    ahead = 0
    behind = 0
    tracked_paths = 0
    index_paths = 0
    worktree_paths = 0
    deleted_paths = 0
    unmerged_paths = 0
    untracked_paths = 0
    ignored_paths = 0

    for raw_line in output.splitlines():
        if raw_line.startswith("# branch.oid "):
            head_oid = raw_line.removeprefix("# branch.oid ").strip()
            continue
        if raw_line.startswith("# branch.head "):
            head_name = raw_line.removeprefix("# branch.head ").strip()
            continue
        if raw_line.startswith("# branch.upstream "):
            upstream = raw_line.removeprefix("# branch.upstream ").strip()
            continue
        if raw_line.startswith("# branch.ab "):
            match = re.match(r"# branch\.ab \+(\d+) \-(\d+)", raw_line)
            if match:
                ahead = int(match.group(1))
                behind = int(match.group(2))
            continue

        kind = raw_line[:1]
        if kind == "1":
            tracked_paths += 1
            fields = raw_line.split(" ", 8)
            xy = fields[1]
            if xy[0] != ".":
                index_paths += 1
            if xy[1] != ".":
                worktree_paths += 1
            if "D" in xy:
                deleted_paths += 1
            continue
        if kind == "2":
            tracked_paths += 1
            fields = raw_line.split(" ", 9)
            xy = fields[1]
            if xy[0] != ".":
                index_paths += 1
            if xy[1] != ".":
                worktree_paths += 1
            if "D" in xy:
                deleted_paths += 1
            continue
        if kind == "u":
            tracked_paths += 1
            unmerged_paths += 1
            continue
        if kind == "?":
            untracked_paths += 1
            continue
        if kind == "!":
            ignored_paths += 1
            continue

    return (
        BranchStatus(
            head_oid=head_oid,
            head_name=head_name,
            upstream=upstream,
            ahead=ahead,
            behind=behind,
        ),
        ChangeCounts(
            tracked_paths=tracked_paths,
            index_paths=index_paths,
            worktree_paths=worktree_paths,
            deleted_paths=deleted_paths,
            unmerged_paths=unmerged_paths,
            untracked_paths=untracked_paths,
            ignored_paths=ignored_paths,
        ),
    )


def _parse_worktree_listing(output: str, current_head_oid: str, repo_root: Path) -> list[WorktreeEntry]:
    entries: list[WorktreeEntry] = []
    current: dict[str, str] = {}

    for raw_line in output.splitlines():
        line = raw_line.strip()
        if not line:
            if current:
                entries.append(_worktree_entry_from_payload(current, current_head_oid, repo_root))
                current = {}
            continue
        key, _, value = line.partition(" ")
        current[key] = value

    if current:
        entries.append(_worktree_entry_from_payload(current, current_head_oid, repo_root))
    return entries


def _worktree_entry_from_payload(payload: dict[str, str], current_head_oid: str, repo_root: Path) -> WorktreeEntry:
    path = Path(payload["worktree"]).resolve()
    branch = payload.get("branch")
    if branch and branch.startswith("refs/heads/"):
        branch = branch.removeprefix("refs/heads/")
    head_oid = payload.get("HEAD", "")
    locked_reason = payload.get("locked") or None
    return WorktreeEntry(
        path=str(path),
        head_oid=head_oid,
        branch=branch,
        is_current=path == repo_root,
        head_matches_current=(head_oid == current_head_oid),
        locked_reason=locked_reason,
    )


def collect_repo_snapshot(repo_root: Path | None = None) -> RepoSnapshot:
    root = discover_repo_root(repo_root)
    status_porcelain = _run_git(
        root,
        "status",
        "--porcelain=v2",
        "--branch",
        "--untracked-files=all",
        "--ignored=matching",
    ).stdout
    branch, changes = _parse_status_porcelain(status_porcelain)
    worktree_listing = _run_git(root, "worktree", "list", "--porcelain").stdout
    worktrees = _parse_worktree_listing(worktree_listing, branch.head_oid, root)
    stash_proc = _run_git(root, "stash", "list", check=False)
    stash_entries = [line for line in stash_proc.stdout.splitlines() if line.strip()]
    hooks_proc = _run_git(root, "config", "--get", "core.hooksPath", check=False)
    hooks_path = hooks_proc.stdout.strip() or None
    reflog_head = _run_git(root, "reflog", "-n", "20", "--date=iso").stdout
    return RepoSnapshot(
        repo_root=str(root),
        captured_at=datetime.now().astimezone().isoformat(timespec="seconds"),
        branch=branch,
        changes=changes,
        hooks_path=hooks_path,
        stash_entries=stash_entries,
        worktrees=worktrees,
        status_porcelain=status_porcelain,
        worktree_listing=worktree_listing,
        reflog_head=reflog_head,
    )


def render_snapshot(snapshot: RepoSnapshot) -> str:
    branch = snapshot.branch
    changes = snapshot.changes
    lines = [
        f"repo: {snapshot.repo_root}",
        f"head: {branch.head_oid[:7]} ({branch.head_name})",
        f"upstream: {branch.upstream or '(none)'} [ahead {branch.ahead}, behind {branch.behind}]",
        (
            "changes: "
            f"tracked {changes.tracked_paths}, "
            f"index {changes.index_paths}, "
            f"worktree {changes.worktree_paths}, "
            f"deleted {changes.deleted_paths}, "
            f"unmerged {changes.unmerged_paths}, "
            f"untracked {changes.untracked_paths}, "
            f"ignored {changes.ignored_paths}"
        ),
        f"hooks: {snapshot.hooks_path or '(default .git/hooks)'}",
        f"stash: {len(snapshot.stash_entries)}",
        f"worktrees: {len(snapshot.worktrees)}",
    ]

    findings: list[str] = []
    if branch.head_name == "main" and (
        changes.tracked_paths or changes.untracked_paths or changes.unmerged_paths
    ):
        findings.append("当前脏改动直接堆在 main 上，建议先做 checkpoint，再按主题切分提交。")
    stale_worktrees = [entry for entry in snapshot.worktrees if not entry.is_current and not entry.head_matches_current]
    if stale_worktrees:
        findings.append(f"有 {len(stale_worktrees)} 个旁路 worktree 不在当前 HEAD，合并或同步前先确认来源。")
    if snapshot.stash_entries:
        findings.append(f"有 {len(snapshot.stash_entries)} 个 stash，需要把它们视为待清算资产，不要长期积压。")
    if snapshot.hooks_path and snapshot.hooks_path != ".githooks":
        findings.append(f"当前 hooksPath 是 {snapshot.hooks_path}，不是 repo 维护文档里的 .githooks。")

    if findings:
        lines.append("findings:")
        lines.extend(f"- {item}" for item in findings)
    return "\n".join(lines)


def _slugify(value: str) -> str:
    cleaned = re.sub(r"[^A-Za-z0-9._-]+", "-", value.strip()).strip("-")
    return cleaned or "snapshot"


def _write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def _list_untracked_paths(root: Path) -> list[Path]:
    proc = _run_git(root, "ls-files", "--others", "--exclude-standard", "-z")
    items = [entry for entry in proc.stdout.split("\0") if entry]
    return [root / item for item in items]


def write_checkpoint(
    snapshot: RepoSnapshot,
    *,
    label: str | None = None,
    output_dir: Path | None = None,
) -> Path:
    root = Path(snapshot.repo_root)
    timestamp = datetime.now().astimezone().strftime("%Y%m%dT%H%M%S%z")
    slug = _slugify(label or snapshot.branch.head_name)
    checkpoint_dir = (output_dir or (root / "artifacts" / "ops" / "git_safety")) / f"{timestamp}_{slug}"
    checkpoint_dir.mkdir(parents=True, exist_ok=True)

    metadata = {
        "repo_root": snapshot.repo_root,
        "captured_at": snapshot.captured_at,
        "branch": asdict(snapshot.branch),
        "changes": asdict(snapshot.changes),
        "hooks_path": snapshot.hooks_path,
        "stash_entries": snapshot.stash_entries,
        "worktrees": [asdict(entry) for entry in snapshot.worktrees],
    }
    _write_text(checkpoint_dir / "metadata.json", json.dumps(metadata, ensure_ascii=False, indent=2) + "\n")
    _write_text(checkpoint_dir / "status.porcelain-v2", snapshot.status_porcelain)
    _write_text(checkpoint_dir / "worktrees.porcelain", snapshot.worktree_listing)
    _write_text(checkpoint_dir / "reflog.txt", snapshot.reflog_head)
    _write_text(checkpoint_dir / "stash.txt", "\n".join(snapshot.stash_entries) + ("\n" if snapshot.stash_entries else ""))

    tracked_patch = _run_git(root, "diff", "--binary", "--full-index").stdout
    staged_patch = _run_git(root, "diff", "--cached", "--binary", "--full-index").stdout
    _write_text(checkpoint_dir / "tracked.patch", tracked_patch)
    _write_text(checkpoint_dir / "staged.patch", staged_patch)

    untracked_paths = _list_untracked_paths(root)
    _write_text(
        checkpoint_dir / "untracked_files.txt",
        "".join(f"{path.relative_to(root).as_posix()}\n" for path in untracked_paths),
    )
    if untracked_paths:
        with tarfile.open(checkpoint_dir / "untracked.tar.gz", "w:gz") as archive:
            for path in untracked_paths:
                if path.exists():
                    archive.add(path, arcname=path.relative_to(root).as_posix())

    restore_doc = "\n".join(
        [
            "# Git Safety Restore",
            "",
            f"- repo_root: {root}",
            f"- checkpoint_dir: {checkpoint_dir}",
            "",
            "## Recovery order",
            "",
            "1. Review metadata.json, reflog.txt, and stash.txt first.",
            "2. Restore staged intent: `git apply --index staged.patch`",
            "3. Restore unstaged tracked edits: `git apply tracked.patch`",
            "4. Restore untracked files: `tar -xzf untracked.tar.gz -C .`",
            "",
            "Run the apply and tar commands from the repository root.",
        ]
    )
    _write_text(checkpoint_dir / "RESTORE.md", restore_doc + "\n")
    return checkpoint_dir


def start_topic_branch(
    branch_name: str,
    *,
    repo_root: Path | None = None,
    checkpoint_label: str | None = None,
) -> tuple[Path, RepoSnapshot]:
    root = discover_repo_root(repo_root)
    snapshot = collect_repo_snapshot(root)
    if snapshot.branch.head_name == branch_name:
        raise SystemExit(f"already on branch: {branch_name}")
    existing = _run_git(root, "rev-parse", "--verify", "--quiet", branch_name, check=False)
    if existing.returncode == 0:
        raise SystemExit(f"branch already exists: {branch_name}")

    label = checkpoint_label or f"switch-{branch_name}"
    checkpoint_dir = write_checkpoint(snapshot, label=label)
    switch_result = _run_git(root, "switch", "-c", branch_name, check=False)
    if switch_result.returncode != 0:
        raise SystemExit(
            switch_result.stderr.strip()
            or switch_result.stdout.strip()
            or f"failed to switch to new branch: {branch_name}"
        )
    return checkpoint_dir, snapshot


def _status_command(args: argparse.Namespace) -> int:
    snapshot = collect_repo_snapshot(Path(args.repo_root) if args.repo_root else None)
    if args.json:
        print(json.dumps(asdict(snapshot), ensure_ascii=False, indent=2))
    else:
        print(render_snapshot(snapshot))
    return 0


def _checkpoint_command(args: argparse.Namespace) -> int:
    snapshot = collect_repo_snapshot(Path(args.repo_root) if args.repo_root else None)
    checkpoint_dir = write_checkpoint(
        snapshot,
        label=args.label,
        output_dir=Path(args.output_dir).resolve() if args.output_dir else None,
    )
    payload: dict[str, Any] = {
        "checkpoint_dir": str(checkpoint_dir),
        "branch": snapshot.branch.head_name,
        "head_oid": snapshot.branch.head_oid,
        "untracked_paths": snapshot.changes.untracked_paths,
        "stash_entries": len(snapshot.stash_entries),
    }
    if args.json:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    else:
        print(f"checkpoint: {checkpoint_dir}")
        print(render_snapshot(snapshot))
    return 0


def _start_topic_command(args: argparse.Namespace) -> int:
    checkpoint_dir, snapshot = start_topic_branch(
        args.branch_name,
        repo_root=Path(args.repo_root) if args.repo_root else None,
        checkpoint_label=args.checkpoint_label,
    )
    payload: dict[str, Any] = {
        "checkpoint_dir": str(checkpoint_dir),
        "source_branch": snapshot.branch.head_name,
        "target_branch": args.branch_name,
        "head_oid": snapshot.branch.head_oid,
    }
    if args.json:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    else:
        print(f"checkpoint: {checkpoint_dir}")
        print(f"switched: {snapshot.branch.head_name} -> {args.branch_name}")
        print("dirty worktree was preserved on the new branch.")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Repository-local Git safety helpers.")
    parser.add_argument("--repo-root", help="Optional repository root override.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    status_parser = subparsers.add_parser("status", help="Summarize current repository risk surfaces.")
    status_parser.add_argument("--json", action="store_true", help="Emit machine-readable JSON.")
    status_parser.set_defaults(func=_status_command)

    checkpoint_parser = subparsers.add_parser(
        "checkpoint",
        help="Write a non-destructive recovery bundle for current tracked and untracked work.",
    )
    checkpoint_parser.add_argument("--label", help="Optional label for the checkpoint directory.")
    checkpoint_parser.add_argument("--output-dir", help="Optional directory for checkpoint bundles.")
    checkpoint_parser.add_argument("--json", action="store_true", help="Emit machine-readable JSON.")
    checkpoint_parser.set_defaults(func=_checkpoint_command)

    start_topic_parser = subparsers.add_parser(
        "start-topic",
        help="Checkpoint current work and switch dirty changes onto a new topic branch.",
    )
    start_topic_parser.add_argument("branch_name", help="New branch name to create from the current HEAD.")
    start_topic_parser.add_argument(
        "--checkpoint-label",
        help="Optional checkpoint label override. Defaults to switch-<branch>.",
    )
    start_topic_parser.add_argument("--json", action="store_true", help="Emit machine-readable JSON.")
    start_topic_parser.set_defaults(func=_start_topic_command)
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
