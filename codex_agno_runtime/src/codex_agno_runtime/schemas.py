"""Typed runtime schemas for the Codex Agno runtime."""

from __future__ import annotations

from datetime import UTC, datetime
from typing import Any

from pydantic import AliasChoices, BaseModel, Field


def _now_iso() -> str:
    """Return a canonical UTC ISO timestamp."""

    return datetime.now(UTC).isoformat()


class SkillMetadata(BaseModel):
    """Normalized skill metadata loaded from `SKILL.md`."""

    name: str
    description: str = ""
    short_description: str = ""
    when_to_use: str = ""
    do_not_use: str = ""
    routing_layer: str = "L3"
    routing_owner: str = "owner"
    routing_gate: str = "none"
    routing_priority: str = "P2"
    session_start: str = "n/a"
    framework_roles: list[str] = Field(default_factory=list)
    tags: list[str] = Field(default_factory=list)
    trigger_hints: list[str] = Field(
        default_factory=list,
        validation_alias=AliasChoices("trigger_hints", "trigger_phrases"),
        serialization_alias="trigger_hints",
    )
    metadata: dict[str, Any] = Field(default_factory=dict)
    health: float = 100.0
    body: str = ""
    body_loaded: bool = True
    source_path: str | None = None

    @property
    def trigger_phrases(self) -> list[str]:
        """Backward-compatible alias for legacy callers during the migration window."""

        return self.trigger_hints


class ScoredSkill(BaseModel):
    """Scored routing candidate."""

    skill: SkillMetadata
    score: float
    reasons: list[str] = Field(default_factory=list)


class RoutingResult(BaseModel):
    """Final routing decision."""

    task: str
    session_id: str
    selected_skill: SkillMetadata
    overlay_skill: SkillMetadata | None = None
    score: float = 0.0
    layer: str
    reasons: list[str] = Field(default_factory=list)
    route_snapshot: "RouteDecisionSnapshot | None" = None
    prompt_preview: str | None = None
    route_engine: str = "rust"
    rollback_to_python: bool = False
    shadow_route_report: "RouteDiffReport | None" = None


class RouteDecisionSnapshot(BaseModel):
    """Stable route snapshot used by shadow/verify comparisons."""

    engine: str
    selected_skill: str
    overlay_skill: str | None = None
    layer: str
    score: float = 0.0
    score_bucket: str
    reasons: list[str] = Field(default_factory=list)
    reasons_class: str = "none"


class RouteExecutionPolicy(BaseModel):
    """Stable route-mode policy owned by the Rust routing core.

    `python_route_required` only covers the explicit legacy primary-authority lane.
    `diagnostic_python_lane` marks compare-only shadow/verify/rollback lanes.
    """

    policy_schema_version: str
    authority: str
    mode: str
    rollback_active: bool = False
    python_route_required: bool = False
    diagnostic_python_lane: bool = False
    primary_authority: str
    route_result_engine: str
    shadow_engine: str | None = None
    diff_report_required: bool = False
    verify_parity_required: bool = False


class RouteDiffReport(BaseModel):
    """Stable parity and soak payload shared by shadow/verify/rust modes."""

    report_schema_version: str
    authority: str
    mode: str
    primary_engine: str
    shadow_engine: str | None = None
    mismatch: bool = False
    mismatch_fields: list[str] = Field(default_factory=list)
    selected_skill_match: bool = True
    overlay_skill_match: bool = True
    layer_match: bool = True
    score_bucket_match: bool = True
    reasons_class_match: bool = True
    rollback_active: bool = False
    python: RouteDecisionSnapshot
    rust: RouteDecisionSnapshot


class FrameworkSessionContract(BaseModel):
    """Host-neutral session contract derived from framework truth."""

    mode: str = "default"
    approval_mode: str = "inherit"
    history_policy: str = "host-managed"
    takeover: bool = False
    extras: dict[str, Any] = Field(default_factory=dict)


class FrameworkSharedContractSurface(BaseModel):
    """Shared outer-contract surface owned by the framework profile."""

    artifact_contract: dict[str, Any] = Field(default_factory=dict)
    memory_mounts: list[dict[str, Any]] = Field(default_factory=list)
    mcp_servers: list[dict[str, Any]] = Field(default_factory=list)
    tool_policy: dict[str, Any] = Field(default_factory=dict)
    approval_policy: dict[str, Any] = Field(default_factory=dict)
    loadout_policy: dict[str, Any] = Field(default_factory=dict)
    workspace_bootstrap: dict[str, Any] = Field(default_factory=dict)
    session_contract: FrameworkSessionContract = Field(default_factory=FrameworkSessionContract)


class FrameworkSharedContract(BaseModel):
    """Canonical host-neutral shared contract for common adapters."""

    schema_version: str
    authority: str
    framework_truth: str = "framework_core"
    profile_id: str
    framework_profile_version: str
    shared_contract_fields: list[str] = Field(default_factory=list)
    shared_contract: FrameworkSharedContractSurface = Field(
        default_factory=FrameworkSharedContractSurface
    )


class FrameworkSharedContractProjection(BaseModel):
    """One adapter projection compared against the canonical shared contract."""

    adapter_id: str
    projection_field: str
    shared_contract_match: bool = True
    shared_contract_mismatch_fields: list[str] = Field(default_factory=list)
    projected_contract: FrameworkSharedContractSurface = Field(
        default_factory=FrameworkSharedContractSurface
    )
    runtime_surface_match: bool | None = None
    runtime_surface_mismatch_fields: list[str] = Field(default_factory=list)
    runtime_surface: FrameworkSharedContractSurface | None = None


class FrameworkSharedContractProjectionReport(BaseModel):
    """Projection parity report for Desktop/CLI-family adapters."""

    schema_version: str
    authority: str
    profile_id: str
    framework_profile_version: str
    shared_contract_schema_version: str
    projection_fields: list[str] = Field(default_factory=list)
    canonical_shared_contract: FrameworkSharedContractSurface = Field(
        default_factory=FrameworkSharedContractSurface
    )
    adapter_projections: list[FrameworkSharedContractProjection] = Field(default_factory=list)
    all_shared_contract_projections_match: bool = True


class PrepareSessionRequest(BaseModel):
    """Session preparation request."""

    task: str
    project_id: str | None = None
    session_id: str | None = None
    user_id: str | None = None
    allow_overlay: bool = True


class PrepareSessionResponse(BaseModel):
    """Prepared session metadata returned before execution."""

    session_id: str
    user_id: str
    skill: str
    overlay: str | None = None
    layer: str
    reasons: list[str] = Field(default_factory=list)
    prompt_preview: str | None = None
    loaded_skill_count: int = 0
    route_engine: str = "rust"
    rollback_to_python: bool = False
    shadow_route_report: RouteDiffReport | None = None


class UsageMetrics(BaseModel):
    """Normalized token usage metrics."""

    input_tokens: int = 0
    output_tokens: int = 0
    total_tokens: int = 0
    mode: str = "estimated"


class RunTaskRequest(BaseModel):
    """Task execution request."""

    task: str
    project_id: str | None = None
    session_id: str | None = None
    user_id: str | None = None
    allow_overlay: bool = True
    dry_run: bool = False


class RunTaskResponse(BaseModel):
    """Task execution response."""

    session_id: str
    user_id: str
    skill: str
    overlay: str | None = None
    live_run: bool
    content: str = ""
    usage: UsageMetrics = Field(default_factory=UsageMetrics)
    prompt_preview: str | None = None
    model_id: str | None = None
    metadata: dict[str, Any] = Field(default_factory=dict)


class BackgroundRunRequest(RunTaskRequest):
    """Background task execution request."""

    multitask_strategy: str = "reject"
    max_attempts: int = 1
    backoff_base_seconds: float = 0.0
    backoff_multiplier: float = 2.0
    max_backoff_seconds: float | None = None


class BackgroundRunStatus(BaseModel):
    """Background job lifecycle state."""

    job_id: str
    session_id: str | None = None
    status: str
    multitask_strategy: str = "reject"
    result: RunTaskResponse | None = None
    error: str | None = None
    created_at: str = Field(default_factory=_now_iso)
    updated_at: str = Field(default_factory=_now_iso)
    attempt: int = 1
    retry_count: int = 0
    max_attempts: int = 1
    timeout_seconds: float | None = None
    claimed_by: str | None = None
    claimed_at: str | None = None
    backoff_base_seconds: float = 0.0
    backoff_multiplier: float = 2.0
    max_backoff_seconds: float | None = None
    backoff_seconds: float | None = None
    next_retry_at: str | None = None
    retry_scheduled_at: str | None = None
    retry_claimed_at: str | None = None
    interrupt_requested_at: str | None = None
    interrupted_at: str | None = None
    last_attempt_started_at: str | None = None
    last_attempt_finished_at: str | None = None
    last_failure_at: str | None = None

    def touch(self, **updates: Any) -> "BackgroundRunStatus":
        """Return a copy with updated timestamp and payload fields.

        Parameters:
            **updates: Updated field values.

        Returns:
            BackgroundRunStatus: Updated status copy.
        """

        return self.model_copy(update={**updates, "updated_at": _now_iso()})
