"""Contract tests for runtime compaction design artifacts."""

from __future__ import annotations

from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]
CONTRACT_DOC = PROJECT_ROOT / "docs" / "runtime_compaction_contract.md"

SNAPSHOT_REQUIRED_FIELDS = [
    "schema_version",
    "generation",
    "snapshot_id",
    "parent_generation",
    "parent_snapshot_id",
    "session_id",
    "job_id",
    "created_at",
    "watermark_event_id",
    "state_digest",
    "artifact_index_ref",
    "state_ref",
    "delta_cursor",
    "summary",
]

DELTA_REQUIRED_FIELDS = [
    "schema_version",
    "generation",
    "delta_id",
    "parent_snapshot_id",
    "seq",
    "ts",
    "kind",
    "payload",
    "artifact_refs",
    "applies_to",
]

ARTIFACT_REF_REQUIRED_FIELDS = [
    "artifact_id",
    "kind",
    "uri",
    "digest",
    "size_bytes",
    "schema_version",
    "created_at",
    "producer",
]

GENERATION_INHERITANCE_RULES = [
    "new generation inherits only the minimal necessary state",
    "session identity",
    "job identity",
    "old generation must remain readable for audit and recovery",
    "one rollover produces exactly one successor generation",
    "generation numbers must be monotonic",
]


def _doc_text() -> str:
    return CONTRACT_DOC.read_text(encoding="utf-8")


def test_compaction_contract_freezes_required_sections() -> None:
    """The design doc should keep the compaction contract headings stable."""

    text = _doc_text()
    assert "# Runtime Compaction Contract" in text
    for heading in [
        "## Contract 1: Snapshot Schema",
        "## Contract 2: Delta Replay Contract",
        "## Contract 3: Generation Rollover Policy",
        "## Contract 4: Artifact Ref Strategy",
        "## Contract 5: Consistency Invariants",
    ]:
        assert heading in text


def test_compaction_contract_snapshot_and_delta_fields_are_explicit() -> None:
    """Snapshot and delta schemas should keep their required fields frozen."""

    text = _doc_text()
    for field in SNAPSHOT_REQUIRED_FIELDS:
        assert f"`{field}:" in text or f"`{field}`" in text
    for field in DELTA_REQUIRED_FIELDS:
        assert f"`{field}:" in text or f"`{field}`" in text
    for field in ARTIFACT_REF_REQUIRED_FIELDS:
        assert f"`{field}:" in text or f"`{field}`" in text


def test_compaction_contract_generation_rules_cover_inheritance_and_recovery() -> None:
    """Generation rollover should preserve minimal state and a recoverable chain."""

    text = _doc_text()
    for rule in GENERATION_INHERITANCE_RULES:
        assert rule in text
    assert "parent_snapshot_id" in text
    assert "latest stable snapshot" in text
    assert "artifact refs" in text
    assert "must not require scanning the full historical stream" in text


def test_compaction_contract_consistency_rules_are_non_negotiable() -> None:
    """Compaction must preserve replay determinism and fail closed on bad refs."""

    text = _doc_text()
    assert "replay must be deterministic" in text
    assert "idempotent" in text
    assert "fail closed" in text
    assert "cross-generation mutable aliasing" in text
    assert "state_digest" in text
