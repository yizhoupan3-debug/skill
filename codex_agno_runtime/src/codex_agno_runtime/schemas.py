"""Typed runtime schemas for the Codex Agno runtime."""

from __future__ import annotations

from datetime import UTC, datetime
from typing import Any, Literal

from pydantic import AliasChoices, BaseModel, Field, model_validator


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
    diagnostic_python_lane_active: bool = Field(
        default=False,
        validation_alias=AliasChoices("diagnostic_python_lane_active", "rollback_to_python"),
        serialization_alias="diagnostic_python_lane_active",
    )
    python_lane_kind: Literal["none", "legacy-primary", "diagnostic-compare-only"] = "none"
    shadow_route_report: "RouteDiffReport | None" = None

    @model_validator(mode="after")
    def _validate_python_lane_contract(self) -> "RoutingResult":
        if self.python_lane_kind == "diagnostic-compare-only":
            if not self.diagnostic_python_lane_active:
                raise ValueError(
                    "diagnostic-compare-only routing results must keep diagnostic_python_lane_active enabled"
                )
            if self.shadow_route_report is None:
                raise ValueError("diagnostic-compare-only routing results must carry shadow_route_report evidence")
            return self
        if self.diagnostic_python_lane_active:
            raise ValueError("non-diagnostic routing results must keep diagnostic_python_lane_active disabled")
        if self.shadow_route_report is not None:
            raise ValueError("non-diagnostic routing results must not expose shadow_route_report")
        return self

    @property
    def rollback_to_python(self) -> bool:
        """Backward-compatible attribute kept during the naming migration."""

        return self.diagnostic_python_lane_active


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
    `rollback_active` records a compatibility/diagnostic marker and never changes
    the live route-result authority on its own.
    """

    policy_schema_version: str
    authority: str
    mode: str
    rollback_active: bool = False
    python_route_required: bool = False
    diagnostic_python_lane: bool = False
    python_lane_kind: Literal["none", "legacy-primary", "diagnostic-compare-only"] = "none"
    primary_authority: str
    route_result_engine: str
    shadow_engine: str | None = None
    diff_report_required: bool = False
    verify_parity_required: bool = False

    @property
    def legacy_primary_python_lane_active(self) -> bool:
        """Return whether Python is the explicit primary route lane."""

        return self.python_lane_kind == "legacy-primary"

    @property
    def diagnostic_compare_only_python_lane_active(self) -> bool:
        """Return whether Python is only participating as compare-only evidence."""

        return self.python_lane_kind == "diagnostic-compare-only"

    @model_validator(mode="after")
    def _validate_python_lane_contract(self) -> "RouteExecutionPolicy":
        if self.verify_parity_required and not self.diff_report_required:
            raise ValueError("verify_parity_required requires diff_report_required")
        if self.python_lane_kind == "legacy-primary":
            if not self.python_route_required or self.diagnostic_python_lane:
                raise ValueError(
                    "legacy-primary route policy must enable python_route_required and disable diagnostic_python_lane"
                )
            if self.primary_authority != "python" or self.route_result_engine != "python":
                raise ValueError("legacy-primary route policy must keep Python as the route-result authority")
            if self.shadow_engine is not None:
                raise ValueError("legacy-primary route policy must not expose shadow_engine")
            return self
        if self.python_lane_kind == "diagnostic-compare-only":
            if self.python_route_required or not self.diagnostic_python_lane:
                raise ValueError(
                    "diagnostic-compare-only route policy must disable python_route_required and enable diagnostic_python_lane"
                )
            if self.primary_authority != "rust" or self.route_result_engine != "rust":
                raise ValueError("diagnostic-compare-only route policy must keep Rust as the route-result authority")
            if self.shadow_engine != "python":
                raise ValueError("diagnostic-compare-only route policy must keep shadow_engine set to python")
            return self
        if self.python_route_required or self.diagnostic_python_lane:
            raise ValueError("python_lane_kind=none must disable all Python route lanes")
        if self.shadow_engine is not None:
            raise ValueError("python_lane_kind=none must not expose shadow_engine")
        return self


class RouteDiffReport(BaseModel):
    """Stable diagnostic evidence payload shared by shadow/verify/rust lanes."""

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
    diagnostic_python_lane_active: bool = Field(
        default=False,
        validation_alias=AliasChoices("diagnostic_python_lane_active", "rollback_to_python"),
        serialization_alias="diagnostic_python_lane_active",
    )
    python_lane_kind: Literal["none", "legacy-primary", "diagnostic-compare-only"] = "none"
    shadow_route_report: RouteDiffReport | None = None

    @model_validator(mode="after")
    def _validate_python_lane_contract(self) -> "PrepareSessionResponse":
        if self.python_lane_kind == "diagnostic-compare-only":
            if not self.diagnostic_python_lane_active:
                raise ValueError(
                    "diagnostic-compare-only prepared sessions must keep diagnostic_python_lane_active enabled"
                )
            if self.shadow_route_report is None:
                raise ValueError("diagnostic-compare-only prepared sessions must carry shadow_route_report")
            return self
        if self.diagnostic_python_lane_active:
            raise ValueError("non-diagnostic prepared sessions must keep diagnostic_python_lane_active disabled")
        if self.shadow_route_report is not None:
            raise ValueError("non-diagnostic prepared sessions must not expose shadow_route_report")
        return self

    @property
    def rollback_to_python(self) -> bool:
        """Backward-compatible attribute kept during the naming migration."""

        return self.diagnostic_python_lane_active


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
    job_id: str | None = None
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

    parallel_group_id: str | None = None
    lane_id: str | None = None
    parent_job_id: str | None = None
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
    parallel_group_id: str | None = None
    lane_id: str | None = None
    parent_job_id: str | None = None
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


class BackgroundParallelGroupSummary(BaseModel):
    """Aggregate one durable parallel background batch."""

    parallel_group_id: str
    job_ids: list[str] = Field(default_factory=list)
    session_ids: list[str] = Field(default_factory=list)
    lane_ids: list[str] = Field(default_factory=list)
    parent_job_ids: list[str] = Field(default_factory=list)
    status_counts: dict[str, int] = Field(default_factory=dict)
    active_job_count: int = 0
    terminal_job_count: int = 0
    total_job_count: int = 0
    latest_updated_at: str | None = None


class BackgroundBatchEnqueueResponse(BaseModel):
    """Result of admitting one bounded parallel batch."""

    parallel_group_id: str
    statuses: list[BackgroundRunStatus] = Field(default_factory=list)
    summary: BackgroundParallelGroupSummary
