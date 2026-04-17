"""Deterministic fixtures for memory and compression contracts."""

from __future__ import annotations

import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "codex_agno_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from codex_agno_runtime.context import ContextEngineer
from codex_agno_runtime.memory import MEMORY_PROVENANCE_KIND, MEMORY_STORE_SCHEMA_VERSION, FactMemoryStore


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

    snapshot = store.contract_snapshot(fixture["user_id"])
    assert snapshot["schema_version"] == MEMORY_STORE_SCHEMA_VERSION
    assert len(snapshot["facts"]) == len(fixture["expected_merged"])


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
