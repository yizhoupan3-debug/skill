from __future__ import annotations

import json
import subprocess
from pathlib import Path

import yaml


PROJECT_ROOT = Path(__file__).resolve().parents[1]
MANIFEST = PROJECT_ROOT / "scripts" / "autoresearch-rs" / "Cargo.toml"
BIN = PROJECT_ROOT / "scripts" / "autoresearch-rs" / "target" / "release" / "autoresearch-rs"


def _bin() -> Path:
    if not BIN.exists():
        subprocess.run(["cargo", "build", "--manifest-path", str(MANIFEST), "--release"], cwd=PROJECT_ROOT, check=True)
    return BIN


def run_ctl(*args: str) -> str:
    result = subprocess.run([str(_bin()), *args], cwd=PROJECT_ROOT, text=True, capture_output=True, check=False)
    if result.returncode != 0:
        raise AssertionError(result.stderr or result.stdout)
    return result.stdout


def load_state(workspace: Path) -> dict:
    return yaml.safe_load((workspace / "research-state.yaml").read_text(encoding="utf-8"))


def test_rust_autoresearch_full_loop(tmp_path: Path) -> None:
    run_ctl("init", "--project", "rust-project", "--question", "Can Rust own autoresearch?", "--dir", str(tmp_path), "--mode", "quick")
    root = tmp_path / "rust-project"
    state = load_state(root)
    assert state["schema_version"] == 3
    assert state["novelty_gate"]["status"] == "pending"

    run_ctl("draft-claims", "--workspace", str(root), "--count", "4")
    state = load_state(root)
    assert len(state["novelty_gate"]["draft_claims"]) == 4
    assert "recommended first search target" in (root / "literature" / "NOVELTY_SEARCH_PLAN.md").read_text(encoding="utf-8")
    assert "research-claim" in (root / "literature" / "EXTERNAL_RESEARCH.md").read_text(encoding="utf-8")

    run_ctl(
        "compare-claim",
        "--workspace",
        str(root),
        "--claim",
        "Rust controller reduces orchestration drift",
        "--axis",
        "workflow",
        "--closest-prior-work",
        "Python controller",
        "--overlap",
        "medium",
        "--difference",
        "Rust is the primary control plane.",
        "--confidence",
        "high",
        "--verdict",
        "defensible",
        "--claim-id",
        "C1",
    )
    run_ctl("set-novelty-gate", "--workspace", str(root), "--status", "passed", "--decision", "Proceed")
    run_ctl(
        "add-hypothesis",
        "--workspace",
        str(root),
        "--claim",
        "Rust controller can run the loop",
        "--prediction",
        "Run and reflection files are materialized",
        "--priority",
        "high",
        "--id",
        "rust-loop",
    )
    run_ctl(
        "record-run",
        "--workspace",
        str(root),
        "--hypothesis-id",
        "rust-loop",
        "--outcome",
        "exploratory",
        "--summary",
        "Rust recorded the run.",
        "--metric-name",
        "files",
        "--metric-value",
        "2",
    )
    run_ctl(
        "reflect",
        "--workspace",
        str(root),
        "--hypothesis-id",
        "rust-loop",
        "--direction",
        "DEEPEN",
        "--reason",
        "The loop is working.",
        "--next-step",
        "Add broader coverage.",
    )
    state = load_state(root)
    assert state["active_hypothesis"] == "rust-loop"
    assert state["hypotheses"][0]["status"] == "active"
    assert (root / "experiments" / "rust-loop" / "run-001.md").is_file()
    assert (root / "experiments" / "rust-loop" / "run-001-reflection.md").is_file()
    assert "latest_direction: DEEPEN" in run_ctl("resume", "--workspace", str(root))
    ledger = (root / "run-ledger.jsonl").read_text(encoding="utf-8").splitlines()
    assert any(json.loads(line)["kind"] == "run.recorded" for line in ledger)


def test_autoresearch_can_record_external_research_from_arxiv(tmp_path: Path) -> None:
    run_ctl(
        "init",
        "--project",
        "external-research",
        "--question",
        "Can retrieval augmented generation improve citation grounded research?",
        "--dir",
        str(tmp_path),
    )
    root = tmp_path / "external-research"
    run_ctl("draft-claims", "--workspace", str(root), "--count", "2")
    run_ctl(
        "research-claim",
        "--workspace",
        str(root),
        "--claim-id",
        "C1",
        "--source",
        "arxiv",
        "--limit",
        "1",
    )
    state = load_state(root)
    assert len(state["external_research"]) == 1
    assert state["external_research"][0]["query"]
    assert state["external_research"][0]["results"]
    assert "Managed External Research" in (root / "literature" / "EXTERNAL_RESEARCH.md").read_text(encoding="utf-8")
