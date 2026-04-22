"""Integration tests for the local background parallel-batch CLI."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]
CLI_PATH = PROJECT_ROOT / "scripts" / "runtime_background_cli.py"


def _run_cli(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(CLI_PATH), *args],
        cwd=PROJECT_ROOT,
        text=True,
        capture_output=True,
        check=True,
    )


def test_runtime_background_cli_enqueue_and_read_group_summary(tmp_path: Path) -> None:
    """The CLI should enqueue a batch, wait for completion, and expose group lookup."""

    data_dir = tmp_path / "runtime-data"
    payload_path = tmp_path / "batch.json"
    payload_path.write_text(
        json.dumps(
            {
                "parallel_group_id": "pgroup-cli",
                "requests": [
                    {
                        "task": "lane-a",
                        "user_id": "tester",
                        "session_id": "cli-session-a",
                        "dry_run": True,
                    },
                    {
                        "task": "lane-b",
                        "user_id": "tester",
                        "session_id": "cli-session-b",
                        "dry_run": True,
                    },
                ],
            },
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )

    enqueue = _run_cli(
        "--codex-home",
        str(PROJECT_ROOT),
        "--data-dir",
        str(data_dir),
        "enqueue-batch",
        "--input-file",
        str(payload_path),
        "--timeout-seconds",
        "30",
    )
    enqueue_payload = json.loads(enqueue.stdout)
    assert enqueue_payload["command"] == "enqueue-batch"
    assert enqueue_payload["parallel_group_id"] == "pgroup-cli"
    assert [status["lane_id"] for status in enqueue_payload["statuses"]] == ["lane-1", "lane-2"]
    assert enqueue_payload["summary"]["parallel_group_id"] == "pgroup-cli"
    assert enqueue_payload["summary"]["terminal_job_count"] == 2
    assert enqueue_payload["summary"]["status_counts"]["completed"] == 2

    summary = _run_cli(
        "--codex-home",
        str(PROJECT_ROOT),
        "--data-dir",
        str(data_dir),
        "group-summary",
        "--parallel-group-id",
        "pgroup-cli",
    )
    summary_payload = json.loads(summary.stdout)
    assert summary_payload["command"] == "group-summary"
    assert summary_payload["summary"]["parallel_group_id"] == "pgroup-cli"
    assert sorted(summary_payload["summary"]["session_ids"]) == ["cli-session-a", "cli-session-b"]


def test_runtime_background_cli_lists_persisted_parallel_groups(tmp_path: Path) -> None:
    """The CLI should list persisted groups after batch completion."""

    data_dir = tmp_path / "runtime-data"
    payload_path = tmp_path / "batch.json"
    payload_path.write_text(
        json.dumps(
            {
                "parallel_group_id": "pgroup-list",
                "requests": [
                    {"task": "lane-a", "user_id": "tester", "session_id": "list-a", "dry_run": True},
                    {"task": "lane-b", "user_id": "tester", "session_id": "list-b", "dry_run": True},
                ],
            },
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )

    _run_cli(
        "--codex-home",
        str(PROJECT_ROOT),
        "--data-dir",
        str(data_dir),
        "enqueue-batch",
        "--input-file",
        str(payload_path),
        "--timeout-seconds",
        "30",
    )

    listed = _run_cli(
        "--codex-home",
        str(PROJECT_ROOT),
        "--data-dir",
        str(data_dir),
        "list-groups",
    )
    listed_payload = json.loads(listed.stdout)
    assert listed_payload["command"] == "list-groups"
    assert len(listed_payload["parallel_groups"]) == 1
    assert listed_payload["parallel_groups"][0]["parallel_group_id"] == "pgroup-list"
