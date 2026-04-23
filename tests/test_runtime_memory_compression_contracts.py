"""Deterministic fixtures for memory and compression contracts."""

from __future__ import annotations

import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "framework_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from framework_runtime.context import ContextEngineer
from framework_runtime.memory import (
    MEMORY_PROVENANCE_KIND,
    MEMORY_STORE_SCHEMA_VERSION,
    DeterministicFactMemoryKernel,
    FactMemoryStore,
    USER_FACT_PATTERNS,
)


FIXTURES = json.loads((PROJECT_ROOT / "tests" / "runtime_memory_compression_fixtures.json").read_text(encoding="utf-8"))


def test_memory_contract_fixture(tmp_path: Path) -> None:
    """Fact memory should honor storage, dedupe, ranking, and provenance semantics."""

    fixture = FIXTURES["memory"]
    store = FactMemoryStore(tmp_path / "memory")

    extracted = store.extract_facts_sync(fixture["conversation"])
    assert extracted == fixture["expected_extracted"]

    store.save_facts(fixture["user_id"], extracted)
    store.save_facts(fixture["user_id"], fixture["new_facts"])

    assert store.load_facts(fixture["user_id"]) == fixture["expected_merged"]

    retrieval = store.retrieve_facts(fixture["user_id"], limit=2)
    assert [row["value"] for row in retrieval] == fixture["expected_retrieval_limit_2"]
    assert [row["rank"] for row in retrieval] == [1, 2]
    assert all(row["provenance"]["kind"] == MEMORY_PROVENANCE_KIND for row in retrieval)
    assert all(row["provenance"]["control_plane_authority"] == "rust-runtime-control-plane" for row in retrieval)
    assert all(row["provenance"]["control_plane_projection"] == "rust-native-projection" for row in retrieval)

    snapshot = store.contract_snapshot(fixture["user_id"])
    assert snapshot["schema_version"] == MEMORY_STORE_SCHEMA_VERSION
    assert snapshot["control_plane"]["authority"] == "rust-runtime-control-plane"
    assert snapshot["control_plane"]["projection"] == "rust-native-projection"
    assert len(snapshot["facts"]) == len(fixture["expected_merged"])

    persisted = json.loads(Path(snapshot["storage_path"]).read_text(encoding="utf-8"))
    assert persisted["schema_version"] == MEMORY_STORE_SCHEMA_VERSION
    assert persisted["control_plane"]["delegate_kind"] == "fact-memory-store"


def test_memory_kernel_keeps_first_insertion_and_applies_limit_after_ranking(tmp_path: Path) -> None:
    """Deterministic memory mechanics should stay stable for later Rust replacement."""

    store = FactMemoryStore(tmp_path / "memory")
    store.save_facts(
        "kernel-user",
        [" Alpha ", "beta", "ALPHA", "", "Beta  ", "Gamma", "gamma rays", "GAMMA RAYS"],
    )

    assert store.load_facts("kernel-user") == ["Alpha", "beta", "Gamma", "gamma rays"]

    retrieval = store.retrieve_facts("kernel-user", limit=3)
    assert [row["value"] for row in retrieval] == ["Alpha", "beta", "Gamma"]
    assert [row["rank"] for row in retrieval] == [1, 2, 3]


def test_memory_kernel_extracts_with_deterministic_normalization() -> None:
    """Extraction should normalize whitespace and dedupe repeated matches deterministically."""

    kernel = DeterministicFactMemoryKernel(patterns=tuple(USER_FACT_PATTERNS))

    extracted = kernel.extract_facts(
        "My name is Ada   Lovelace. I prefer Rust. I prefer rust. I work at OpenAI."
    )

    assert extracted == ["Ada Lovelace", "OpenAI", "Rust"]


def test_compression_contract_fixture() -> None:
    """Context compression should match the frozen deterministic fixture."""

    fixture = FIXTURES["compression"]
    expected = fixture["expected"]
    result = ContextEngineer().compress_contract(fixture["prompt"], fixture["token_limit"])

    assert result.schema_version == expected["schema_version"]
    assert result.strategy == expected["strategy"]
    assert result.omitted_sections == expected["omitted_sections"]
    assert result.truncated is expected["truncated"]
    assert result.artifact_offload_decision is expected["artifact_offload_decision"]
    assert result.output_token_estimate == expected["output_token_estimate"]
    assert result.prompt == expected["prompt"]


def test_compression_identity_when_prompt_already_fits() -> None:
    """Prompts within budget must remain byte-identical."""

    prompt = "Short prompt with enough budget."
    result = ContextEngineer().compress_contract(prompt, token_limit=128)

    assert result.strategy == "none"
    assert result.truncated is False
    assert result.omitted_sections == 0
    assert result.prompt == prompt
    assert result.input_token_estimate == result.output_token_estimate


def test_compression_zero_budget_returns_deterministic_omission_marker() -> None:
    """Zero-token budgets should never produce ad-hoc truncation text."""

    result = ContextEngineer().compress_contract("Section 1\n\nSection 2", token_limit=0)

    assert result.truncated is True
    assert result.prompt == "[Context compression]\nPrompt omitted due to zero token budget."
    assert result.strategy == "truncate"


def test_compression_prefers_dedupe_before_head_tail() -> None:
    """Repeated sections should be removed before structured compression chooses survivors."""

    repeated = "How to reply:\n- Lead with the answer or result.\n- Keep the default reply short."
    prompt = "\n\n".join([
        repeated,
        "Section A",
        repeated,
        "Section B",
        repeated,
        "Section C",
    ])

    result = ContextEngineer().compress_contract(prompt, token_limit=20)

    assert result.strategy.startswith("dedupe+")
    assert result.prompt.count("How to reply:") == 1
