"""Regression tests for the Rust contract total ledger narrative."""

from __future__ import annotations

from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]
CONTRACT_DOC = PROJECT_ROOT / "docs" / "rust_contracts.md"


def _doc_text() -> str:
    return CONTRACT_DOC.read_text(encoding="utf-8")


def test_rust_contracts_doc_keeps_the_three_part_status_ledger() -> None:
    text = _doc_text()

    for heading in [
        "## Current Status Ledger",
        "### 已实现",
        "### 已退休",
        "### 下一 safe slice",
    ]:
        assert heading in text


def test_rust_contracts_doc_no_longer_uses_stale_transition_wording() -> None:
    text = _doc_text()

    for stale_phrase in [
        "escape hatch",
        "not live yet",
        "implementation remains pending",
        "hidden behind an escape hatch",
    ]:
        assert stale_phrase not in text


def test_rust_contracts_doc_records_current_minimal_implementation_truth() -> None:
    text = _doc_text()

    for required_phrase in [
        "compatibility live fallback is retired with explicit requests rejected",
        "contract-backed minimal implementation already",
        "live in the host runtime",
        "compaction is a gated minimal lane on supported backends",
        "compatibility live fallback runtime path is retired",
    ]:
        assert required_phrase in text
