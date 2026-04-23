#!/usr/bin/env python3
"""Build the machine-readable framework surface policy."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
LOADOUTS_PATH = ROOT / "skills" / "SKILL_LOADOUTS.json"
TIERS_PATH = ROOT / "skills" / "SKILL_TIERS.json"
OUTPUT_PATH = ROOT / "configs" / "framework" / "FRAMEWORK_SURFACE_POLICY.json"

KERNEL_AXES = ["routing", "memory", "continuity", "host_projection"]
SOURCE_ROOTS = [
    "framework_runtime/src/",
    "scripts/",
    "skills/",
    "docs/",
    "tests/",
    "tools/",
    "configs/",
]
COMPILED_OUTPUT_ROOTS = [
    "target/",
    "rust_tools/target/",
    "scripts/**/target/",
    "tools/**/dist/",
    "tools/**/output/",
]
GENERATED_ROOTS = [
    "skills/SKILL_*.json",
    "skills/SKILL_ROUTING_*.md",
    "AGENT.md",
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
]
SESSION_ARTIFACT_ROOTS = [
    "SESSION_SUMMARY.md",
    "NEXT_ACTIONS.json",
    "EVIDENCE_INDEX.json",
    "TRACE_METADATA.json",
    ".supervisor_state.json",
    "artifacts/current/",
    "artifacts/bootstrap/",
    "artifacts/ops/",
    "artifacts/evidence/",
    "artifacts/scratch/",
]
OUTCOME_METRICS = [
    {
        "id": "first_attempt_success_rate",
        "label": "第一次成功率",
        "definition": "在默认面内、不借兼容回退也不补人工热修的情况下，一次执行直接完成任务的比例。",
    },
    {
        "id": "cross_host_consistency",
        "label": "跨宿主一致性",
        "definition": "同一 framework truth 在 Codex CLI、Claude Code、Gemini CLI 等宿主上的 contract 和行为一致度。",
    },
    {
        "id": "checkpoint_resume_success_rate",
        "label": "断点恢复成功率",
        "definition": "依靠 continuity artifacts 和 resume binding 恢复任务时，能否稳定接回同一 task story 的比例。",
    },
    {
        "id": "new_task_onboarding_cost",
        "label": "新任务接入成本",
        "definition": "把一个新任务接入默认工作流所需的显式配置、额外说明和定制 loadout 成本。",
    },
]


def load_json(path: Path) -> dict[str, Any]:
    """Load a JSON file from disk."""

    return json.loads(path.read_text(encoding="utf-8"))


def build_framework_surface_policy(
    loadouts: dict[str, Any],
    tiers: dict[str, Any],
) -> dict[str, Any]:
    """Build the framework surface policy payload."""

    activation_policy = loadouts.get("activation_policy", {})
    default_loadouts = [
        str(name)
        for name in activation_policy.get("default_loadouts", [])
        if isinstance(name, str) and name
    ]
    if not default_loadouts:
        default_loadouts = sorted(
            str(name)
            for name, config in loadouts.get("loadouts", {}).items()
            if isinstance(config, dict) and str(config.get("activation", "explicit")) == "default"
        )
    explicit_loadouts = [
        str(name)
        for name in activation_policy.get("explicit_opt_in_loadouts", [])
        if isinstance(name, str) and name
    ]
    if not explicit_loadouts:
        explicit_loadouts = sorted(
            str(name)
            for name in loadouts.get("loadouts", {})
            if name not in set(default_loadouts)
        )

    default_loadout_name = default_loadouts[0] if default_loadouts else None
    default_surface = loadouts.get("loadouts", {}).get(default_loadout_name or "", {})
    summary = tiers.get("summary", {})

    return {
        "version": 1,
        "schema_version": "framework-surface-policy-v1",
        "kernel": {
            "canonical_axes": KERNEL_AXES,
            "policy": "Keep only routing, memory, continuity, and host projection on the mainline; everything else is an opt-in capability.",
        },
        "migration_guardrails": {
            "preserve_rust_runtime_authority": True,
            "avoid_runtime_kernel_fork": True,
            "compatibility_surfaces_explicit_only": True,
        },
        "default_surface": {
            "default_loadouts": default_loadouts,
            "explicit_opt_in_loadouts": explicit_loadouts,
            "default_entry_loadout": default_loadout_name,
            "lean_default_owners": list(default_surface.get("owners", [])),
            "default_overlays": list(default_surface.get("overlays", [])),
            "tier_activation_defaults": tiers.get("surface_policy", {}).get("tier_activation_defaults", {}),
        },
        "skill_system": {
            "tier_catalog_path": "skills/SKILL_TIERS.json",
            "loadout_catalog_path": "skills/SKILL_LOADOUTS.json",
            "tier_counts": summary.get("tier_counts", {}),
            "activation_counts": summary.get("activation_counts", {}),
        },
        "physical_boundaries": {
            "source_roots": SOURCE_ROOTS,
            "compiled_output_roots": COMPILED_OUTPUT_ROOTS,
            "generated_roots": GENERATED_ROOTS,
            "session_artifact_roots": SESSION_ARTIFACT_ROOTS,
            "rules": [
                "Do not mix compiled outputs or scratch runs back into source roots.",
                "Generated routing and host projection artifacts remain replaceable outputs, not authoring sources of truth.",
                "Session continuity stays under root mirrors plus artifacts/current and must not drift into random repo folders.",
            ],
        },
        "outcome_metrics": OUTCOME_METRICS,
    }


def write_policy(path: Path, payload: dict[str, Any]) -> None:
    """Write the policy payload using stable formatting."""

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def main() -> int:
    """CLI entry point."""

    parser = argparse.ArgumentParser(description="Build the framework surface policy.")
    parser.add_argument("--loadouts", type=Path, default=LOADOUTS_PATH, help="Skill loadouts JSON path.")
    parser.add_argument("--tiers", type=Path, default=TIERS_PATH, help="Skill tiers JSON path.")
    parser.add_argument("--output", type=Path, default=OUTPUT_PATH, help="Surface policy output path.")
    parser.add_argument("--apply", action="store_true", help="Write the policy file to disk.")
    args = parser.parse_args()

    payload = build_framework_surface_policy(
        load_json(args.loadouts),
        load_json(args.tiers),
    )
    if args.apply:
        write_policy(args.output, payload)
        print(f"Wrote {args.output}")
    else:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
