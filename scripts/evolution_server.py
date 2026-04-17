from __future__ import annotations

import hashlib
import json
import logging
import os
import subprocess
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Literal
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

import mysql.connector
from fastapi import FastAPI, HTTPException
from fastapi.responses import FileResponse
from fastapi.staticfiles import StaticFiles
from pydantic import BaseModel, ConfigDict, Field

app = FastAPI(
    title="Evolution Dashboard API",
    version="1.1.0",
    description=(
        "Codex-safe dashboard API for health snapshots, journal data, "
        "and masked sub-account token management."
    ),
)

LOGGER = logging.getLogger(__name__)
BASE_DIR = Path(__file__).parent.parent
RS_BIN = BASE_DIR / "scripts" / "evolution-rs" / "target" / "release" / "evolution-rs"
JOURNAL_PATH = BASE_DIR / "skills" / ".evolution_journal.jsonl"
MANIFEST_PATH = BASE_DIR / "skills" / "SKILL_MANIFEST.json"
DASHBOARD_DIR = BASE_DIR / "scripts" / "evolution_dashboard"
SUB_ACCOUNTS_CONFIG_PATH = Path(
    os.getenv(
        "CODEX_SUB_ACCOUNTS_CONFIG",
        str(BASE_DIR / "configs" / "codex" / "sub_accounts.json"),
    )
)
SUB_ACCOUNTS_STATE_PATH = Path(
    os.getenv(
        "CODEX_SUB_ACCOUNTS_STATE",
        str(BASE_DIR / "configs" / "codex" / "sub_accounts.state.json"),
    )
)
DB_CONFIG = {
    "host": os.getenv("CODEX_DB_HOST", os.getenv("DB_HOST", "localhost")),
    "user": os.getenv("CODEX_DB_USER", os.getenv("DB_USER", "root")),
    "password": os.getenv("CODEX_DB_PASS", os.getenv("DB_PASS", "123456")),
    "database": os.getenv("CODEX_DB_NAME", os.getenv("DB_NAME", "oneapi")),
    "connection_timeout": int(os.getenv("CODEX_DB_TIMEOUT", "2")),
}
DEFAULT_MAIN_API_NAME = os.getenv("CODEX_MAIN_API_NAME", os.getenv("MAIN_API_NAME", "Codex Aggregated API"))
DEFAULT_MAIN_API_BASE_URL = os.getenv("CODEX_MAIN_API_BASE_URL", os.getenv("MAIN_API_URL", ""))
DEFAULT_MAIN_API_PROVIDER = os.getenv("CODEX_MAIN_API_PROVIDER", "codex-compatible")
DEFAULT_USAGE_LIMIT_5H = int(os.getenv("CODEX_USAGE_LIMIT_5H", "40"))
DEFAULT_USAGE_LIMIT_7D = int(os.getenv("CODEX_USAGE_LIMIT_7D", "1000"))

DASHBOARD_DIR.mkdir(parents=True, exist_ok=True)


class ApiErrorResponse(BaseModel):
    """Represent a normalized API error payload.

    Parameters:
        detail: Human-readable error message.

    Returns:
        None.
    """

    detail: str


class SkillHealthInfo(BaseModel):
    """Represent one skill health row from the Rust manifest output.

    Parameters:
        dynamic_score: Current runtime score.
        health_status: Human-readable health status.
        reroutes_30d: Recent reroute count.
        static_score: Static lint or structure score.
        usage_30d: Recent usage count.

    Returns:
        None.
    """

    model_config = ConfigDict(extra="allow")

    dynamic_score: float = 0.0
    health_status: str = "Unknown"
    reroutes_30d: int = 0
    static_score: float = 0.0
    usage_30d: int = 0


class HealthSummary(BaseModel):
    """Represent aggregate dashboard health metrics.

    Parameters:
        avg_health: Average skill health score.
        critical_skills: Number of critical skills.
        total_skills: Total indexed skills.
        total_usage: Sum of recent usage counts.

    Returns:
        None.
    """

    avg_health: float = 0.0
    critical_skills: int = 0
    total_skills: int = 0
    total_usage: int = 0


class HealthResponse(BaseModel):
    """Represent the typed response for the dashboard health snapshot.

    Parameters:
        critical_outliers: Names of critical skills.
        skills: Per-skill health rows.
        summary: Aggregate summary block.
        ts: Snapshot timestamp.

    Returns:
        None.
    """

    model_config = ConfigDict(extra="allow")

    critical_outliers: list[str] = Field(default_factory=list)
    skills: dict[str, SkillHealthInfo] = Field(default_factory=dict)
    summary: HealthSummary = Field(default_factory=HealthSummary)
    ts: datetime = Field(default_factory=lambda: datetime.now(UTC))


class JournalEntry(BaseModel):
    """Represent one routing journal entry.

    Parameters:
        ts: Event timestamp.
        task: Task summary.
        init: Initial routed skill.
        final: Final routed skill.
        conf: Routing confidence.
        diff: Difficulty score.
        reroute: Whether rerouting occurred.
        struggle: Retry or struggle score.
        notes: Optional operator note.

    Returns:
        None.
    """

    ts: datetime
    task: str
    init: str
    final: str
    conf: float = 0.0
    diff: int = 0
    reroute: bool = False
    struggle: int = 0
    notes: str = ""


class TokenView(BaseModel):
    """Represent masked token state returned to the UI.

    Parameters:
        source: Token source category.
        masked_preview: Masked token preview.
        status: Token availability state.
        stability: Token stability classification.
        refreshable: Whether manual reload is supported.
        revision: Stable revision counter.
        last_refreshed_at: Last reload timestamp.

    Returns:
        None.
    """

    source: Literal["access_token", "refresh_token", "session_token", "unconfigured"]
    masked_preview: str | None = None
    status: Literal["ready", "missing"] = "missing"
    stability: Literal["fresh", "stable", "rotated", "unknown"] = "unknown"
    refreshable: bool = False
    revision: int = 0
    last_refreshed_at: datetime | None = None


class UsageView(BaseModel):
    """Represent per-account usage counters.

    Parameters:
        used_5h: Requests made in the last five hours.
        limit_5h: Soft five-hour limit.
        used_7d: Requests made in the last seven days.
        limit_7d: Soft seven-day limit.
        source: Usage source classification.
        frozen: Whether the stats are currently frozen.

    Returns:
        None.
    """

    used_5h: int = 0
    limit_5h: int = DEFAULT_USAGE_LIMIT_5H
    used_7d: int = 0
    limit_7d: int = DEFAULT_USAGE_LIMIT_7D
    source: Literal["database", "fallback"] = "fallback"
    frozen: bool = True


class AccountView(BaseModel):
    """Represent one sub-account card payload.

    Parameters:
        id: Stable account identifier.
        name: Display name.
        provider: Account provider label.
        status: UI status.
        channel_id: Optional upstream channel identifier.
        token: Masked token payload.
        usage: Usage counters.
        warnings: Account-local warnings.

    Returns:
        None.
    """

    id: str
    name: str
    provider: str = "codex"
    status: Literal["active", "degraded", "inactive"] = "inactive"
    channel_id: int | None = None
    token: TokenView
    usage: UsageView
    token_preview: str | None = None
    token_status: Literal["ready", "missing"] = "missing"
    token_source: str = "unconfigured"
    stability: Literal["fresh", "stable", "rotated", "unknown"] = "unknown"
    refreshed_at: datetime | None = None
    used_5h: int = 0
    limit_5h: int = DEFAULT_USAGE_LIMIT_5H
    used_7d: int = 0
    limit_7d: int = DEFAULT_USAGE_LIMIT_7D
    alert_message: str | None = None
    warnings: list[str] = Field(default_factory=list)


class MainApiProbe(BaseModel):
    """Represent the probed main API card payload.

    Parameters:
        name: Display name.
        base_url: Base URL of the probed API.
        provider: Provider label.
        status: Probe status.
        codex_compatible: Whether the probe looked Codex-compatible.
        checked_at: Probe timestamp.
        message: Optional probe note.

    Returns:
        None.
    """

    name: str = DEFAULT_MAIN_API_NAME
    url: str | None = None
    base_url: str | None = None
    provider: str = DEFAULT_MAIN_API_PROVIDER
    status: Literal["healthy", "degraded", "down", "unconfigured"] = "unconfigured"
    availability: Literal["active", "degraded", "unconfigured"] = "unconfigured"
    codex_compatible: bool = False
    checked_at: datetime = Field(default_factory=lambda: datetime.now(UTC))
    message: str | None = None


class AccountsSummary(BaseModel):
    """Represent aggregate account summary metrics.

    Parameters:
        total_accounts: Total account count.
        active_accounts: Active account count.
        ready_tokens: Ready token count.
        degraded_accounts: Degraded or inactive count.
        stats_frozen: Whether dashboard stats stay frozen during token refresh.

    Returns:
        None.
    """

    total_accounts: int = 0
    active_accounts: int = 0
    token_ready_accounts: int = 0
    frozen_accounts: int = 0
    ready_tokens: int = 0
    degraded_accounts: int = 0
    stats_frozen: bool = True


class AccountsResponse(BaseModel):
    """Represent the typed response for account inventory requests.

    Parameters:
        main_api: Main API probe payload.
        summary: Aggregate account summary.
        accounts: Account card payloads.
        warnings: Response-level warnings.
        fetched_at: Response timestamp.

    Returns:
        None.
    """

    main_api: MainApiProbe
    summary: AccountsSummary
    accounts: list[AccountView] = Field(default_factory=list)
    warnings: list[str] = Field(default_factory=list)
    fetched_at: datetime = Field(default_factory=lambda: datetime.now(UTC))


class RefreshAccountsRequest(BaseModel):
    """Represent the request body for token-pool refresh operations.

    Parameters:
        reason: Caller-provided refresh reason.

    Returns:
        None.
    """

    reason: str = "manual_ui_refresh"


class RefreshAccountResult(BaseModel):
    """Represent one refreshed account result.

    Parameters:
        account_id: Stable account identifier.
        token_status: Refreshed token status.
        stability: Refreshed token stability.
        refreshed: Whether a token payload is available after reload.
        last_refreshed_at: Last reload timestamp.

    Returns:
        None.
    """

    account_id: str
    token_status: Literal["ready", "missing"]
    stability: Literal["fresh", "stable", "rotated", "unknown"]
    refreshed: bool
    last_refreshed_at: datetime | None = None


class RefreshAccountsResponse(BaseModel):
    """Represent the response for token-pool refresh operations.

    Parameters:
        stats_frozen: Whether homepage stats remain frozen.
        refreshed_count: Count of accounts with usable token material.
        results: Per-account refresh results.
        warnings: Response-level warnings.
        fetched_at: Response timestamp.

    Returns:
        None.
    """

    stats_frozen: bool = True
    refresh_scope: Literal["accounts"] = "accounts"
    refreshed_count: int = 0
    results: list[RefreshAccountResult] = Field(default_factory=list)
    warnings: list[str] = Field(default_factory=list)
    fetched_at: datetime = Field(default_factory=lambda: datetime.now(UTC))


class AuditResponse(BaseModel):
    """Represent a flexible audit payload.

    Parameters:
        payload: Raw audit response.

    Returns:
        None.
    """

    model_config = ConfigDict(extra="allow")


def utc_now() -> datetime:
    """Return the current UTC timestamp.

    Parameters:
        None.

    Returns:
        datetime: Current UTC datetime.
    """

    return datetime.now(UTC)


def get_db_connection() -> mysql.connector.MySQLConnection:
    """Create a MySQL connection using the configured dashboard settings.

    Parameters:
        None.

    Returns:
        mysql.connector.MySQLConnection: Open database connection.
    """

    return mysql.connector.connect(**DB_CONFIG)


def run_rust_command(*args: str) -> dict[str, Any]:
    """Execute the Rust helper and decode its JSON payload.

    Parameters:
        *args: Command-line arguments passed to the Rust binary.

    Returns:
        dict[str, Any]: Parsed JSON payload.
    """

    if not RS_BIN.exists():
        raise HTTPException(status_code=503, detail="Rust binary missing. Build it first.")

    cmd = [str(RS_BIN), *args]
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=20)
    if result.returncode != 0:
        raise HTTPException(status_code=502, detail="Rust helper execution failed.")
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        LOGGER.warning("Failed to decode Rust output: %s", exc)
        raise HTTPException(status_code=502, detail="Rust helper returned invalid JSON.") from exc


def read_json_file(path: Path) -> dict[str, Any]:
    """Read a JSON document from disk.

    Parameters:
        path: Target file path.

    Returns:
        dict[str, Any]: Parsed JSON document or an empty mapping.
    """

    if not path.exists():
        return {}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        LOGGER.warning("Failed to decode JSON file %s: %s", path, exc)
        return {}


def write_json_file(path: Path, payload: dict[str, Any]) -> None:
    """Persist a JSON document to disk.

    Parameters:
        path: Target file path.
        payload: JSON-serializable mapping.

    Returns:
        None.
    """

    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")


def load_sub_accounts_config() -> dict[str, Any]:
    """Load the sub-account registry configuration.

    Parameters:
        None.

    Returns:
        dict[str, Any]: Normalized config payload.
    """

    payload = read_json_file(SUB_ACCOUNTS_CONFIG_PATH)
    return {
        "main_api": payload.get(
            "main_api",
            {
                "name": os.getenv("CODEX_MAIN_API_NAME", os.getenv("MAIN_API_NAME", DEFAULT_MAIN_API_NAME)),
                "base_url": os.getenv("CODEX_MAIN_API_BASE_URL", os.getenv("MAIN_API_URL", DEFAULT_MAIN_API_BASE_URL)),
                "provider": os.getenv("CODEX_MAIN_API_PROVIDER", DEFAULT_MAIN_API_PROVIDER),
            },
        ),
        "accounts": payload.get("accounts", []),
    }


def load_runtime_state() -> dict[str, Any]:
    """Load the persisted token runtime state.

    Parameters:
        None.

    Returns:
        dict[str, Any]: Runtime state document.
    """

    state = read_json_file(SUB_ACCOUNTS_STATE_PATH)
    if "accounts" not in state:
        state["accounts"] = {}
    return state


def build_account_id(raw_account: dict[str, Any], fallback_name: str) -> str:
    """Build a stable account identifier.

    Parameters:
        raw_account: Raw account config entry.
        fallback_name: Name fallback for slug generation.

    Returns:
        str: Stable account identifier.
    """

    explicit = str(raw_account.get("id", "")).strip()
    if explicit:
        return explicit
    seed = fallback_name.strip().lower() or "sub-account"
    allowed = [char if char.isalnum() else "-" for char in seed]
    collapsed = "".join(allowed)
    while "--" in collapsed:
        collapsed = collapsed.replace("--", "-")
    return collapsed.strip("-") or "sub-account"


def mask_secret(secret: str) -> str:
    """Return a masked preview for secret material.

    Parameters:
        secret: Raw secret value.

    Returns:
        str: Masked secret preview.
    """

    if len(secret) <= 8:
        return f"{secret[:2]}***{secret[-2:]}"
    return f"{secret[:4]}…{secret[-4:]}"


def fingerprint_secret(secret: str) -> str:
    """Create a stable fingerprint for secret material.

    Parameters:
        secret: Raw secret value.

    Returns:
        str: Short SHA-256 fingerprint.
    """

    return hashlib.sha256(secret.encode("utf-8")).hexdigest()[:16]


def resolve_secret_value(raw_value: str | None, env_key: str | None) -> str | None:
    """Resolve a secret from an inline value or environment variable.

    Parameters:
        raw_value: Inline secret value.
        env_key: Environment variable name.

    Returns:
        str | None: Resolved secret value.
    """

    if raw_value:
        return raw_value
    if env_key:
        return os.getenv(env_key)
    return None


def resolve_account_token(raw_account: dict[str, Any]) -> tuple[str | None, str]:
    """Resolve token material for one account without exposing raw secrets.

    Parameters:
        raw_account: Raw account config entry.

    Returns:
        tuple[str | None, str]: Secret value and logical token source.
    """

    auth = raw_account.get("auth", {}) if isinstance(raw_account.get("auth"), dict) else {}
    candidates = [
        (
            resolve_secret_value(auth.get("access_token"), auth.get("access_token_env")),
            "access_token",
        ),
        (
            resolve_secret_value(auth.get("refresh_token"), auth.get("refresh_token_env")),
            "refresh_token",
        ),
        (
            resolve_secret_value(auth.get("session_token"), auth.get("session_token_env")),
            "session_token",
        ),
    ]
    for secret, source in candidates:
        if secret:
            return secret, source
    return None, "unconfigured"


def fetch_channel_usage_rows() -> tuple[list[dict[str, Any]], str | None]:
    """Fetch account usage rows from the upstream database.

    Parameters:
        None.

    Returns:
        tuple[list[dict[str, Any]], str | None]: Usage rows and an optional warning.
    """

    try:
        conn = get_db_connection()
        cursor = conn.cursor(dictionary=True)
        cursor.execute("SELECT id, name, status FROM channels ORDER BY id ASC")
        channels = cursor.fetchall()
        rows: list[dict[str, Any]] = []
        for channel in channels:
            cursor.execute(
                """
                SELECT count(*) AS count FROM logs
                WHERE channel_id = %s
                  AND created_at > UNIX_TIMESTAMP(NOW() - INTERVAL 5 HOUR)
                """,
                (channel["id"],),
            )
            used_5h = int(cursor.fetchone()["count"])
            cursor.execute(
                """
                SELECT count(*) AS count FROM logs
                WHERE channel_id = %s
                  AND created_at > UNIX_TIMESTAMP(NOW() - INTERVAL 7 DAY)
                """,
                (channel["id"],),
            )
            used_7d = int(cursor.fetchone()["count"])
            rows.append(
                {
                    "id": channel["id"],
                    "name": channel["name"],
                    "status": channel["status"],
                    "used_5h": used_5h,
                    "used_7d": used_7d,
                }
            )
        cursor.close()
        conn.close()
        return rows, None
    except Exception as exc:
        LOGGER.warning("Database usage fetch failed: %s", exc)
        return [], "Usage metrics database is unavailable; quota stats are frozen."


def classify_account_status(raw_status: str | int | None, token_ready: bool) -> Literal["active", "degraded", "inactive"]:
    """Normalize an account status into the public API shape.

    Parameters:
        raw_status: Config or database status value.
        token_ready: Whether token material is currently available.

    Returns:
        Literal["active", "degraded", "inactive"]: Normalized account status.
    """

    if raw_status in {1, "1", "active"} and token_ready:
        return "active"
    if token_ready:
        return "degraded"
    return "inactive"


def probe_main_api(main_api_config: dict[str, Any]) -> MainApiProbe:
    """Probe the configured main API endpoint for Codex-style compatibility.

    Parameters:
        main_api_config: Raw main API config payload.

    Returns:
        MainApiProbe: Typed probe result.
    """

    base_url = str(main_api_config.get("base_url", "")).strip()
    probe = MainApiProbe(
        name=str(main_api_config.get("name", DEFAULT_MAIN_API_NAME)).strip() or DEFAULT_MAIN_API_NAME,
        url=base_url or None,
        base_url=base_url or None,
        provider=str(main_api_config.get("provider", DEFAULT_MAIN_API_PROVIDER)).strip()
        or DEFAULT_MAIN_API_PROVIDER,
        checked_at=utc_now(),
    )
    if not base_url:
        probe.message = "Main API is not configured yet."
        return probe

    request = Request(
        url=f"{base_url.rstrip('/')}/v1/models",
        headers={"Accept": "application/json", "User-Agent": "Codex-Dashboard/1.1"},
        method="GET",
    )
    try:
        with urlopen(request, timeout=2) as response:  # noqa: S310
            content_type = response.headers.get("Content-Type", "")
            payload = response.read().decode("utf-8")
        if "application/json" not in content_type.lower():
            probe.status = "degraded"
            probe.availability = "degraded"
            probe.message = "Probe endpoint did not return JSON."
            return probe
        decoded = json.loads(payload)
        if isinstance(decoded, dict):
            probe.status = "healthy"
            probe.availability = "active"
            probe.codex_compatible = True
            probe.message = "Probe succeeded against /v1/models."
            return probe
        probe.status = "degraded"
        probe.availability = "degraded"
        probe.message = "Probe returned an unexpected response shape."
        return probe
    except HTTPError as exc:
        LOGGER.warning("Main API probe HTTP error: %s", exc)
        probe.status = "degraded"
        probe.availability = "degraded"
        probe.message = f"Probe failed with HTTP {exc.code}."
        return probe
    except (URLError, TimeoutError, json.JSONDecodeError) as exc:
        LOGGER.warning("Main API probe failed: %s", exc)
        probe.status = "down"
        probe.availability = "degraded"
        probe.message = "Probe could not verify a Codex-compatible JSON API."
        return probe


def build_usage_view(
    raw_account: dict[str, Any],
    usage_row: dict[str, Any] | None,
    stats_warning: str | None,
) -> UsageView:
    """Build the public usage view for an account.

    Parameters:
        raw_account: Raw account config entry.
        usage_row: Matched usage row from the database.
        stats_warning: Global stats warning.

    Returns:
        UsageView: Normalized usage view.
    """

    limits = raw_account.get("limits", {}) if isinstance(raw_account.get("limits"), dict) else {}
    return UsageView(
        used_5h=int((usage_row or {}).get("used_5h", 0)),
        limit_5h=int(limits.get("limit_5h", DEFAULT_USAGE_LIMIT_5H)),
        used_7d=int((usage_row or {}).get("used_7d", 0)),
        limit_7d=int(limits.get("limit_7d", DEFAULT_USAGE_LIMIT_7D)),
        source="database" if usage_row else "fallback",
        frozen=stats_warning is not None,
    )


def build_token_view(
    account_id: str,
    secret: str | None,
    token_source: str,
    runtime_state: dict[str, Any],
    *,
    force_refresh: bool,
    refreshed_at: datetime,
) -> TokenView:
    """Build the public token view and update runtime stability state.

    Parameters:
        account_id: Stable account identifier.
        secret: Resolved secret value.
        token_source: Logical secret source.
        runtime_state: Mutable runtime state document.
        force_refresh: Whether this read was triggered by manual refresh.
        refreshed_at: Timestamp for this refresh cycle.

    Returns:
        TokenView: Masked token view.
    """

    account_state = runtime_state.setdefault("accounts", {}).get(account_id, {})
    if not secret:
        return TokenView(
            source="unconfigured",
            status="missing",
            stability="unknown",
            refreshable=False,
            revision=int(account_state.get("revision", 0)),
            last_refreshed_at=None,
        )

    fingerprint = fingerprint_secret(secret)
    previous_fingerprint = account_state.get("fingerprint")
    previous_revision = int(account_state.get("revision", 0))
    if previous_fingerprint is None:
        stability = "fresh"
        revision = 1
    elif previous_fingerprint == fingerprint:
        stability = "stable"
        revision = max(previous_revision, 1)
    else:
        stability = "rotated"
        revision = previous_revision + 1

    last_refreshed_at = refreshed_at if force_refresh or previous_fingerprint != fingerprint else account_state.get("last_refreshed_at")
    runtime_state["accounts"][account_id] = {
        "fingerprint": fingerprint,
        "revision": revision,
        "last_refreshed_at": last_refreshed_at.isoformat() if isinstance(last_refreshed_at, datetime) else last_refreshed_at,
    }
    return TokenView(
        source=token_source if token_source in {"access_token", "refresh_token", "session_token"} else "unconfigured",
        masked_preview=mask_secret(secret),
        status="ready",
        stability=stability,
        refreshable=True,
        revision=revision,
        last_refreshed_at=last_refreshed_at,
    )


def build_accounts_payload(force_refresh: bool) -> AccountsResponse:
    """Build the typed account inventory response.

    Parameters:
        force_refresh: Whether the token registry should be reloaded for a manual refresh.

    Returns:
        AccountsResponse: Typed account inventory response.
    """

    config = load_sub_accounts_config()
    runtime_state = load_runtime_state()
    usage_rows, stats_warning = fetch_channel_usage_rows()
    usage_by_channel = {int(row["id"]): row for row in usage_rows if row.get("id") is not None}
    usage_by_name = {str(row["name"]): row for row in usage_rows if row.get("name")}
    warnings: list[str] = []

    if not SUB_ACCOUNTS_CONFIG_PATH.exists():
        warnings.append(
            "Sub-account registry is not configured. Copy configs/codex/sub_accounts.example.json to sub_accounts.json and wire secrets via env vars."
        )
    if stats_warning:
        warnings.append(stats_warning)

    refreshed_at = utc_now()
    accounts: list[AccountView] = []
    raw_accounts = config.get("accounts", []) if isinstance(config.get("accounts"), list) else []
    env_tokens = [token.strip() for token in os.getenv("COPILOT_TOKENS", "").split(",") if token.strip()]
    if not raw_accounts and env_tokens:
        for index, secret in enumerate(env_tokens, start=1):
            usage_row = usage_rows[index - 1] if index - 1 < len(usage_rows) else {}
            raw_accounts.append(
                {
                    "id": usage_row.get("id") or f"env-account-{index}",
                    "name": usage_row.get("name") or f"Env Account {index:02d}",
                    "provider": "env-token",
                    "status": usage_row.get("status", "active"),
                    "channel_id": usage_row.get("id"),
                    "limits": {},
                    "auth": {"access_token": secret},
                }
            )
    for raw_account in raw_accounts:
        if not isinstance(raw_account, dict):
            continue
        display_name = str(raw_account.get("name", "Sub Account")).strip() or "Sub Account"
        account_id = build_account_id(raw_account, display_name)
        channel_id = raw_account.get("channel_id")
        usage_row = None
        if isinstance(channel_id, int):
            usage_row = usage_by_channel.get(channel_id)
        if usage_row is None:
            usage_row = usage_by_name.get(display_name)
        secret, token_source = resolve_account_token(raw_account)
        token_view = build_token_view(
            account_id,
            secret,
            token_source,
            runtime_state,
            force_refresh=force_refresh,
            refreshed_at=refreshed_at,
        )
        usage_view = build_usage_view(raw_account, usage_row, stats_warning)
        account_warnings: list[str] = []
        if token_view.status == "missing":
            account_warnings.append("No token material is configured for this account.")
        if usage_row is None and stats_warning is None:
            account_warnings.append("Usage metrics are unavailable for this account.")
        accounts.append(
            AccountView(
                id=account_id,
                name=display_name,
                provider=str(raw_account.get("provider", "codex")).strip() or "codex",
                status=classify_account_status(raw_account.get("status"), token_view.status == "ready"),
                channel_id=channel_id if isinstance(channel_id, int) else None,
                token=token_view,
                usage=usage_view,
                token_preview=token_view.masked_preview,
                token_status=token_view.status,
                token_source=token_view.source,
                stability=token_view.stability,
                refreshed_at=token_view.last_refreshed_at,
                used_5h=usage_view.used_5h,
                limit_5h=usage_view.limit_5h,
                used_7d=usage_view.used_7d,
                limit_7d=usage_view.limit_7d,
                alert_message=account_warnings[0] if account_warnings else None,
                warnings=account_warnings,
            )
        )

    if not accounts and usage_rows:
        for row in usage_rows:
            raw_account = {"limits": {}}
            account_id = build_account_id({"id": row.get("id")}, str(row.get("name", "DB Channel")))
            accounts.append(
                AccountView(
                    id=account_id,
                    name=str(row.get("name", "DB Channel")),
                    provider="database",
                    status=classify_account_status(row.get("status"), token_ready=False),
                    channel_id=int(row["id"]),
                    token=TokenView(source="unconfigured", status="missing", stability="unknown", refreshable=False),
                    usage=build_usage_view(raw_account, row, stats_warning),
                    token_preview=None,
                    token_status="missing",
                    token_source="unconfigured",
                    stability="unknown",
                    refreshed_at=None,
                    used_5h=int(row.get("used_5h", 0)),
                    limit_5h=DEFAULT_USAGE_LIMIT_5H,
                    used_7d=int(row.get("used_7d", 0)),
                    limit_7d=DEFAULT_USAGE_LIMIT_7D,
                    alert_message="Database channel exists but no token registry entry is configured.",
                    warnings=["Database channel exists but no token registry entry is configured."],
                )
            )

    write_json_file(SUB_ACCOUNTS_STATE_PATH, runtime_state)
    summary = AccountsSummary(
        total_accounts=len(accounts),
        active_accounts=sum(1 for account in accounts if account.status == "active"),
        token_ready_accounts=sum(1 for account in accounts if account.token.status == "ready"),
        frozen_accounts=sum(1 for account in accounts if account.status != "active"),
        ready_tokens=sum(1 for account in accounts if account.token.status == "ready"),
        degraded_accounts=sum(1 for account in accounts if account.status != "active"),
        stats_frozen=True,
    )
    return AccountsResponse(
        main_api=probe_main_api(config.get("main_api", {})),
        summary=summary,
        accounts=accounts,
        warnings=warnings,
        fetched_at=refreshed_at,
    )


def build_refresh_response(accounts_response: AccountsResponse) -> RefreshAccountsResponse:
    """Build the public refresh response from the account inventory payload.

    Parameters:
        accounts_response: Fresh account inventory payload.

    Returns:
        RefreshAccountsResponse: Token refresh response.
    """

    results = [
        RefreshAccountResult(
            account_id=account.id,
            token_status=account.token.status,
            stability=account.token.stability,
            refreshed=account.token.status == "ready",
            last_refreshed_at=account.token.last_refreshed_at,
        )
        for account in accounts_response.accounts
    ]
    return RefreshAccountsResponse(
        stats_frozen=True,
        refresh_scope="accounts",
        refreshed_count=sum(1 for result in results if result.refreshed),
        results=results,
        warnings=accounts_response.warnings,
        fetched_at=accounts_response.fetched_at,
    )


@app.get(
    "/api/health",
    response_model=HealthResponse,
    responses={500: {"model": ApiErrorResponse}, 502: {"model": ApiErrorResponse}, 503: {"model": ApiErrorResponse}},
    tags=["dashboard"],
    summary="Get the current dashboard health snapshot",
)
async def get_health() -> HealthResponse:
    """Return the current typed dashboard health snapshot.

    Parameters:
        None.

    Returns:
        HealthResponse: Typed dashboard health snapshot.
    """

    payload = run_rust_command("manifest", "--journal", str(JOURNAL_PATH), *( ["--manifest", str(MANIFEST_PATH)] if MANIFEST_PATH.exists() else [] ))
    skills = payload.get("skills", {}) if isinstance(payload.get("skills"), dict) else {}
    summary_payload = payload.get("summary", {}) if isinstance(payload.get("summary"), dict) else {}
    total_usage = sum(int((info or {}).get("usage_30d", 0)) for info in skills.values())
    summary_payload["total_usage"] = total_usage
    payload["summary"] = summary_payload
    return HealthResponse.model_validate(payload)


@app.get(
    "/api/audit",
    response_model=dict[str, Any],
    responses={500: {"model": ApiErrorResponse}, 502: {"model": ApiErrorResponse}, 503: {"model": ApiErrorResponse}},
    tags=["dashboard"],
    summary="Run the current audit snapshot",
)
async def get_audit() -> dict[str, Any]:
    """Return the latest typed audit snapshot.

    Parameters:
        None.

    Returns:
        dict[str, Any]: Audit payload.
    """

    return run_rust_command("audit", "--journal", str(JOURNAL_PATH), "--json")


@app.get(
    "/api/journal",
    response_model=list[JournalEntry],
    responses={500: {"model": ApiErrorResponse}},
    tags=["dashboard"],
    summary="Read recent routing journal entries",
)
async def get_journal(limit: int = 100) -> list[JournalEntry]:
    """Return the latest routing journal entries.

    Parameters:
        limit: Maximum number of entries to return.

    Returns:
        list[JournalEntry]: Recent journal entries.
    """

    if not JOURNAL_PATH.exists():
        return []
    try:
        lines = JOURNAL_PATH.read_text(encoding="utf-8").splitlines()
        payload = [json.loads(line) for line in lines[-max(limit, 0):] if line.strip()]
        return [JournalEntry.model_validate(item) for item in payload]
    except (OSError, json.JSONDecodeError) as exc:
        LOGGER.warning("Journal read failed: %s", exc)
        raise HTTPException(status_code=500, detail="Failed to load the routing journal.") from exc


@app.get(
    "/api/accounts",
    response_model=AccountsResponse,
    responses={500: {"model": ApiErrorResponse}},
    tags=["accounts"],
    summary="Get masked sub-account inventory and main API status",
)
async def get_accounts() -> AccountsResponse:
    """Return masked sub-account inventory without leaking raw secrets.

    Parameters:
        None.

    Returns:
        AccountsResponse: Typed masked account payload.
    """

    return build_accounts_payload(force_refresh=False)


@app.post(
    "/api/accounts/refresh",
    response_model=RefreshAccountsResponse,
    responses={500: {"model": ApiErrorResponse}},
    tags=["accounts"],
    summary="Reload the token pool without refreshing homepage stats",
)
async def refresh_accounts(_: RefreshAccountsRequest | None = None) -> RefreshAccountsResponse:
    """Reload token metadata while keeping homepage stats frozen.

    Parameters:
        _: Request payload with a caller-provided reason.

    Returns:
        RefreshAccountsResponse: Refresh results.
    """

    accounts_response = build_accounts_payload(force_refresh=True)
    return build_refresh_response(accounts_response)


@app.get("/", include_in_schema=False)
async def serve_dashboard() -> FileResponse:
    """Serve the dashboard frontend entrypoint.

    Parameters:
        None.

    Returns:
        FileResponse: Dashboard HTML file.
    """

    index_path = DASHBOARD_DIR / "index.html"
    if not index_path.exists():
        raise HTTPException(status_code=404, detail="Dashboard frontend is missing.")
    return FileResponse(index_path)


app.mount("/static", StaticFiles(directory=str(DASHBOARD_DIR)), name="static")


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="0.0.0.0", port=8000)
