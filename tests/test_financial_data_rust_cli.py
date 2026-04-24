from __future__ import annotations

import subprocess
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]
FINANCIAL_DATA_MANIFEST = PROJECT_ROOT / "rust_tools" / "financial_data_rs" / "Cargo.toml"


def run_financial_data_error(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(FINANCIAL_DATA_MANIFEST),
            "--",
            *args,
        ],
        check=False,
        capture_output=True,
        text=True,
    )


def test_financial_data_rejects_zero_limit() -> None:
    result = run_financial_data_error(
        "ohlcv",
        "--market",
        "crypto",
        "--symbol",
        "BTC/USDT",
        "--limit",
        "0",
    )

    assert result.returncode != 0
    assert "--limit must be greater than zero" in result.stderr


def test_financial_data_rejects_adjusted_stooq() -> None:
    result = run_financial_data_error(
        "ohlcv",
        "--market",
        "us",
        "--symbol",
        "AAPL",
        "--source",
        "stooq",
        "--adjusted",
    )

    assert result.returncode != 0
    assert "Stooq does not support adjusted OHLCV" in result.stderr
