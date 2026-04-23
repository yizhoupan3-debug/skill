"""Typed runtime schemas for the Codex Agno runtime."""

from __future__ import annotations

from dataclasses import dataclass, field as dataclass_field
from datetime import UTC, datetime
from typing import Any, Literal

from pydantic import AliasChoices, BaseModel, Field, field_validator, model_validator


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


class SearchMatchResult(BaseModel):
    """Hydrated Rust search row backed by shared skill metadata."""

    record: SkillMetadata
    score: float = Field(ge=0.0)
    matched_terms: int = Field(ge=0)
    total_terms: int = Field(ge=0)

    @model_validator(mode="after")
    def _validate_match_counts(self) -> "SearchMatchResult":
        if self.matched_terms > self.total_terms:
            raise ValueError("matched_terms cannot exceed total_terms")
        return self


class SearchMatchesContract(BaseModel):
    """Stable Rust-owned search envelope consumed by Python route helpers."""

    search_schema_version: str
    authority: str
    query: str
    matches: list[SearchMatchResult]


class RouteContractDiffField(BaseModel):
    """One typed route contract mismatch field."""

    field: str
    rust_value: Any | None = None
    python_value: Any | None = None


class RouteContractDiffReport(BaseModel):
    """Structured route decision diff report."""

    verified_contract_fields: list[str] = Field(default_factory=list)
    contract_mismatch_fields: list[str] = Field(default_factory=list)
    mismatched_fields: list[RouteContractDiffField] = Field(default_factory=list)


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
    route_engine: str = "rust"
    diagnostic_route_mode: Literal["none", "shadow", "verify"] = "none"
    route_diagnostic_report: "RouteDiagnosticReport | None" = None

    @field_validator("route_snapshot", "route_diagnostic_report", mode="before")
    @classmethod
    def _coerce_foreign_pydantic_models(cls, value: Any) -> Any:
        if value is not None and hasattr(value, "model_dump"):
            return value.model_dump(mode="json")
        return value

    @model_validator(mode="after")
    def _validate_diagnostic_route_contract(self) -> "RoutingResult":
        if self.diagnostic_route_mode == "none":
            if self.route_diagnostic_report is not None:
                raise ValueError("non-diagnostic routing results must not expose route_diagnostic_report")
            return self
        if self.route_diagnostic_report is None:
            raise ValueError("diagnostic routing results must carry route_diagnostic_report evidence")
        if self.route_diagnostic_report.mode != self.diagnostic_route_mode:
            raise ValueError("diagnostic routing results must align diagnostic_route_mode with route_diagnostic_report")
        return self


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


class RouteDecisionContract(BaseModel):
    """Stable Rust-owned route decision envelope consumed by the Python host."""

    decision_schema_version: str
    authority: str
    compile_authority: str
    task: str
    session_id: str
    selected_skill: str
    overlay_skill: str | None = None
    layer: str
    score: float = 0.0
    reasons: list[str] = Field(default_factory=list)
    route_snapshot: RouteDecisionSnapshot

    @model_validator(mode="after")
    def _validate_rust_route_decision_contract(self) -> "RouteDecisionContract":
        if self.route_snapshot.engine != "rust":
            raise ValueError("route decision contract must keep route_snapshot.engine=rust")
        if self.route_snapshot.selected_skill != self.selected_skill:
            raise ValueError("route decision contract selected_skill must match route_snapshot")
        if self.route_snapshot.overlay_skill != self.overlay_skill:
            raise ValueError("route decision contract overlay_skill must match route_snapshot")
        if self.route_snapshot.layer != self.layer:
            raise ValueError("route decision contract layer must match route_snapshot")
        return self


class RouteExecutionPolicy(BaseModel):
    """Stable Rust-owned route-mode policy."""

    policy_schema_version: str
    authority: str
    mode: str
    diagnostic_route_mode: Literal["none", "shadow", "verify"] = "none"
    primary_authority: str
    route_result_engine: str
    diagnostic_report_required: bool = False
    strict_verification_required: bool = False

    @property
    def diagnostic_route_active(self) -> bool:
        """Return whether one Rust-owned diagnostic route mode is active."""

        return self.diagnostic_route_mode != "none"

    @model_validator(mode="after")
    def _validate_rust_only_route_policy(self) -> "RouteExecutionPolicy":
        if self.primary_authority != "rust" or self.route_result_engine != "rust":
            raise ValueError("route policy must keep Rust as the route-result authority")
        if self.mode == "rust":
            if self.diagnostic_route_mode != "none":
                raise ValueError("rust route policy must disable diagnostic_route_mode")
            if self.diagnostic_report_required or self.strict_verification_required:
                raise ValueError("rust route policy must not require diagnostic reporting")
            return self
        if self.mode == "shadow":
            if self.diagnostic_route_mode != "shadow":
                raise ValueError("shadow route policy must set diagnostic_route_mode=shadow")
            if not self.diagnostic_report_required or self.strict_verification_required:
                raise ValueError("shadow route policy must require report-only diagnostics")
            return self
        if self.mode == "verify":
            if self.diagnostic_route_mode != "verify":
                raise ValueError("verify route policy must set diagnostic_route_mode=verify")
            if not self.diagnostic_report_required or not self.strict_verification_required:
                raise ValueError("verify route policy must require strict Rust verification")
            return self
        raise ValueError(f"unsupported route policy mode: {self.mode}")


class RouteDiagnosticReport(BaseModel):
    """Stable Rust-owned diagnostic evidence payload for shadow/verify modes."""

    report_schema_version: str
    authority: str
    mode: Literal["shadow", "verify"]
    primary_engine: str
    evidence_kind: str = "rust-owned-snapshot"
    strict_verification: bool = False
    verification_passed: bool = True
    verified_contract_fields: list[str] = Field(default_factory=list)
    contract_mismatch_fields: list[str] = Field(default_factory=list)
    route_diff: RouteContractDiffReport | None = None
    route_snapshot: RouteDecisionSnapshot

    @model_validator(mode="after")
    def _validate_route_diagnostic_report(self) -> "RouteDiagnosticReport":
        if self.primary_engine != "rust":
            raise ValueError("route diagnostic report must keep Rust as the primary engine")
        if self.evidence_kind != "rust-owned-snapshot":
            raise ValueError("route diagnostic report must use the rust-owned-snapshot evidence kind")
        expected_verification = self.mode == "verify"
        if self.strict_verification != expected_verification:
            raise ValueError("route diagnostic report strict_verification must align with mode")
        if self.verification_passed and self.contract_mismatch_fields:
            raise ValueError("route diagnostic report must not list contract mismatches when verification passes")
        if not self.verification_passed and not self.contract_mismatch_fields:
            raise ValueError("route diagnostic report must describe contract mismatches when verification fails")
        return self


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
    framework_surface_policy: dict[str, Any] = Field(default_factory=dict)
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
    bridge_contract_match: bool | None = None
    bridge_contract_mismatch_fields: list[str] = Field(default_factory=list)
    bridge_contract: dict[str, Any] | None = None
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
    canonical_bridge_contract: dict[str, Any] = Field(default_factory=dict)
    adapter_projections: list[FrameworkSharedContractProjection] = Field(default_factory=list)
    all_shared_contract_projections_match: bool = True
    all_bridge_contract_projections_match: bool = True


class RoutingEvalCase(BaseModel):
    """One routing-eval input row."""

    id: str | int | None = None
    task: str
    category: str
    first_turn: bool = True
    expected_owner: str | None = None
    expected_overlay: str | None = None
    focus_skill: str | None = None
    forbidden_owners: list[str] = Field(default_factory=list)


class RoutingEvalCases(BaseModel):
    """Typed payload loaded from `routing_eval_cases.json`."""

    schema_version: str
    cases: list[RoutingEvalCase] = Field(default_factory=list)


class RoutingEvalResult(BaseModel):
    """One typed routing-eval output row."""

    id: str | int | None = None
    category: str
    task: str
    focus_skill: str | None = None
    selected_owner: str
    selected_overlay: str | None = None
    expected_owner: str | None = None
    expected_overlay: str | None = None
    forbidden_owners: list[str] = Field(default_factory=list)
    trigger_hit: bool = False
    overtrigger: bool = False
    owner_correct: bool = False
    overlay_correct: bool = False


class RoutingEvalMetrics(BaseModel):
    """Aggregated routing-eval metrics."""

    case_count: int = 0
    trigger_hit: int = 0
    trigger_miss: int = 0
    overtrigger: int = 0
    owner_correct: int = 0
    overlay_correct: int = 0


class RoutingEvalReport(BaseModel):
    """Typed routing-eval output."""

    schema_version: str
    metrics: RoutingEvalMetrics
    results: list[RoutingEvalResult] = Field(default_factory=list)


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
    loaded_skill_count: int = 0
    route_engine: str = "rust"
    diagnostic_route_mode: Literal["none", "shadow", "verify"] = "none"
    route_diagnostic_report: RouteDiagnosticReport | None = None

    @model_validator(mode="after")
    def _validate_diagnostic_route_contract(self) -> "PrepareSessionResponse":
        if self.diagnostic_route_mode == "none":
            if self.route_diagnostic_report is not None:
                raise ValueError("non-diagnostic prepared sessions must not expose route_diagnostic_report")
            return self
        if self.route_diagnostic_report is None:
            raise ValueError("diagnostic prepared sessions must carry route_diagnostic_report")
        if self.route_diagnostic_report.mode != self.diagnostic_route_mode:
            raise ValueError(
                "diagnostic prepared sessions must align diagnostic_route_mode with route_diagnostic_report"
            )
        return self


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


SANDBOX_CAPABILITY_CATEGORIES = (
    "read_only",
    "workspace_mutating",
    "networked",
    "high_risk",
)


@dataclass(slots=True, frozen=True)
class SandboxExecutionPolicy:
    """Explicit sandbox capability policy carried with one kernel request."""

    profile: str = "workspace-default"
    capability_categories: tuple[str, ...] = ("read_only", "workspace_mutating", "networked")
    dedicated_profile: bool = False
    reusable: bool = True
    schema_version: str = "runtime-sandbox-policy-v1"

    def to_metadata(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "profile": self.profile,
            "capability_categories": list(self.capability_categories),
            "dedicated_profile": self.dedicated_profile,
            "reusable": self.reusable,
        }


@dataclass(slots=True, frozen=True)
class SandboxResourceBudget:
    """Runtime sandbox resource budgets attached to one execution."""

    cpu: float = 30.0
    memory: int = 512 * 1024 * 1024
    wall_clock: float = 30.0
    output_size: int = 64 * 1024
    schema_version: str = "runtime-sandbox-budget-v1"

    def to_metadata(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "cpu": self.cpu,
            "memory": self.memory,
            "wall_clock": self.wall_clock,
            "output_size": self.output_size,
        }


@dataclass(slots=True, frozen=True)
class SandboxRuntimeProbe:
    """Optional runtime measurements supplied by the host around kernel execution."""

    cpu: float | None = None
    memory: int | None = None
    wall_clock: float | None = None
    output_size: int | None = None
    source: str = "host-runtime"
    schema_version: str = "runtime-sandbox-runtime-probe-v1"

    def to_metadata(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "cpu": self.cpu,
            "memory": self.memory,
            "wall_clock": self.wall_clock,
            "output_size": self.output_size,
            "source": self.source,
        }


@dataclass(slots=True)
class ExecutionKernelRequest:
    """Normalized execution payload passed to the active kernel adapter."""

    task: str
    session_id: str
    user_id: str
    routing_result: RoutingResult
    job_id: str | None = None
    dry_run: bool = False
    trace_event_count: int = 0
    trace_output_path: str | None = None
    sandbox_policy: SandboxExecutionPolicy = dataclass_field(default_factory=SandboxExecutionPolicy)
    sandbox_budget: SandboxResourceBudget = dataclass_field(default_factory=SandboxResourceBudget)
    sandbox_tool_category: str = "workspace_mutating"
    sandbox_runtime_probe: SandboxRuntimeProbe | None = None


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
