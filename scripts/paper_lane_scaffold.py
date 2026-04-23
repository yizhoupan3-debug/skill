#!/usr/bin/env python3
"""Scaffold bounded parallel paper-review lanes under one active main gate."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path


DEFAULT_MERGE_BACK_RULE = (
    "Main thread adjudicates lane outputs locally. Lane results may supply evidence "
    "or local edits, but may not freeze the gate or redefine frozen inputs."
)
DEFAULT_STOP_CONDITION = (
    "Stop when every merge-critical lane is marked merged, blocked with an explicit "
    "reason, or dropped as no longer relevant."
)


@dataclass(frozen=True)
class LaneSpec:
    lane_id: str
    lane_kind: str
    lane_scope: str
    lane_owner: str


@dataclass(frozen=True)
class BatchPreset:
    batch_id: str
    batch_goal: str
    lanes: list[LaneSpec]


GATE_PRESETS: dict[str, BatchPreset] = {
    "G5": BatchPreset(
        batch_id="g05_refs_a",
        batch_goal="Verify claim-to-citation support for the active gate.",
        lanes=[
            LaneSpec("refs_a", "citation_verify", "citation_cluster:C1-C4", "citation-management"),
            LaneSpec("refs_b", "citation_verify", "citation_cluster:C5-C8", "citation-management"),
        ],
    ),
    "G9": BatchPreset(
        batch_id="g09_mirror_a",
        batch_goal="Check mirrored claim surfaces for drift after the active revision batch.",
        lanes=[
            LaneSpec("mirror_front", "mirror_cleanup", "front_door_text:abstract+intro", "paper-writing"),
            LaneSpec("mirror_back", "mirror_cleanup", "conclusion+captions+rebuttal", "paper-writing"),
        ],
    ),
    "G10": BatchPreset(
        batch_id="g10_notation_a",
        batch_goal="Audit notation, symbols, and abbreviations before gate closeout.",
        lanes=[
            LaneSpec("notation_a", "notation_audit", "notation_set:N1-N3", "paper-notation-audit"),
            LaneSpec("notation_b", "notation_audit", "notation_set:N4-N6", "paper-notation-audit"),
        ],
    ),
    "G11": BatchPreset(
        batch_id="g11_figures_a",
        batch_goal="Audit final-scale figures before gate closeout.",
        lanes=[
            LaneSpec("fig_a", "figure_audit", "figure:F1-F2", "paper-visuals"),
            LaneSpec("fig_b", "figure_audit", "figure:F3-F4", "paper-visuals"),
        ],
    ),
    "G12": BatchPreset(
        batch_id="g12_tables_a",
        batch_goal="Audit final-scale tables before gate closeout.",
        lanes=[
            LaneSpec("tbl_a", "table_audit", "table:T1-T2", "paper-visuals"),
            LaneSpec("tbl_b", "table_audit", "table:T3-T4", "paper-visuals"),
        ],
    ),
    "G13": BatchPreset(
        batch_id="g13_prose_a",
        batch_goal="Polish local prose surfaces after claim boundaries are frozen.",
        lanes=[
            LaneSpec("prose_front", "prose_local", "abstract+intro", "paper-writing"),
            LaneSpec("prose_back", "prose_local", "discussion+conclusion", "paper-writing"),
        ],
    ),
    "G14": BatchPreset(
        batch_id="g14_layout_a",
        batch_goal="Audit float placement and page economy before layout closeout.",
        lanes=[
            LaneSpec("layout_a", "layout_audit", "layout_surface:P1-P4", "pdf"),
            LaneSpec("layout_b", "layout_audit", "layout_surface:P5-P8", "pdf"),
        ],
    ),
}


def normalize_gate_id(main_gate: str) -> str:
    return main_gate.strip().upper()


def preset_for_gate(main_gate: str) -> BatchPreset | None:
    return GATE_PRESETS.get(normalize_gate_id(main_gate))


def resolve_batch_plan(
    *,
    main_gate: str,
    batch_id: str | None,
    batch_goal: str | None,
    lanes: list[LaneSpec],
    use_preset: bool,
) -> tuple[str, str, list[LaneSpec]]:
    """Resolve one batch plan from explicit lanes and optional gate presets."""

    preset = preset_for_gate(main_gate) if use_preset else None
    resolved_lanes = lanes or (preset.lanes if preset is not None else [])
    resolved_batch_id = batch_id or (preset.batch_id if preset is not None else None)
    resolved_batch_goal = batch_goal or (preset.batch_goal if preset is not None else None)

    if not resolved_lanes:
        raise ValueError(
            "No lanes resolved. Provide --lane entries or use --preset-by-gate for a supported gate."
        )
    if not resolved_batch_id:
        raise ValueError(
            "No batch id resolved. Provide --batch-id or use --preset-by-gate for a supported gate."
        )
    if not resolved_batch_goal:
        raise ValueError(
            "No batch goal resolved. Provide --batch-goal or use --preset-by-gate for a supported gate."
        )
    return resolved_batch_id, resolved_batch_goal, resolved_lanes


def parse_lane_spec(raw: str) -> LaneSpec:
    """Parse one lane spec from a compact CLI form.

    Format:
        lane_id|lane_kind|lane_scope|lane_owner
    """

    parts = [part.strip() for part in raw.split("|")]
    if len(parts) != 4 or any(not part for part in parts):
        raise ValueError(
            "Lane spec must use 'lane_id|lane_kind|lane_scope|lane_owner', "
            f"got: {raw!r}"
        )
    return LaneSpec(
        lane_id=parts[0],
        lane_kind=parts[1],
        lane_scope=parts[2],
        lane_owner=parts[3],
    )


def _render_lane_manifest(
    *,
    main_gate: str,
    batch_goal: str,
    frozen_inputs: list[str],
    lanes: list[LaneSpec],
    merge_back_rule: str,
    stop_condition: str,
) -> str:
    frozen_lines = "\n".join(f"- {item}" for item in frozen_inputs) if frozen_inputs else "- None declared yet"
    lane_rows = [
        "| lane_id | lane_kind | lane_scope | lane_owner | status | output_artifact | blocked_by |",
        "|---|---|---|---|---|---|---|",
    ]
    for lane in lanes:
        lane_rows.append(
            "| {lane_id} | {lane_kind} | {lane_scope} | {lane_owner} | queued | "
            "{lane_id}/lane.md | - |".format(
                lane_id=lane.lane_id,
                lane_kind=lane.lane_kind,
                lane_scope=lane.lane_scope,
                lane_owner=lane.lane_owner,
            )
        )
    lane_table = "\n".join(lane_rows)
    return (
        f"# Parallel Lane Batch: {main_gate}\n\n"
        "## Main Gate\n\n"
        f"- {main_gate}\n\n"
        "## Batch Goal\n\n"
        f"- {batch_goal}\n\n"
        "## Frozen Inputs\n\n"
        f"{frozen_lines}\n\n"
        "## Lane Table\n\n"
        f"{lane_table}\n\n"
        "## Merge Back Rule\n\n"
        f"- {merge_back_rule}\n\n"
        "## Stop Condition\n\n"
        f"- {stop_condition}\n"
    )


def _render_lane_note(*, main_gate: str, lane: LaneSpec) -> str:
    return (
        f"# Lane: {lane.lane_id}\n\n"
        "## Main Gate\n\n"
        f"- {main_gate}\n\n"
        "## Lane Kind\n\n"
        f"- {lane.lane_kind}\n\n"
        "## Lane Scope\n\n"
        f"- {lane.lane_scope}\n\n"
        "## Owner\n\n"
        f"- {lane.lane_owner}\n\n"
        "## Status\n\n"
        "- queued\n\n"
        "## Output Contract\n\n"
        "- Write only lane-local findings or local edit proposals here.\n"
        "- Do not freeze the main gate from this file.\n"
        "- Escalate contradictions back to the main thread for merge-back.\n"
    )


def scaffold_parallel_batch(
    *,
    workspace_root: Path,
    review_dir: str,
    batch_id: str,
    main_gate: str,
    batch_goal: str,
    lanes: list[LaneSpec],
    frozen_inputs: list[str] | None = None,
    merge_back_rule: str = DEFAULT_MERGE_BACK_RULE,
    stop_condition: str = DEFAULT_STOP_CONDITION,
    force: bool = False,
) -> dict[str, str]:
    """Create one bounded sidecar batch for the active paper gate."""

    if not lanes:
        raise ValueError("At least one lane is required.")

    review_root = workspace_root / review_dir
    batch_root = review_root / "lanes" / batch_id
    manifest_path = batch_root / "lane_manifest.md"

    if manifest_path.exists() and not force:
        raise FileExistsError(
            f"Lane manifest already exists: {manifest_path}. Pass --force to rewrite."
        )

    batch_root.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(
        _render_lane_manifest(
            main_gate=main_gate,
            batch_goal=batch_goal,
            frozen_inputs=frozen_inputs or [],
            lanes=lanes,
            merge_back_rule=merge_back_rule,
            stop_condition=stop_condition,
        ),
        encoding="utf-8",
    )

    outputs = {"manifest_path": str(manifest_path)}
    for lane in lanes:
        lane_dir = batch_root / lane.lane_id
        lane_dir.mkdir(parents=True, exist_ok=True)
        lane_note = lane_dir / "lane.md"
        if lane_note.exists() and not force:
            raise FileExistsError(
                f"Lane note already exists: {lane_note}. Pass --force to rewrite."
            )
        lane_note.write_text(
            _render_lane_note(main_gate=main_gate, lane=lane),
            encoding="utf-8",
        )
        outputs[f"lane:{lane.lane_id}"] = str(lane_note)

    return outputs


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Scaffold one bounded parallel lane batch for a paper review workflow."
    )
    parser.add_argument("--workspace", type=Path, required=True, help="Manuscript workspace root.")
    parser.add_argument(
        "--review-dir",
        required=True,
        help="Review directory name, for example paper_review_v3.",
    )
    parser.add_argument(
        "--batch-id",
        help="Parallel batch id, for example g11_figures_a. Optional when --preset-by-gate is used.",
    )
    parser.add_argument("--main-gate", required=True, help="Active main gate, for example G11.")
    parser.add_argument(
        "--batch-goal",
        help="One-sentence goal for this batch. Optional when --preset-by-gate is used.",
    )
    parser.add_argument(
        "--frozen-input",
        action="append",
        default=[],
        help="Repeatable frozen input line.",
    )
    parser.add_argument(
        "--lane",
        action="append",
        default=[],
        help="Repeatable lane spec: lane_id|lane_kind|lane_scope|lane_owner",
    )
    parser.add_argument(
        "--preset-by-gate",
        action="store_true",
        help="Auto-fill batch id, goal, and default lanes for supported gates.",
    )
    parser.add_argument(
        "--merge-back-rule",
        default=DEFAULT_MERGE_BACK_RULE,
        help="Optional merge-back rule override.",
    )
    parser.add_argument(
        "--stop-condition",
        default=DEFAULT_STOP_CONDITION,
        help="Optional stop condition override.",
    )
    parser.add_argument("--force", action="store_true", help="Rewrite manifest and lane notes if they exist.")
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    lanes = [parse_lane_spec(raw) for raw in args.lane]
    batch_id, batch_goal, resolved_lanes = resolve_batch_plan(
        main_gate=args.main_gate,
        batch_id=args.batch_id,
        batch_goal=args.batch_goal,
        lanes=lanes,
        use_preset=args.preset_by_gate,
    )
    outputs = scaffold_parallel_batch(
        workspace_root=args.workspace,
        review_dir=args.review_dir,
        batch_id=batch_id,
        main_gate=args.main_gate,
        batch_goal=batch_goal,
        lanes=resolved_lanes,
        frozen_inputs=args.frozen_input,
        merge_back_rule=args.merge_back_rule,
        stop_condition=args.stop_condition,
        force=args.force,
    )
    print(f"manifest: {outputs['manifest_path']}")
    for key, value in outputs.items():
        if key.startswith("lane:"):
            print(f"{key}: {value}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
