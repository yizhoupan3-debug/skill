from __future__ import annotations

import subprocess
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.paper_lane_scaffold import LaneSpec, parse_lane_spec, scaffold_parallel_batch


def test_parse_lane_spec_requires_four_fields() -> None:
    spec = parse_lane_spec("fig_a|figure_audit|figure:F1-F3|paper-visuals")

    assert spec == LaneSpec(
        lane_id="fig_a",
        lane_kind="figure_audit",
        lane_scope="figure:F1-F3",
        lane_owner="paper-visuals",
    )


def test_scaffold_parallel_batch_writes_manifest_and_lane_files(tmp_path: Path) -> None:
    outputs = scaffold_parallel_batch(
        workspace_root=tmp_path,
        review_dir="paper_review_v3",
        batch_id="g11_figures_a",
        main_gate="G11",
        batch_goal="Audit final-scale figures before gate closeout.",
        frozen_inputs=["G2 passed", "G3 selected_claim_level locked"],
        lanes=[
            LaneSpec("fig_a", "figure_audit", "figure:F1-F2", "paper-visuals"),
            LaneSpec("fig_b", "figure_audit", "figure:F3-F4", "paper-visuals"),
        ],
    )

    manifest_path = Path(outputs["manifest_path"])
    assert manifest_path.exists()
    manifest = manifest_path.read_text(encoding="utf-8")
    assert "# Parallel Lane Batch: G11" in manifest
    assert "## Lane Table" in manifest
    assert "| fig_a | figure_audit | figure:F1-F2 | paper-visuals | queued | fig_a/lane.md | - |" in manifest
    assert "| fig_b | figure_audit | figure:F3-F4 | paper-visuals | queued | fig_b/lane.md | - |" in manifest

    lane_a = manifest_path.parent / "fig_a" / "lane.md"
    lane_b = manifest_path.parent / "fig_b" / "lane.md"
    assert lane_a.exists()
    assert lane_b.exists()
    assert "- figure:F1-F2" in lane_a.read_text(encoding="utf-8")
    assert "- figure:F3-F4" in lane_b.read_text(encoding="utf-8")


def test_scaffold_parallel_batch_cli(tmp_path: Path) -> None:
    script = PROJECT_ROOT / "scripts" / "paper_lane_scaffold.py"
    result = subprocess.run(
        [
            "python3",
            str(script),
            "--workspace",
            str(tmp_path),
            "--review-dir",
            "paper_review_v4",
            "--batch-id",
            "g05_refs_a",
            "--main-gate",
            "G5",
            "--batch-goal",
            "Verify claim-to-citation support for the active gate.",
            "--frozen-input",
            "G3 passed",
            "--lane",
            "refs_a|citation_verify|citation_cluster:C1-C4|citation-management",
            "--lane",
            "refs_b|citation_verify|citation_cluster:C5-C8|citation-management",
        ],
        cwd=PROJECT_ROOT,
        capture_output=True,
        text=True,
        check=True,
    )

    assert "manifest:" in result.stdout
    manifest_path = tmp_path / "paper_review_v4" / "lanes" / "g05_refs_a" / "lane_manifest.md"
    assert manifest_path.exists()
    assert (manifest_path.parent / "refs_a" / "lane.md").exists()
    assert (manifest_path.parent / "refs_b" / "lane.md").exists()
