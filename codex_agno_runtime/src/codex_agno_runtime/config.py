"""Configuration for the Codex Agno runtime."""

from __future__ import annotations

from pathlib import Path
from typing import Optional

from pydantic import AliasChoices, BaseModel, Field

try:  # pragma: no cover - dependency availability is environment-specific
    from pydantic_settings import BaseSettings, SettingsConfigDict
except Exception:  # pragma: no cover - lightweight fallback for test/dev envs
    BaseSettings = BaseModel

    def SettingsConfigDict(**kwargs: object) -> dict[str, object]:
        """Fallback stub mirroring the pydantic-settings factory."""

        return dict(kwargs)

from codex_agno_runtime.paths import default_codex_home

# Default aggregator credentials (shared with agno-server)
_DEFAULT_AGGREGATOR_BASE_URL = "http://127.0.0.1:20128/v1"
_DEFAULT_AGGREGATOR_API_KEY = "sk-aggregator-aa996d6ea7b880c9886cabe7a7532372"
_DEFAULT_MODEL_ID = "gpt-5.4"


class RuntimeSettings(BaseSettings):
    """Application settings loaded from environment variables.

    Parameters:
        None.

    Returns:
        RuntimeSettings: The validated runtime settings instance.
    """

    model_config = SettingsConfigDict(
        env_file=".env",
        env_file_encoding="utf-8",
        env_prefix="",
        extra="ignore",
        populate_by_name=True,
    )

    codex_home: Path = Field(
        default_factory=default_codex_home,
        validation_alias=AliasChoices("CODEX_HOME", "CODEX_AGNO_CODEX_HOME"),
    )
    data_dir: Path = Field(default=Path("data"), validation_alias=AliasChoices("CODEX_AGNO_DATA_DIR"))
    db_file: Path = Field(default=Path("data/codex.db"), validation_alias=AliasChoices("CODEX_AGNO_DB_FILE"))
    checkpoint_storage_backend_family: str = Field(
        default="filesystem",
        validation_alias=AliasChoices("CODEX_AGNO_CHECKPOINT_STORAGE_BACKEND_FAMILY"),
    )
    checkpoint_storage_db_file: Path = Field(
        default=Path("runtime_checkpoint_store.sqlite3"),
        validation_alias=AliasChoices("CODEX_AGNO_CHECKPOINT_STORAGE_DB_FILE"),
    )

    # Model and aggregator settings
    model_id: str = Field(
        default=_DEFAULT_MODEL_ID,
        validation_alias=AliasChoices("CODEX_AGNO_MODEL_ID"),
    )
    aggregator_base_url: str = Field(
        default=_DEFAULT_AGGREGATOR_BASE_URL,
        validation_alias=AliasChoices("CODEX_AGNO_AGGREGATOR_BASE_URL"),
    )
    aggregator_api_key: str = Field(
        default=_DEFAULT_AGGREGATOR_API_KEY,
        validation_alias=AliasChoices("CODEX_AGNO_AGGREGATOR_API_KEY"),
    )

    # Legacy OPENAI_API_KEY support (no longer required with aggregator)
    openai_api_key: Optional[str] = Field(
        default=None,
        validation_alias=AliasChoices("OPENAI_API_KEY", "CODEX_AGNO_OPENAI_API_KEY"),
    )

    default_output_tokens: int = Field(
        default=4096,
        validation_alias=AliasChoices("CODEX_AGNO_DEFAULT_OUTPUT_TOKENS"),
    )
    agent_os_id: str = Field(default="codex-agent-os", validation_alias=AliasChoices("CODEX_AGNO_AGENT_OS_ID"))
    agent_os_name: str = Field(default="Codex AgentOS", validation_alias=AliasChoices("CODEX_AGNO_AGENT_OS_NAME"))
    timezone_identifier: str = Field(default="Asia/Shanghai", validation_alias=AliasChoices("TZ", "CODEX_AGNO_TIMEZONE"))
    max_bash_output_chars: int = Field(default=6000, validation_alias=AliasChoices("CODEX_AGNO_MAX_BASH_OUTPUT_CHARS"))
    enable_mock_compat: bool = Field(default=True, validation_alias=AliasChoices("CODEX_AGNO_ENABLE_MOCK_COMPAT"))
    enable_subagents: bool = Field(default=False, validation_alias=AliasChoices("CODEX_AGNO_ENABLE_SUBAGENTS"))
    subagent_model_id: str = Field(
        default="gpt-5.4-mini",
        validation_alias=AliasChoices("CODEX_AGNO_SUBAGENT_MODEL_ID"),
    )

    # --- DeerFlow-inspired middleware settings ---
    # Context budget and compression (ContextCompressionMiddleware)
    context_budget_tokens: int = Field(
        default=80000,
        validation_alias=AliasChoices("CODEX_AGNO_CONTEXT_BUDGET_TOKENS"),
    )
    compression_threshold: float = Field(
        default=0.75,
        validation_alias=AliasChoices("CODEX_AGNO_COMPRESSION_THRESHOLD"),
    )

    # Sub-agent hard limits (SubagentLimitMiddleware)
    max_concurrent_subagents: int = Field(
        default=3,
        validation_alias=AliasChoices("CODEX_AGNO_MAX_CONCURRENT_SUBAGENTS"),
    )
    subagent_timeout_seconds: int = Field(
        default=900,
        validation_alias=AliasChoices("CODEX_AGNO_SUBAGENT_TIMEOUT_SECONDS"),
    )

    # Long-term memory (MemoryMiddleware)
    memory_enabled: bool = Field(
        default=True,
        validation_alias=AliasChoices("CODEX_AGNO_MEMORY_ENABLED"),
    )
    memory_debounce_seconds: float = Field(
        default=5.0,
        validation_alias=AliasChoices("CODEX_AGNO_MEMORY_DEBOUNCE_SECONDS"),
    )

    # Progressive skill loading
    progressive_skill_loading: bool = Field(
        default=True,
        validation_alias=AliasChoices("CODEX_AGNO_PROGRESSIVE_SKILL_LOADING"),
    )

    route_engine_mode: str = Field(
        default="rust",
        validation_alias=AliasChoices("CODEX_AGNO_ROUTE_ENGINE_MODE"),
    )
    rust_route_rollback_to_python: bool = Field(
        default=False,
        validation_alias=AliasChoices("CODEX_AGNO_RUST_ROUTE_ROLLBACK_TO_PYTHON"),
    )
    rust_router_timeout_seconds: float = Field(
        default=5.0,
        validation_alias=AliasChoices("CODEX_AGNO_RUST_ROUTER_TIMEOUT_SECONDS"),
    )
    rust_execute_fallback_to_python: bool = Field(
        default=False,
        validation_alias=AliasChoices("CODEX_AGNO_RUST_EXECUTE_FALLBACK_TO_PYTHON"),
        description=(
            "Retired explicit-request surface only. When enabled, the runtime still "
            "rejects the old Python fallback path instead of reopening it."
        ),
    )

    trace_output_path: Optional[Path] = Field(
        default=None,
        validation_alias=AliasChoices("CODEX_AGNO_TRACE_OUTPUT_PATH"),
    )

    # Explicit override for live model mode; when None, auto-detect from aggregator config.
    live_model_override: Optional[bool] = Field(
        default=None,
        validation_alias=AliasChoices("CODEX_AGNO_LIVE_MODEL"),
    )

    @property
    def resolved_data_dir(self) -> Path:
        """Resolve the runtime data directory.

        Parameters:
            None.

        Returns:
            Path: The absolute data directory.
        """
        return (self.codex_home / "codex_agno_runtime" / self.data_dir).resolve()

    @property
    def resolved_memory_dir(self) -> Path:
        """Resolve the long-term memory directory.

        Parameters:
            None.

        Returns:
            Path: The absolute memory directory.
        """
        return (self.codex_home / "codex_agno_runtime" / "data" / "memory").resolve()

    @property
    def resolved_db_file(self) -> Path:
        """Resolve the SQLite database file path.

        Parameters:
            None.

        Returns:
            Path: The absolute SQLite database file path.
        """
        return (self.codex_home / "codex_agno_runtime" / self.db_file).resolve()

    @property
    def resolved_checkpoint_storage_db_file(self) -> Path:
        """Resolve the optional SQLite-backed checkpoint storage file path."""

        if self.checkpoint_storage_db_file.is_absolute():
            return self.checkpoint_storage_db_file.expanduser().resolve()
        return (self.resolved_data_dir / self.checkpoint_storage_db_file).resolve()

    @property
    def resolved_trace_output_path(self) -> Path | None:
        """Resolve the optional runtime trace artifact output path.

        Returns:
            Path | None: Trace metadata artifact path when configured.
        """

        if self.trace_output_path is None:
            return None
        path = self.trace_output_path.expanduser()
        if path.is_absolute():
            return path.resolve()
        return (self.codex_home / path).resolve()

    @property
    def use_live_model(self) -> bool:
        """Whether a live model backend is available.

        Returns True only when explicitly enabled via CODEX_AGNO_LIVE_MODEL=true,
        or when the aggregator API key has been changed from its default placeholder.

        Parameters:
            None.

        Returns:
            bool: True when live model execution is intended.
        """
        if self.live_model_override is not None:
            return bool(self.live_model_override)
        # Auto-detect: only enable live mode if the API key was explicitly configured
        return (
            bool(self.aggregator_base_url)
            and bool(self.aggregator_api_key)
            and self.aggregator_api_key != _DEFAULT_AGGREGATOR_API_KEY
        )

    @property
    def context_compression_threshold(self) -> float:
        """Alias for compression_threshold for backward compatibility.

        Parameters:
            None.

        Returns:
            float: The context compression threshold fraction.
        """
        return self.compression_threshold
