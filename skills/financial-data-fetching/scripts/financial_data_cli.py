#!/usr/bin/env python3
"""Rust-first CLI for OHLCV/export, with Python kept for fundamentals surfaces."""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from financial_data import MarketDataClient  # noqa: E402

REPO_ROOT = ROOT.parents[1]
RUST_MANIFEST = REPO_ROOT / "rust_tools/financial_data_rs/Cargo.toml"
RUST_BIN = REPO_ROOT / "rust_tools/target/debug/financial_data_rs"


def build_parser() -> argparse.ArgumentParser:
    """Build argument parser with all subcommands."""
    parser = argparse.ArgumentParser(description="Reusable financial market-data CLI")
    sub = parser.add_subparsers(dest="command", required=True)

    ohlcv = sub.add_parser("ohlcv", help="Fetch OHLCV data")
    ohlcv.add_argument("--market", choices=["crypto", "us", "cn-index"], required=True)
    ohlcv.add_argument("--symbol", required=True)
    ohlcv.add_argument("--exchange", default="binance")
    ohlcv.add_argument("--interval", default="1d")
    ohlcv.add_argument("--limit", type=int, default=200)
    ohlcv.add_argument("--period", default="1mo")
    ohlcv.add_argument("--source", choices=["auto", "yfinance", "stooq"], default="auto")
    ohlcv.add_argument("--adjusted", action="store_true")
    ohlcv.add_argument("--format", choices=["json", "csv"], default="json")

    cons = sub.add_parser("constituents", help="Fetch China index constituents")
    cons.add_argument("--index", required=True)
    cons.add_argument("--format", choices=["json", "csv"], default="json")

    weights = sub.add_parser("weights", help="Fetch China index weights")
    weights.add_argument("--index", required=True)
    weights.add_argument("--format", choices=["json", "csv"], default="json")

    export = sub.add_parser("export", help="Fetch data and export unified backtest format")
    export.add_argument("--market", choices=["crypto", "us", "cn-index"], required=True)
    export.add_argument("--symbol", required=True)
    export.add_argument("--exchange", default="binance")
    export.add_argument("--interval", default="1d")
    export.add_argument("--limit", type=int, default=200)
    export.add_argument("--period", default="1mo")
    export.add_argument("--source", choices=["auto", "yfinance", "stooq"], default="auto")
    export.add_argument("--adjusted", action="store_true")
    export.add_argument("--schema", choices=["generic", "vectorbt", "backtrader"], default="generic")
    export.add_argument("--file-format", choices=["csv", "json", "parquet"], default="csv")
    export.add_argument("--output", required=True)
    export.add_argument("--metadata-output")

    fund = sub.add_parser("fundamentals", help="Fetch financial statements or key metrics")
    fund.add_argument("--market", choices=["us", "cn"], required=True)
    fund.add_argument("--symbol", required=True)
    fund.add_argument("--report", choices=["income", "balance", "cashflow", "key_metrics"], default="key_metrics")
    fund.add_argument("--freq", choices=["yearly", "quarterly"], default="yearly")
    fund.add_argument("--format", choices=["json", "csv"], default="json")

    hold = sub.add_parser("holders", help="Fetch shareholder / institutional holder data")
    hold.add_argument("--market", choices=["us", "cn"], required=True)
    hold.add_argument("--symbol", required=True)
    hold.add_argument("--type", choices=["major", "institutional", "top10"], default="major", dest="holder_type")
    hold.add_argument("--format", choices=["json", "csv"], default="json")

    cap = sub.add_parser("capital", help="Fetch capital / valuation metrics")
    cap.add_argument("--market", choices=["us", "cn"], required=True)
    cap.add_argument("--symbol", required=True)
    cap.add_argument("--format", choices=["json", "csv"], default="json")

    return parser


def emit_result(result, fmt: str) -> None:
    """Print a FetchResult in the requested format."""
    if fmt == "csv":
        print(result.data.to_csv(index=False))
        return
    payload = {
        "metadata": result.metadata(),
        "records": result.data.to_dict(orient="records"),
    }
    print(json.dumps(payload, ensure_ascii=False, default=str, indent=2))


def should_use_rust(args: argparse.Namespace) -> bool:
    """Route network-heavy paths to Rust when the feature set matches."""
    if args.command == "ohlcv":
        return args.market in {"crypto", "us"}
    if args.command == "export":
        return (
            args.market in {"crypto", "us"}
            and args.file_format in {"csv", "json"}
        )
    return False


def run_rust_cli(extra_args: list[str]) -> None:
    """Run the Rust core CLI and forward its output."""
    cmd = rust_command_prefix() + extra_args
    completed = subprocess.run(cmd, check=False)
    if completed.returncode != 0:
        raise SystemExit(completed.returncode)


def rust_command_prefix() -> list[str]:
    """Prefer a built Rust binary and fall back to cargo run."""
    override = os.environ.get("FINANCIAL_DATA_RS_BIN")
    if override:
        return [override]
    if RUST_BIN.exists():
        return [str(RUST_BIN)]
    return [
        "cargo",
        "run",
        "--quiet",
        "--manifest-path",
        str(RUST_MANIFEST),
        "--",
    ]


def rust_args_for_ohlcv(args: argparse.Namespace) -> list[str]:
    """Translate argparse Namespace to Rust CLI flags."""
    out = [
        "ohlcv",
        "--market",
        args.market,
        "--symbol",
        args.symbol,
        "--exchange",
        args.exchange,
        "--interval",
        args.interval,
        "--limit",
        str(args.limit),
        "--period",
        args.period,
        "--source",
        "yahoo" if args.source == "yfinance" else args.source,
        "--format",
        args.format,
    ]
    if args.adjusted:
        out.append("--adjusted")
    return out


def rust_args_for_export(args: argparse.Namespace) -> list[str]:
    """Translate export arguments to Rust CLI flags."""
    out = [
        "export",
        "--market",
        args.market,
        "--symbol",
        args.symbol,
        "--exchange",
        args.exchange,
        "--interval",
        args.interval,
        "--limit",
        str(args.limit),
        "--period",
        args.period,
        "--source",
        "yahoo" if args.source == "yfinance" else args.source,
        "--schema",
        args.schema,
        "--file-format",
        args.file_format,
        "--output",
        args.output,
    ]
    if args.adjusted:
        out.append("--adjusted")
    if args.metadata_output:
        out.extend(["--metadata-output", args.metadata_output])
    return out


def main() -> None:
    """Entry point for the CLI."""
    args = build_parser().parse_args()

    if should_use_rust(args):
        if args.command == "ohlcv":
            run_rust_cli(rust_args_for_ohlcv(args))
            return
        if args.command == "export":
            run_rust_cli(rust_args_for_export(args))
            return

    client = MarketDataClient()

    if args.command == "ohlcv":
        result = client.fetch_ohlcv(
            market=args.market,
            symbol=args.symbol,
            exchange=args.exchange,
            interval=args.interval,
            limit=args.limit,
            period=args.period,
            source=args.source,
            adjusted=args.adjusted,
        )
        emit_result(result, args.format)
        return

    if args.command == "constituents":
        emit_result(client.fetch_cn_index_constituents(index_code=args.index), args.format)
        return

    if args.command == "weights":
        emit_result(client.fetch_cn_index_weights(index_code=args.index), args.format)
        return

    if args.command == "export":
        result = client.fetch_ohlcv(
            market=args.market,
            symbol=args.symbol,
            exchange=args.exchange,
            interval=args.interval,
            limit=args.limit,
            period=args.period,
            source=args.source,
            adjusted=args.adjusted,
        )
        output_path = result.export_backtest(
            path=args.output,
            schema=args.schema,
            file_format=args.file_format,
        )
        payload = {
            "output": str(output_path),
            "schema": args.schema,
            "file_format": args.file_format,
            "metadata": result.metadata(),
        }
        if args.metadata_output:
            Path(args.metadata_output).write_text(
                json.dumps(payload["metadata"], ensure_ascii=False, indent=2),
                encoding="utf-8",
            )
            payload["metadata_output"] = args.metadata_output
        print(json.dumps(payload, ensure_ascii=False, indent=2))
        return

    if args.command == "fundamentals":
        result = client.fetch_fundamentals(
            market=args.market,
            symbol=args.symbol,
            report=args.report,
            freq=args.freq,
        )
        emit_result(result, args.format)
        return

    if args.command == "holders":
        result = client.fetch_holders(
            market=args.market,
            symbol=args.symbol,
            holder_type=args.holder_type,
        )
        emit_result(result, args.format)
        return

    if args.command == "capital":
        result = client.fetch_capital_metrics(
            market=args.market,
            symbol=args.symbol,
        )
        emit_result(result, args.format)
        return


if __name__ == "__main__":
    main()
