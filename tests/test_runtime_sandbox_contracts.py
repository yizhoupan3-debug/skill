"""Sandbox lifecycle and policy contract checks.

These tests freeze the contract documented in
``docs/runtime_sandbox_contract.md`` so the sandbox lane can evolve without
silently drifting from the control-plane specification.
"""

from __future__ import annotations

import json
import re
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = PROJECT_ROOT / "docs" / "runtime_sandbox_contract.md"


def _load_contract_schema() -> dict[str, object]:
    text = CONTRACT_PATH.read_text(encoding="utf-8")
    match = re.search(r"```json sandbox-contract-v1\n(.*?)\n```", text, re.DOTALL)
    if match is None:
        raise AssertionError("sandbox contract schema block is missing")
    return json.loads(match.group(1))


def test_runtime_sandbox_contract_schema_freezes_control_plane_semantics() -> None:
    """The documented sandbox contract must keep the frozen control-plane vocabulary."""

    schema = _load_contract_schema()

    assert schema["schema_version"] == "runtime-sandbox-contract-v1"
    assert schema["lifecycle_states"] == [
        "created",
        "warm",
        "busy",
        "draining",
        "recycled",
        "failed",
    ]
    assert schema["allowed_transitions"] == [
        ["created", "warm"],
        ["warm", "busy"],
        ["busy", "draining"],
        ["draining", "recycled"],
        ["draining", "failed"],
        ["warm", "failed"],
        ["busy", "failed"],
        ["recycled", "warm"],
    ]
    assert schema["tool_capability_categories"] == [
        "read_only",
        "workspace_mutating",
        "networked",
        "high_risk",
    ]
    assert schema["resource_budgets"] == ["cpu", "memory", "wall_clock", "output_size"]
    assert schema["recoverability_boundary"] == {
        "recoverable": [
            "transient timeout",
            "transient kill request",
            "cleanup retry after a failed async cleanup attempt",
            "takeover after control-plane interruption when policy-compliant",
        ],
        "non_recoverable": [
            "repeated cleanup failure",
            "policy violation that invalidates the sandbox profile",
            "contamination of sandbox-local state that cannot be deterministically cleared",
            "any state where reuse would require privilege expansion or hidden host repair",
        ],
    }


def test_runtime_sandbox_contract_text_mentions_required_policy_boundaries() -> None:
    """The human-facing contract should still spell out the required sandbox semantics."""

    text = CONTRACT_PATH.read_text(encoding="utf-8").lower()

    for phrase in [
        "async cleanup",
        "failure isolation",
        "recoverability boundary",
        "deny-by-default",
        "high-risk tools must use a dedicated sandbox profile",
        "budgets are part of the contract",
    ]:
        assert phrase in text
