"""High-signal regression tests for the evolution dashboard API."""

from __future__ import annotations

import asyncio
import json
import sys
from pathlib import Path
from types import ModuleType
from types import SimpleNamespace
from typing import Any

import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))


def _install_runtime_shims() -> None:
    """Install lightweight module shims when optional runtime dependencies are absent.

    Parameters:
        None.

    Returns:
        None.
    """

    fastapi_module = ModuleType("fastapi")

    class _HTTPException(Exception):
        """Minimal HTTP exception shim used by the test target."""

        def __init__(self, status_code: int, detail: str) -> None:
            super().__init__(detail)
            self.status_code = status_code
            self.detail = detail

    class _FastAPI:
        """Minimal FastAPI shim that records registered routes."""

        def __init__(self, title: str = "", **kwargs: Any) -> None:
            self.title = title
            self.routes: dict[str, Any] = {}

        def get(self, path: str, **kwargs: Any):
            """Register a GET handler.

            Parameters:
                path: Route path.

            Returns:
                A decorator that stores the handler.
            """

            def decorator(func):
                self.routes[path] = func
                return func

            return decorator

        def post(self, path: str, **kwargs: Any):
            """Register a POST handler.

            Parameters:
                path: Route path.
                kwargs: Unused decorator keyword arguments.

            Returns:
                A decorator that stores the handler.
            """

            def decorator(func):
                self.routes[path] = func
                return func

            return decorator

        def mount(self, *args, **kwargs) -> None:
            """Accept mount calls from the target module.

            Parameters:
                args: Positional mount arguments.
                kwargs: Keyword mount arguments.

            Returns:
                None.
            """

    class _StaticFiles:
        """Placeholder StaticFiles shim."""

        def __init__(self, directory: str) -> None:
            self.directory = directory

    class _FileResponse:
        """Placeholder FileResponse shim."""

        def __init__(self, path: Path) -> None:
            self.path = Path(path)

    fastapi_module.FastAPI = _FastAPI
    fastapi_module.HTTPException = _HTTPException
    staticfiles_module = ModuleType("fastapi.staticfiles")
    staticfiles_module.StaticFiles = _StaticFiles
    responses_module = ModuleType("fastapi.responses")
    responses_module.FileResponse = _FileResponse
    sys.modules["fastapi"] = fastapi_module
    sys.modules["fastapi.staticfiles"] = staticfiles_module
    sys.modules["fastapi.responses"] = responses_module

    mysql_module = ModuleType("mysql")
    connector_module = ModuleType("mysql.connector")

    class _MySQLError(Exception):
        """Minimal mysql connector error shim."""

    def _connect(**kwargs):
        """Placeholder connector entrypoint.

        Parameters:
            kwargs: Connection parameters.

        Returns:
            None.
        """

        raise _MySQLError("mysql connector shim cannot connect")

    connector_module.Error = _MySQLError
    connector_module.connect = _connect
    mysql_module.connector = connector_module
    sys.modules["mysql"] = mysql_module
    sys.modules["mysql.connector"] = connector_module


_install_runtime_shims()

from scripts import evolution_server


class _FakeCursor:
    """Emulate the minimal MySQL cursor behavior needed by the API.

    Parameters:
        channels: Channel rows returned from the channels query.
        counts: Per-channel usage counters keyed by channel id.

    Returns:
        A cursor object with execute/fetchall/fetchone/close methods.
    """

    def __init__(self, channels: list[dict[str, Any]], counts: dict[int, dict[str, int]]) -> None:
        self._channels = channels
        self._counts = counts
        self._rows: list[dict[str, Any]] = []

    def execute(self, query: str, params: tuple[Any, ...] | None = None) -> None:
        """Store deterministic rows for the next fetch call.

        Parameters:
            query: SQL string used by the endpoint.
            params: Optional SQL parameters.

        Returns:
            None.
        """

        normalized = " ".join(query.lower().split())
        if "from channels" in normalized:
            self._rows = list(self._channels)
            return

        if "from logs" in normalized and params:
            channel_id = int(params[0])
            if "interval 5 hour" in normalized:
                self._rows = [{"count": self._counts[channel_id]["5h"]}]
                return
            if "interval 7 day" in normalized:
                self._rows = [{"count": self._counts[channel_id]["7d"]}]
                return

        raise AssertionError(f"Unexpected SQL in test double: {query!r} params={params!r}")

    def fetchall(self) -> list[dict[str, Any]]:
        """Return the last row set produced by execute.

        Parameters:
            None.

        Returns:
            List of dictionary rows.
        """

        return list(self._rows)

    def fetchone(self) -> dict[str, Any]:
        """Return the first row from the last result set.

        Parameters:
            None.

        Returns:
            A single dictionary row.
        """

        return dict(self._rows[0])

    def close(self) -> None:
        """Close the cursor stub.

        Parameters:
            None.

        Returns:
            None.
        """
        return None


class _FakeConnection:
    """Emulate the minimal MySQL connection behavior needed by the API.

    Parameters:
        channels: Channel rows returned from the channels query.
        counts: Per-channel usage counters keyed by channel id.

    Returns:
        A connection object with cursor/close methods.
    """

    def __init__(self, channels: list[dict[str, Any]], counts: dict[int, dict[str, int]]) -> None:
        self._channels = channels
        self._counts = counts

    def cursor(self, dictionary: bool = True) -> _FakeCursor:
        """Create a cursor stub.

        Parameters:
            dictionary: Kept for interface compatibility.

        Returns:
            A fake cursor.
        """

        return _FakeCursor(self._channels, self._counts)

    def close(self) -> None:
        """Close the connection stub.

        Parameters:
            None.

        Returns:
            None.
        """
        return None


async def _invoke_route(path: str, *args: Any, **kwargs: Any) -> Any:
    """Invoke a registered route directly.

    Parameters:
        path: Route path registered on the shimmed app.
        args: Positional arguments passed to the route handler.
        kwargs: Keyword arguments passed to the route handler.

    Returns:
        The route handler result.
    """

    handler = evolution_server.app.routes[path]
    result = handler(*args, **kwargs)
    if asyncio.iscoroutine(result):
        result = await result
    if hasattr(result, "model_dump"):
        return result.model_dump(mode="json")
    return result


def _set_health_and_journal_fixtures(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
    health_payload: dict[str, Any],
    journal_entries: list[dict[str, Any]],
) -> None:
    """Freeze health and journal outputs to deterministic fixtures.

    Parameters:
        monkeypatch: Pytest monkeypatch helper.
        tmp_path: Temporary directory for fixture files.
        health_payload: JSON payload returned by /api/health.
        journal_entries: JSONL rows returned by /api/journal.

    Returns:
        None.
    """

    fake_binary = tmp_path / "evolution-rs"
    fake_binary.write_text("stub", encoding="utf-8")
    fake_journal = tmp_path / ".evolution_journal.jsonl"
    fake_journal.write_text(
        "\n".join(json.dumps(entry, ensure_ascii=False) for entry in journal_entries) + "\n",
        encoding="utf-8",
    )

    monkeypatch.setattr(evolution_server, "RS_BIN", fake_binary)
    monkeypatch.setattr(evolution_server, "JOURNAL_PATH", fake_journal)
    monkeypatch.setattr(
        evolution_server.subprocess,
        "run",
        lambda *args, **kwargs: SimpleNamespace(
            returncode=0,
            stdout=json.dumps(health_payload, ensure_ascii=False),
            stderr="",
        ),
    )


def _install_fake_accounts_db(
    monkeypatch: pytest.MonkeyPatch,
    channels: list[dict[str, Any]],
    counts: dict[int, dict[str, int]],
) -> None:
    """Install a deterministic fake database connection.

    Parameters:
        monkeypatch: Pytest monkeypatch helper.
        channels: Channel rows returned by the fake channels table.
        counts: Per-channel usage counters keyed by channel id.

    Returns:
        None.
    """

    monkeypatch.setattr(
        evolution_server,
        "get_db_connection",
        lambda: _FakeConnection(channels=channels, counts=counts),
    )


def test_accounts_endpoint_hides_backend_connection_errors(monkeypatch: pytest.MonkeyPatch) -> None:
    """Verify backend connection failures do not leak raw infrastructure errors.

    Parameters:
        monkeypatch: Pytest monkeypatch helper.

    Returns:
        None.
    """

    secret_error = "secret-sentinel-41f8db"
    monkeypatch.setattr(
        evolution_server,
        "get_db_connection",
        lambda: (_ for _ in ()).throw(RuntimeError(secret_error)),
    )
    response = asyncio.run(_invoke_route("/api/accounts"))

    assert secret_error not in json.dumps(response, ensure_ascii=False)
    assert "secret-sentinel" not in json.dumps(response, ensure_ascii=False)


def test_accounts_endpoint_returns_masked_token_preview_only(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Verify token-bearing account data only exposes masked previews.

    Parameters:
        monkeypatch: Pytest monkeypatch helper.

    Returns:
        None.
    """

    monkeypatch.setenv(
        "COPILOT_TOKENS",
        "gho_live_aaaaaaaaaaaaaaaa,gho_live_bbbbbbbbbbbbbbbb",
    )
    _install_fake_accounts_db(
        monkeypatch,
        channels=[
            {"id": 11, "name": "alpha", "type": 15, "status": 1},
            {"id": 12, "name": "beta", "type": 15, "status": 1},
        ],
        counts={
            11: {"5h": 3, "7d": 17},
            12: {"5h": 8, "7d": 42},
        },
    )
    payload = asyncio.run(_invoke_route("/api/accounts"))
    assert isinstance(payload, dict)
    assert "accounts" in payload
    assert len(payload["accounts"]) == 2
    for account in payload["accounts"]:
        assert "token_preview" in account
        if "token" in account:
            assert isinstance(account["token"], dict)
            assert "masked_preview" in account["token"]
        serialized = json.dumps(account, ensure_ascii=False)
        assert "gho_live_aaaaaaaaaaaaaaaa" not in serialized
        assert "gho_live_bbbbbbbbbbbbbbbb" not in serialized


def test_refresh_endpoint_keeps_homepage_statistics_frozen(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """Verify token refresh does not mutate homepage-level stats outputs.

    Parameters:
        monkeypatch: Pytest monkeypatch helper.
        tmp_path: Temporary directory for deterministic fixtures.

    Returns:
        None.
    """

    _set_health_and_journal_fixtures(
        monkeypatch,
        tmp_path=tmp_path,
        health_payload={
            "critical_outliers": ["paper-writing"],
            "summary": {"avg_health": 92.8, "critical_skills": 1, "total_skills": 133},
            "skills": {"paper-writing": {"health_status": "Critical", "usage_30d": 1}},
            "ts": "2026-03-22T17:22:51.519711+00:00",
        },
        journal_entries=[
            {
                "ts": "2026-03-22T17:22:07.031022+00:00",
                "task": "Audit and optimize localhost:8000 app API integration and refresh/token behavior",
                "init": "execution-controller-coding",
                "final": "execution-controller-coding",
                "conf": 0.78,
                "diff": 4,
                "reroute": False,
                "struggle": 0,
                "notes": "checkpoint",
            }
        ],
    )
    _install_fake_accounts_db(
        monkeypatch,
        channels=[
            {"id": 21, "name": "alpha", "type": 15, "status": 1},
        ],
        counts={21: {"5h": 2, "7d": 9}},
    )
    before_health = asyncio.run(_invoke_route("/api/health"))
    before_journal = asyncio.run(_invoke_route("/api/journal", limit=1))
    body = asyncio.run(_invoke_route("/api/accounts/refresh"))
    after_health = asyncio.run(_invoke_route("/api/health"))
    after_journal = asyncio.run(_invoke_route("/api/journal", limit=1))

    assert body["stats_frozen"] is True
    assert body["refresh_scope"] == "accounts"
    assert after_health == before_health
    assert after_journal == before_journal


def test_accounts_summary_and_main_api_schema_are_structured(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Verify the accounts payload is a structured summary, not a flat list.

    Parameters:
        monkeypatch: Pytest monkeypatch helper.

    Returns:
        None.
    """

    monkeypatch.setenv("MAIN_API_NAME", "Codex Aggregated API")
    monkeypatch.setenv("MAIN_API_URL", "http://localhost:3000")
    monkeypatch.setenv(
        "COPILOT_TOKENS",
        "gho_live_aaaaaaaaaaaaaaaa,gho_live_bbbbbbbbbbbbbbbb,gho_live_cccccccccccccccc",
    )
    _install_fake_accounts_db(
        monkeypatch,
        channels=[
            {"id": 31, "name": "alpha", "type": 15, "status": 1},
            {"id": 32, "name": "beta", "type": 15, "status": 0},
            {"id": 33, "name": "gamma", "type": 15, "status": 1},
        ],
        counts={
            31: {"5h": 5, "7d": 15},
            32: {"5h": 0, "7d": 0},
            33: {"5h": 9, "7d": 21},
        },
    )
    payload = asyncio.run(_invoke_route("/api/accounts"))
    assert isinstance(payload, dict)
    assert set(payload) >= {"summary", "main_api", "accounts"}

    summary = payload["summary"]
    assert summary["total_accounts"] == 3
    assert summary["active_accounts"] == 2
    assert summary["frozen_accounts"] == 1
    assert summary["token_ready_accounts"] == 3

    main_api = payload["main_api"]
    assert main_api["name"] == "Codex Aggregated API"
    assert main_api["url"] == "http://localhost:3000"
    assert main_api["status"] in {"healthy", "degraded", "down"}

    accounts = payload["accounts"]
    assert isinstance(accounts, list)
    assert len(accounts) == 3
    for account in accounts:
        assert {"id", "name", "status", "used_5h", "used_7d", "token_preview"}.issubset(
            account
        )
        if "token" in account:
            assert isinstance(account["token"], dict)
            assert "masked_preview" in account["token"]
