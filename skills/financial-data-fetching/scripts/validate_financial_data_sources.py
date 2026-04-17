#!/usr/bin/env python3
"""Probe script to validate all financial data sources in the current environment."""
from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

try:
    import pandas as pd
except ImportError as exc:  # pragma: no cover - import guard
    raise SystemExit(
        "Missing Python package 'pandas'. Install it with "
        "`python3 -m pip install pandas` and retry."
    ) from exc

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from financial_data import MarketDataClient  # noqa: E402


def summarize_time_series(result) -> dict[str, Any]:
    """Summarize an OHLCV FetchResult for validation."""
    df = result.data.copy()
    ts_col = "timestamp"
    if ts_col not in df.columns:
        raise ValueError(f"missing {ts_col} column")
    ts = pd.to_datetime(df[ts_col], errors="coerce")
    if ts.isna().all():
        raise ValueError("could not parse timestamps")
    non_na = ts.dropna()
    if non_na.duplicated().any():
        raise ValueError("duplicate timestamps detected")
    return {
        **result.metadata(),
        "first_timestamp": str(non_na.iloc[0]),
        "last_timestamp": str(non_na.iloc[-1]),
        "monotonic_increasing": bool(non_na.is_monotonic_increasing),
        "last_close": float(df["close"].iloc[-1]),
        "last_volume": float(df["volume"].iloc[-1]),
    }


def summarize_generic(result) -> dict[str, Any]:
    """Summarize a non-OHLCV FetchResult (fundamentals, holders, capital)."""
    return {
        **result.metadata(),
        "preview_columns": list(result.data.columns[:10]),
    }


def run_probe(name: str, fn):
    """Run a single probe and return a structured result."""
    try:
        result = fn()
        if result.dataset == "ohlcv":
            details = summarize_time_series(result)
        elif result.dataset == "constituents":
            details = {
                **result.metadata(),
                "date": str(result.data["date"].iloc[0]),
                "row_count_check": int(len(result.data)),
            }
        elif result.dataset == "weights":
            weight_sum = float(result.data["weight"].astype(float).sum())
            details = {
                **result.metadata(),
                "latest_date": str(result.data["date"].iloc[0]),
                "row_count_check": int(len(result.data)),
                "weight_sum": weight_sum,
            }
            if abs(weight_sum - 100.0) > 0.2:
                raise ValueError(f"weight sum out of range: {weight_sum}")
        else:
            details = summarize_generic(result)
        return {"name": name, "ok": True, "details": details, "error": None}
    except Exception as exc:  # noqa: BLE001
        return {"name": name, "ok": False, "details": {}, "error": repr(exc)}


def main() -> None:
    """Run all probes and output JSON report."""
    client = MarketDataClient()
    probes = [
        # ── Existing OHLCV probes ───────────────────────────────
        ("crypto.binance.BTCUSDT.1h", lambda: client.fetch_ohlcv(market="crypto", exchange="binance", symbol="BTC/USDT", interval="1h", limit=5)),
        ("crypto.kraken.BTCUSD.1h", lambda: client.fetch_ohlcv(market="crypto", exchange="kraken", symbol="BTC/USD", interval="1h", limit=5)),
        ("crypto.coinbase.BTCUSD.1h", lambda: client.fetch_ohlcv(market="crypto", exchange="coinbase", symbol="BTC/USD", interval="1h", limit=5)),
        ("us.yfinance.AAPL.1h", lambda: client.fetch_ohlcv(market="us", symbol="AAPL", interval="1h", period="5d", source="yfinance")),
        ("us.stooq.AAPL.1d", lambda: client.fetch_ohlcv(market="us", symbol="AAPL", source="stooq")),
        ("cn.index.000300.1d", lambda: client.fetch_ohlcv(market="cn-index", symbol="000300")),
        ("cn.index.000905.1d", lambda: client.fetch_ohlcv(market="cn-index", symbol="000905")),
        ("cn.constituents.000300", lambda: client.fetch_cn_index_constituents(index_code="000300")),
        ("cn.constituents.000905", lambda: client.fetch_cn_index_constituents(index_code="000905")),
        ("cn.weights.000300", lambda: client.fetch_cn_index_weights(index_code="000300")),
        ("cn.weights.000905", lambda: client.fetch_cn_index_weights(index_code="000905")),

        # ── Fundamentals probes ─────────────────────────────────
        ("us.fundamentals.AAPL.key_metrics", lambda: client.fetch_fundamentals(market="us", symbol="AAPL", report="key_metrics")),
        ("us.fundamentals.AAPL.income", lambda: client.fetch_fundamentals(market="us", symbol="AAPL", report="income")),
        ("us.fundamentals.AAPL.balance", lambda: client.fetch_fundamentals(market="us", symbol="AAPL", report="balance")),
        ("cn.fundamentals.600519.key_metrics", lambda: client.fetch_fundamentals(market="cn", symbol="600519", report="key_metrics")),
        ("cn.fundamentals.600519.income", lambda: client.fetch_fundamentals(market="cn", symbol="600519", report="income")),

        # ── Holders probes ──────────────────────────────────────
        ("us.holders.AAPL.major", lambda: client.fetch_holders(market="us", symbol="AAPL", holder_type="major")),
        ("us.holders.AAPL.institutional", lambda: client.fetch_holders(market="us", symbol="AAPL", holder_type="institutional")),
        ("cn.holders.600519.top10", lambda: client.fetch_holders(market="cn", symbol="600519", holder_type="top10")),

        # ── Capital metrics probes ──────────────────────────────
        ("us.capital.AAPL", lambda: client.fetch_capital_metrics(market="us", symbol="AAPL")),
        ("cn.capital.600519", lambda: client.fetch_capital_metrics(market="cn", symbol="600519")),
    ]
    results = [run_probe(name, fn) for name, fn in probes]
    payload = {
        "generated_at_utc": pd.Timestamp.utcnow().isoformat(),
        "summary": {
            "probe_count": len(results),
            "ok_count": sum(1 for item in results if item["ok"]),
            "fail_count": sum(1 for item in results if not item["ok"]),
        },
        "results": results,
    }
    print(json.dumps(payload, ensure_ascii=False, default=str, indent=2))


if __name__ == "__main__":
    main()
