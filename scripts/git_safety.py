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


@dataclass
class DoctorReport:
    repo_root: str
    head_name: str
    upstream: str | None
    risk_level: str
    findings: list[str]
    next_actions: list[str]
    suggested_topic_branch: str | None
    suggested_target_branch: str
    suggested_push_remote: str


@dataclass
class PublishPlan:
    repo_root: str
    current_branch: str
    target_branch: str
    push_remote: str
    blocked: bool
    warnings: list[str]
    steps: list[str]


@dataclass
class AutoCloseoutResult:
    repo_root: str
    original_branch: str
    working_branch: str
    target_branch: str
    push_remote: str
    blocked: bool
    created_topic_branch: str | None
    checkpoint_dir: str | None
    commit_created: bool
    commit_oid: str | None
    merged_to_target: bool
    pushed: bool
    deleted_topic_branch: bool
    warnings: list[str]
    actions: list[str]


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


def _stale_worktrees(snapshot: RepoSnapshot) -> list[WorktreeEntry]:
    return [entry for entry in snapshot.worktrees if not entry.is_current and not entry.head_matches_current]


def _has_dirty_changes(snapshot: RepoSnapshot) -> bool:
    changes = snapshot.changes
    return bool(changes.tracked_paths or changes.untracked_paths or changes.unmerged_paths)


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
    if snapshot.branch.head_name == "main" and _has_dirty_changes(snapshot):
        findings.append("当前脏改动直接堆在 main 上，建议先做 checkpoint，再按主题切分提交。")
    stale_worktrees = _stale_worktrees(snapshot)
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


def _branch_exists(root: Path, branch_name: str) -> bool:
    existing = _run_git(root, "rev-parse", "--verify", "--quiet", branch_name, check=False)
    return existing.returncode == 0


def _pick_available_topic_branch(root: Path, preferred: str) -> str:
    if not _branch_exists(root, preferred):
        return preferred
    suffix = 2
    while True:
        candidate = f"{preferred}-{suffix}"
        if not _branch_exists(root, candidate):
            return candidate
        suffix += 1


def _branch_upstream(root: Path, branch_name: str) -> str | None:
    proc = _run_git(
        root,
        "for-each-ref",
        "--format=%(upstream:short)",
        f"refs/heads/{branch_name}",
        check=False,
    )
    value = proc.stdout.strip()
    return value or None


def _branch_relation(root: Path, branch_name: str) -> tuple[int, int] | None:
    upstream = _branch_upstream(root, branch_name)
    if not upstream:
        return None
    proc = _run_git(root, "rev-list", "--left-right", "--count", f"{branch_name}...{upstream}", check=False)
    if proc.returncode != 0:
        return None
    parts = proc.stdout.strip().split()
    if len(parts) != 2:
        return None
    ahead, behind = int(parts[0]), int(parts[1])
    return ahead, behind


def _infer_push_remote(snapshot: RepoSnapshot) -> str:
    if snapshot.branch.upstream and "/" in snapshot.branch.upstream:
        return snapshot.branch.upstream.split("/", 1)[0]
    return "origin"


def _infer_target_branch(snapshot: RepoSnapshot) -> str:
    current = snapshot.branch.head_name
    if current in {"main", "master"}:
        return current
    if snapshot.branch.upstream and "/" in snapshot.branch.upstream:
        upstream_branch = snapshot.branch.upstream.split("/", 1)[1]
        if upstream_branch:
            return upstream_branch
    return "main"


def _candidate_paths(root: Path) -> list[str]:
    seen: set[str] = set()
    candidates: list[str] = []
    for args in (
        ("diff", "--name-only"),
        ("diff", "--cached", "--name-only"),
        ("ls-files", "--others", "--exclude-standard"),
    ):
        proc = _run_git(root, *args, check=False)
        for raw_line in proc.stdout.splitlines():
            path = raw_line.strip()
            if not path or path in seen:
                continue
            seen.add(path)
            candidates.append(path)
    return candidates


def _current_head_oid(root: Path) -> str | None:
    proc = _run_git(root, "rev-parse", "HEAD", check=False)
    value = proc.stdout.strip()
    return value or None


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
    if _branch_exists(root, branch_name):
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


def suggest_topic_branch(snapshot: RepoSnapshot) -> str:
    repo_name = _slugify(Path(snapshot.repo_root).name)
    day = datetime.now().astimezone().strftime("%Y%m%d")
    return f"topic/{repo_name}-closeout-{day}"


def build_doctor_report(snapshot: RepoSnapshot) -> DoctorReport:
    branch = snapshot.branch
    changes = snapshot.changes
    stale_worktrees = _stale_worktrees(snapshot)
    dirty_on_main = branch.head_name == "main" and _has_dirty_changes(snapshot)
    diverged = branch.ahead > 0 and branch.behind > 0
    blocked = bool(changes.unmerged_paths or diverged or branch.behind > 0)
    caution = bool(
        dirty_on_main
        or stale_worktrees
        or snapshot.stash_entries
        or (snapshot.hooks_path and snapshot.hooks_path != ".githooks")
    )

    findings: list[str] = []
    next_actions: list[str] = []

    if changes.unmerged_paths:
        findings.append(f"当前有 {changes.unmerged_paths} 个冲突文件，先解冲突，不能直接收口。")
        next_actions.append("先处理冲突并重新 `git add` / `git commit`，再继续 gitx 收口。")
    if diverged:
        findings.append("当前分支与上游已经分叉，不能直接盲推。")
        next_actions.append("先 `git fetch --prune`，再明确决定 rebase 还是 merge。")
    elif branch.behind > 0:
        findings.append(f"当前分支落后上游 {branch.behind} 个提交，推送前要先同步。")
        next_actions.append("先 `git fetch --prune`，处理同步后再提交和推送。")
    if dirty_on_main:
        findings.append("脏改动直接堆在 main 上，风险最高。")
        next_actions.append("先做 checkpoint，再切 topic 分支承接脏改动。")
    if stale_worktrees:
        findings.append(f"有 {len(stale_worktrees)} 个 worktree 不在当前 HEAD，合并前先确认来源。")
        next_actions.append("先检查这些旁路 worktree 是历史残留还是仍在使用。")
    if snapshot.stash_entries:
        findings.append(f"有 {len(snapshot.stash_entries)} 个 stash，说明还有未清算改动面。")
        next_actions.append("先把 stash 当成资产清点，不要一边忘记一边收口。")
    if snapshot.hooks_path and snapshot.hooks_path != ".githooks":
        findings.append(f"hooksPath 现在是 {snapshot.hooks_path}，不是 repo 默认 `.githooks`。")
        next_actions.append("提交前确认 hooks 行为，避免出现“改动被吞”错觉。")
    if branch.upstream is None and branch.head_name not in {"main", "master"}:
        findings.append("当前分支还没有 upstream，首次推送要显式 `-u`。")
        next_actions.append("准备推送时使用 `git push -u <remote> <branch>`。")
    if not findings:
        findings.append("当前仓库没有明显高风险阻塞，可以进入 review -> commit -> merge/push 收口。")
        next_actions.append("先做 scoped review 和最小验证，再按显式分支名提交/推送。")

    risk_level = "high" if blocked or dirty_on_main else ("medium" if caution else "low")
    if dirty_on_main:
        next_actions.append(
            f"推荐直接运行：`python3 scripts/git_safety.py start-topic {suggest_topic_branch(snapshot)}`"
        )

    return DoctorReport(
        repo_root=snapshot.repo_root,
        head_name=branch.head_name,
        upstream=branch.upstream,
        risk_level=risk_level,
        findings=findings,
        next_actions=next_actions,
        suggested_topic_branch=suggest_topic_branch(snapshot) if dirty_on_main else None,
        suggested_target_branch=_infer_target_branch(snapshot),
        suggested_push_remote=_infer_push_remote(snapshot),
    )


def render_doctor_report(report: DoctorReport) -> str:
    lines = [
        f"repo: {report.repo_root}",
        f"branch: {report.head_name}",
        f"upstream: {report.upstream or '(none)'}",
        f"risk: {report.risk_level}",
        "findings:",
    ]
    lines.extend(f"- {item}" for item in report.findings)
    lines.append("next_actions:")
    lines.extend(f"- {item}" for item in report.next_actions)
    if report.suggested_topic_branch:
        lines.append(f"suggested_topic_branch: {report.suggested_topic_branch}")
    lines.append(f"suggested_target_branch: {report.suggested_target_branch}")
    lines.append(f"suggested_push_remote: {report.suggested_push_remote}")
    return "\n".join(lines)


def build_publish_plan(snapshot: RepoSnapshot, *, target_branch: str | None = None) -> PublishPlan:
    branch = snapshot.branch
    current_branch = branch.head_name
    resolved_target = target_branch or _infer_target_branch(snapshot)
    push_remote = _infer_push_remote(snapshot)
    warnings: list[str] = []
    steps: list[str] = []

    if snapshot.changes.unmerged_paths:
        warnings.append("当前存在未解决冲突，发布计划被阻塞。")
        return PublishPlan(
            repo_root=snapshot.repo_root,
            current_branch=current_branch,
            target_branch=resolved_target,
            push_remote=push_remote,
            blocked=True,
            warnings=warnings,
            steps=["先解决冲突，再重新运行 `python3 scripts/git_safety.py publish-plan`。"],
        )

    if branch.ahead > 0 and branch.behind > 0:
        warnings.append("当前分支与上游分叉，发布计划被阻塞。")
        return PublishPlan(
            repo_root=snapshot.repo_root,
            current_branch=current_branch,
            target_branch=resolved_target,
            push_remote=push_remote,
            blocked=True,
            warnings=warnings,
            steps=["先 `git fetch --prune`，再明确处理分叉后重新生成计划。"],
        )

    if branch.behind > 0:
        warnings.append(f"当前分支落后上游 {branch.behind} 个提交。")
        steps.append("git fetch --prune")
        steps.append("# 同步上游后，再继续下面的提交/合并/推送步骤")

    if current_branch == resolved_target and _has_dirty_changes(snapshot):
        if current_branch in {"main", "master"}:
            warnings.append("你现在就在主分支上带着脏改动，建议先切 topic。")
            steps.append("python3 scripts/git_safety.py checkpoint --label gitx-closeout")
            steps.append(f"python3 scripts/git_safety.py start-topic {suggest_topic_branch(snapshot)}")
            current_branch = suggest_topic_branch(snapshot)
        steps.append("git add <scoped-paths>")
        steps.append('git commit -m "chore: close out scoped changes"')
        if current_branch != resolved_target:
            steps.append(f"git switch {resolved_target}")
            steps.append(f"git merge --ff-only {current_branch}")
        steps.append(f"git push {push_remote} {resolved_target}")
    elif current_branch != resolved_target:
        if _has_dirty_changes(snapshot):
            steps.append("git add <scoped-paths>")
            steps.append('git commit -m "chore: close out scoped changes"')
        steps.append(f"git switch {resolved_target}")
        steps.append(f"git merge --ff-only {current_branch}")
        steps.append(f"git push {push_remote} {resolved_target}")
    else:
        if branch.ahead > 0:
            steps.append(f"git push {push_remote} {resolved_target}")
        else:
            warnings.append("当前分支既没有脏改动，也没有待推送提交。")
            steps.append("# 暂时没有需要发布的内容")

    if snapshot.stash_entries:
        warnings.append("仓库里还有 stash，推送后最好再清算，不要长期积压。")
    if _stale_worktrees(snapshot):
        warnings.append("存在旁路 worktree，正式合并前最好再确认没有遗漏来源。")

    return PublishPlan(
        repo_root=snapshot.repo_root,
        current_branch=branch.head_name,
        target_branch=resolved_target,
        push_remote=push_remote,
        blocked=False,
        warnings=warnings,
        steps=steps,
    )


def render_publish_plan(plan: PublishPlan) -> str:
    lines = [
        f"repo: {plan.repo_root}",
        f"current_branch: {plan.current_branch}",
        f"target_branch: {plan.target_branch}",
        f"push_remote: {plan.push_remote}",
        f"blocked: {'yes' if plan.blocked else 'no'}",
    ]
    if plan.warnings:
        lines.append("warnings:")
        lines.extend(f"- {item}" for item in plan.warnings)
    lines.append("steps:")
    lines.extend(f"{index}. {step}" for index, step in enumerate(plan.steps, start=1))
    return "\n".join(lines)


def run_auto_closeout(
    *,
    repo_root: Path | None = None,
    target_branch: str | None = None,
    commit_message: str | None = None,
    push: bool = False,
    delete_topic_branch: bool = False,
) -> AutoCloseoutResult:
    root = discover_repo_root(repo_root)
    snapshot = collect_repo_snapshot(root)
    branch = snapshot.branch
    resolved_target = target_branch or _infer_target_branch(snapshot)
    push_remote = _infer_push_remote(snapshot)
    original_branch = branch.head_name
    working_branch = branch.head_name
    warnings: list[str] = []
    actions: list[str] = []
    checkpoint_dir: str | None = None
    created_topic_branch: str | None = None
    commit_created = False
    commit_oid: str | None = None
    merged_to_target = False
    pushed = False
    deleted_topic = False

    if snapshot.changes.unmerged_paths:
        return AutoCloseoutResult(
            repo_root=str(root),
            original_branch=original_branch,
            working_branch=working_branch,
            target_branch=resolved_target,
            push_remote=push_remote,
            blocked=True,
            created_topic_branch=None,
            checkpoint_dir=None,
            commit_created=False,
            commit_oid=None,
            merged_to_target=False,
            pushed=False,
            deleted_topic_branch=False,
            warnings=["当前存在未解决冲突，自动收口已停止。"],
            actions=["先手动解决冲突，再重新运行 auto-closeout。"],
        )

    if branch.ahead > 0 and branch.behind > 0:
        return AutoCloseoutResult(
            repo_root=str(root),
            original_branch=original_branch,
            working_branch=working_branch,
            target_branch=resolved_target,
            push_remote=push_remote,
            blocked=True,
            created_topic_branch=None,
            checkpoint_dir=None,
            commit_created=False,
            commit_oid=None,
            merged_to_target=False,
            pushed=False,
            deleted_topic_branch=False,
            warnings=["当前分支与上游分叉，自动收口已停止。"],
            actions=["先 `git fetch --prune` 并明确处理分叉，再重新运行 auto-closeout。"],
        )

    if branch.behind > 0:
        return AutoCloseoutResult(
            repo_root=str(root),
            original_branch=original_branch,
            working_branch=working_branch,
            target_branch=resolved_target,
            push_remote=push_remote,
            blocked=True,
            created_topic_branch=None,
            checkpoint_dir=None,
            commit_created=False,
            commit_oid=None,
            merged_to_target=False,
            pushed=False,
            deleted_topic_branch=False,
            warnings=[f"当前分支落后上游 {branch.behind} 个提交，自动收口已停止。"],
            actions=["先同步当前分支，再重新运行 auto-closeout。"],
        )

    if _branch_exists(root, resolved_target):
        target_relation = _branch_relation(root, resolved_target)
        if target_relation and target_relation[1] > 0:
            return AutoCloseoutResult(
                repo_root=str(root),
                original_branch=original_branch,
                working_branch=working_branch,
                target_branch=resolved_target,
                push_remote=push_remote,
                blocked=True,
                created_topic_branch=None,
                checkpoint_dir=None,
                commit_created=False,
                commit_oid=None,
                merged_to_target=False,
                pushed=False,
                deleted_topic_branch=False,
                warnings=[f"目标分支 {resolved_target} 落后上游 {target_relation[1]} 个提交，自动收口已停止。"],
                actions=[f"先同步目标分支 {resolved_target}，再重新运行 auto-closeout。"],
            )

    if working_branch == resolved_target and resolved_target in {"main", "master"} and _has_dirty_changes(snapshot):
        preferred_topic = suggest_topic_branch(snapshot)
        chosen_topic = _pick_available_topic_branch(root, preferred_topic)
        checkpoint, _ = start_topic_branch(chosen_topic, repo_root=root, checkpoint_label="gitx-auto-closeout")
        checkpoint_dir = str(checkpoint)
        created_topic_branch = chosen_topic
        working_branch = chosen_topic
        actions.append(f"checkpoint -> {checkpoint_dir}")
        actions.append(f"switch -> {chosen_topic}")
        snapshot = collect_repo_snapshot(root)

    candidate_paths = _candidate_paths(root)
    if candidate_paths:
        _run_git(root, "add", "-A")
        actions.append(f"stage -> {len(candidate_paths)} paths")
        staged_after_add = [line for line in _run_git(root, "diff", "--cached", "--name-only", check=False).stdout.splitlines() if line.strip()]
        if staged_after_add:
            message = commit_message or "chore: gitx auto closeout"
            _run_git(root, "commit", "-m", message)
            commit_created = True
            commit_oid = _current_head_oid(root)
            actions.append(f"commit -> {message}")
        else:
            warnings.append("发现候选路径，但 `git add -A` 之后没有形成可提交变更。")
    else:
        warnings.append("当前没有未提交路径。")

    if working_branch != resolved_target:
        _run_git(root, "switch", resolved_target)
        actions.append(f"switch -> {resolved_target}")
        _run_git(root, "merge", "--ff-only", working_branch)
        merged_to_target = True
        actions.append(f"merge --ff-only -> {working_branch}")

    final_branch = resolved_target if merged_to_target or working_branch != resolved_target else working_branch
    if push:
        upstream = _branch_upstream(root, final_branch)
        if upstream:
            _run_git(root, "push", push_remote, final_branch)
            actions.append(f"push -> {push_remote}/{final_branch}")
        else:
            _run_git(root, "push", "-u", push_remote, final_branch)
            actions.append(f"push -u -> {push_remote}/{final_branch}")
        pushed = True

    if delete_topic_branch and created_topic_branch and merged_to_target:
        _run_git(root, "branch", "-d", created_topic_branch)
        deleted_topic = True
        actions.append(f"delete branch -> {created_topic_branch}")

    if not candidate_paths and not commit_created and not merged_to_target and not pushed:
        actions.append("nothing to do")

    return AutoCloseoutResult(
        repo_root=str(root),
        original_branch=original_branch,
        working_branch=working_branch,
        target_branch=resolved_target,
        push_remote=push_remote,
        blocked=False,
        created_topic_branch=created_topic_branch,
        checkpoint_dir=checkpoint_dir,
        commit_created=commit_created,
        commit_oid=commit_oid,
        merged_to_target=merged_to_target,
        pushed=pushed,
        deleted_topic_branch=deleted_topic,
        warnings=warnings,
        actions=actions,
    )


def render_auto_closeout(result: AutoCloseoutResult) -> str:
    lines = [
        f"repo: {result.repo_root}",
        f"original_branch: {result.original_branch}",
        f"working_branch: {result.working_branch}",
        f"target_branch: {result.target_branch}",
        f"push_remote: {result.push_remote}",
        f"blocked: {'yes' if result.blocked else 'no'}",
        f"commit_created: {'yes' if result.commit_created else 'no'}",
        f"merged_to_target: {'yes' if result.merged_to_target else 'no'}",
        f"pushed: {'yes' if result.pushed else 'no'}",
    ]
    if result.created_topic_branch:
        lines.append(f"created_topic_branch: {result.created_topic_branch}")
    if result.checkpoint_dir:
        lines.append(f"checkpoint_dir: {result.checkpoint_dir}")
    if result.commit_oid:
        lines.append(f"commit_oid: {result.commit_oid}")
    if result.warnings:
        lines.append("warnings:")
        lines.extend(f"- {item}" for item in result.warnings)
    lines.append("actions:")
    lines.extend(f"- {item}" for item in result.actions)
    return "\n".join(lines)


def _status_command(args: argparse.Namespace) -> int:
    snapshot = collect_repo_snapshot(Path(args.repo_root) if args.repo_root else None)
    if args.json:
        print(json.dumps(asdict(snapshot), ensure_ascii=False, indent=2))
    else:
        print(render_snapshot(snapshot))
    return 0


def _doctor_command(args: argparse.Namespace) -> int:
    snapshot = collect_repo_snapshot(Path(args.repo_root) if args.repo_root else None)
    report = build_doctor_report(snapshot)
    if args.json:
        print(json.dumps(asdict(report), ensure_ascii=False, indent=2))
    else:
        print(render_doctor_report(report))
    return 0


def _publish_plan_command(args: argparse.Namespace) -> int:
    snapshot = collect_repo_snapshot(Path(args.repo_root) if args.repo_root else None)
    plan = build_publish_plan(snapshot, target_branch=args.target_branch)
    if args.json:
        print(json.dumps(asdict(plan), ensure_ascii=False, indent=2))
    else:
        print(render_publish_plan(plan))
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


def _auto_closeout_command(args: argparse.Namespace) -> int:
    result = run_auto_closeout(
        repo_root=Path(args.repo_root) if args.repo_root else None,
        target_branch=args.target_branch,
        commit_message=args.commit_message,
        push=args.push,
        delete_topic_branch=args.delete_topic_branch,
    )
    if args.json:
        print(json.dumps(asdict(result), ensure_ascii=False, indent=2))
    else:
        print(render_auto_closeout(result))
    return 0 if not result.blocked else 2


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Repository-local Git safety helpers.")
    parser.add_argument("--repo-root", help="Optional repository root override.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    status_parser = subparsers.add_parser("status", help="Summarize current repository risk surfaces.")
    status_parser.add_argument("--json", action="store_true", help="Emit machine-readable JSON.")
    status_parser.set_defaults(func=_status_command)

    doctor_parser = subparsers.add_parser(
        "doctor",
        help="Classify Git closeout risk and print the next safe actions.",
    )
    doctor_parser.add_argument("--json", action="store_true", help="Emit machine-readable JSON.")
    doctor_parser.set_defaults(func=_doctor_command)

    publish_plan_parser = subparsers.add_parser(
        "publish-plan",
        help="Render a concrete closeout plan for commit -> merge -> push.",
    )
    publish_plan_parser.add_argument(
        "--target-branch",
        help="Optional target branch for the final fast-forward merge/push. Defaults to inferred mainline.",
    )
    publish_plan_parser.add_argument("--json", action="store_true", help="Emit machine-readable JSON.")
    publish_plan_parser.set_defaults(func=_publish_plan_command)

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

    auto_closeout_parser = subparsers.add_parser(
        "auto-closeout",
        help="Automatically checkpoint, topic-branch, commit, ff-only merge, and optionally push.",
    )
    auto_closeout_parser.add_argument(
        "--target-branch",
        help="Optional final target branch. Defaults to inferred mainline.",
    )
    auto_closeout_parser.add_argument(
        "--commit-message",
        help='Optional commit message. Defaults to "chore: gitx auto closeout".',
    )
    auto_closeout_parser.add_argument(
        "--push",
        action="store_true",
        help="Push the final branch automatically after merge.",
    )
    auto_closeout_parser.add_argument(
        "--delete-topic-branch",
        action="store_true",
        help="Delete the auto-created topic branch after a successful fast-forward merge.",
    )
    auto_closeout_parser.add_argument("--json", action="store_true", help="Emit machine-readable JSON.")
    auto_closeout_parser.set_defaults(func=_auto_closeout_command)
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
