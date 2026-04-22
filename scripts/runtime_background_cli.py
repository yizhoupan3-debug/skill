#!/usr/bin/env python3
"""Thin local CLI for durable background parallel-batch control."""

from __future__ import annotations

import argparse
import asyncio
import json
import sys
from pathlib import Path

if __package__ in {None, ""}:
    PROJECT_ROOT = Path(__file__).resolve().parents[1]
    RUNTIME_SRC = PROJECT_ROOT / "codex_agno_runtime" / "src"
    if str(PROJECT_ROOT) not in sys.path:
        sys.path.insert(0, str(PROJECT_ROOT))
    if str(RUNTIME_SRC) not in sys.path:
        sys.path.insert(0, str(RUNTIME_SRC))
else:
    PROJECT_ROOT = Path(__file__).resolve().parents[1]

from pydantic import BaseModel, Field

from codex_agno_runtime.config import RuntimeSettings
from codex_agno_runtime.runtime import CodexAgnoRuntime
from codex_agno_runtime.schemas import BackgroundRunRequest


class BackgroundBatchCliPayload(BaseModel):
    """CLI payload for one bounded parallel background batch."""

    requests: list[BackgroundRunRequest] = Field(default_factory=list)
    parallel_group_id: str | None = None
    lane_id_prefix: str = "lane"


def _load_json_payload(*, input_json: str | None, input_file: Path | None) -> dict[str, object]:
    if input_json is not None:
        return json.loads(input_json)
    if input_file is None:
        raise ValueError("One of --input-json or --input-file is required.")
    return json.loads(input_file.read_text(encoding="utf-8"))


def _build_runtime(args: argparse.Namespace) -> CodexAgnoRuntime:
    settings = RuntimeSettings(
        codex_home=args.codex_home,
        data_dir=args.data_dir,
        trace_output_path=args.trace_output_path,
    )
    return CodexAgnoRuntime(settings)


async def wait_for_parallel_group_terminal(
    runtime: CodexAgnoRuntime,
    parallel_group_id: str,
    *,
    timeout_seconds: float,
    poll_interval_seconds: float = 0.05,
) -> dict[str, object]:
    """Wait until one parallel group reaches a fully terminal state."""

    deadline = asyncio.get_running_loop().time() + timeout_seconds
    last_summary = runtime.get_background_parallel_group_summary(parallel_group_id)
    while asyncio.get_running_loop().time() < deadline:
        summary = runtime.get_background_parallel_group_summary(parallel_group_id)
        if summary is not None:
            last_summary = summary
            if summary.total_job_count > 0 and summary.terminal_job_count == summary.total_job_count:
                return summary.model_dump(mode="json")
        await asyncio.sleep(poll_interval_seconds)
    raise TimeoutError(
        f"Timed out waiting for parallel group {parallel_group_id!r} to reach a terminal state."
    )


async def enqueue_background_batch_command(
    runtime: CodexAgnoRuntime,
    payload: BackgroundBatchCliPayload,
    *,
    timeout_seconds: float,
) -> dict[str, object]:
    """Enqueue one batch and wait until the whole group reaches terminal state."""

    batch = await runtime.enqueue_background_batch(
        payload.requests,
        parallel_group_id=payload.parallel_group_id,
        lane_id_prefix=payload.lane_id_prefix,
    )
    summary = await wait_for_parallel_group_terminal(
        runtime,
        batch.parallel_group_id,
        timeout_seconds=timeout_seconds,
    )
    return {
        "command": "enqueue-batch",
        "parallel_group_id": batch.parallel_group_id,
        "statuses": [status.model_dump(mode="json") for status in batch.statuses],
        "summary": summary,
    }


def get_parallel_group_summary_command(runtime: CodexAgnoRuntime, parallel_group_id: str) -> dict[str, object]:
    """Return one persisted parallel-group summary."""

    summary = runtime.get_background_parallel_group_summary(parallel_group_id)
    if summary is None:
        raise KeyError(f"Parallel group {parallel_group_id!r} was not found.")
    return {
        "command": "group-summary",
        "parallel_group_id": parallel_group_id,
        "summary": summary.model_dump(mode="json"),
    }


def list_parallel_groups_command(runtime: CodexAgnoRuntime) -> dict[str, object]:
    """Return all persisted parallel-group summaries."""

    summaries = runtime.list_background_parallel_groups()
    return {
        "command": "list-groups",
        "parallel_groups": [summary.model_dump(mode="json") for summary in summaries],
    }


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description=(
            "Expose the runtime's durable background parallel-batch controls through "
            "a thin local CLI."
        )
    )
    parser.add_argument(
        "--codex-home",
        type=Path,
        default=PROJECT_ROOT,
        help="Runtime codex_home. Defaults to the current repo root.",
    )
    parser.add_argument(
        "--data-dir",
        type=Path,
        default=Path("data"),
        help="Runtime data_dir passed into RuntimeSettings.",
    )
    parser.add_argument(
        "--trace-output-path",
        type=Path,
        default=None,
        help="Optional TRACE_METADATA.json path for runtime trace artifacts.",
    )

    subparsers = parser.add_subparsers(dest="command", required=True)

    enqueue_parser = subparsers.add_parser(
        "enqueue-batch",
        help="Admit one bounded background batch and wait for the whole group to finish.",
    )
    enqueue_inputs = enqueue_parser.add_mutually_exclusive_group(required=True)
    enqueue_inputs.add_argument(
        "--input-json",
        help="Inline JSON payload with {requests, parallel_group_id?, lane_id_prefix?}.",
    )
    enqueue_inputs.add_argument(
        "--input-file",
        type=Path,
        help="JSON file with {requests, parallel_group_id?, lane_id_prefix?}.",
    )
    enqueue_parser.add_argument(
        "--timeout-seconds",
        type=float,
        default=60.0,
        help="Maximum time to wait for the whole batch to reach a terminal state.",
    )

    summary_parser = subparsers.add_parser(
        "group-summary",
        help="Read one durable parallel-group summary by id.",
    )
    summary_parser.add_argument("--parallel-group-id", required=True)

    subparsers.add_parser(
        "list-groups",
        help="List all durable parallel-group summaries in the configured data dir.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    runtime = _build_runtime(args)
    runtime.startup()
    try:
        if args.command == "enqueue-batch":
            payload = BackgroundBatchCliPayload.model_validate(
                _load_json_payload(input_json=args.input_json, input_file=args.input_file)
            )
            result = asyncio.run(
                enqueue_background_batch_command(
                    runtime,
                    payload,
                    timeout_seconds=args.timeout_seconds,
                )
            )
        elif args.command == "group-summary":
            result = get_parallel_group_summary_command(runtime, args.parallel_group_id)
        else:
            result = list_parallel_groups_command(runtime)
    except Exception as exc:
        print(str(exc), file=sys.stderr)
        return 1
    finally:
        runtime.shutdown()

    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
