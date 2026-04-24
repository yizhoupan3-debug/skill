from __future__ import annotations

import importlib
import json
import subprocess
from dataclasses import dataclass
from datetime import datetime, timezone
from io import StringIO
from pathlib import Path
from typing import Any, Literal


def require_dependency(
    module_name: str,
    *,
    install_name: str | None = None,
    feature: str | None = None,
):
    try:
        return importlib.import_module(module_name)
    except ImportError as exc:  # pragma: no cover - import guard
        package_name = install_name or module_name
        feature_text = f" for {feature}" if feature else ""
        raise RuntimeError(
            f"Missing Python package '{package_name}' required{feature_text}. "
            f"Install it with `python3 -m pip install {package_name}` and retry."
        ) from exc


pd = require_dependency("pandas", feature="financial data normalization")
requests = require_dependency("requests", feature="HTTP financial data access")

SKILL_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = SKILL_ROOT.parents[1]
RUST_MANIFEST = REPO_ROOT / "rust_tools/financial_data_rs/Cargo.toml"


@dataclass(slots=True)
class FetchResult:
    dataset: str
    source: str
    market: str
    symbol: str
    interval: str | None
    timezone: str | None
    adjusted: bool | None
    fetched_at_utc: str
    data: pd.DataFrame
    notes: list[str]

    def metadata(self) -> dict[str, Any]:
        return {
            "dataset": self.dataset,
            "source": self.source,
            "market": self.market,
            "symbol": self.symbol,
            "interval": self.interval,
            "timezone": self.timezone,
            "adjusted": self.adjusted,
            "fetched_at_utc": self.fetched_at_utc,
            "row_count": int(len(self.data)),
            "columns": list(self.data.columns),
            "notes": self.notes,
        }

    def to_backtest_frame(
        self,
        schema: Literal["generic", "vectorbt", "backtrader"] = "generic",
    ) -> pd.DataFrame:
        if self.dataset != "ohlcv":
            raise ValueError(f"Backtest export only supports ohlcv datasets, got {self.dataset}")

        required = {"timestamp", "open", "high", "low", "close", "volume"}
        missing = required - set(self.data.columns)
        if missing:
            raise ValueError(f"Missing OHLCV columns for export: {sorted(missing)}")

        df = self.data.copy()
        df["timestamp"] = pd.to_datetime(df["timestamp"], errors="coerce")
        if df["timestamp"].isna().all():
            raise ValueError("Could not parse timestamp column")
        df = df.sort_values("timestamp").drop_duplicates(subset=["timestamp"]).reset_index(drop=True)

        if schema == "generic":
            ordered = [
                "timestamp",
                "open",
                "high",
                "low",
                "close",
                "volume",
                "symbol",
                "market",
                "source",
            ]
            optional = ["adj_close"]
            keep = [c for c in ordered + optional if c in df.columns]
            return df[keep].copy()

        if schema == "vectorbt":
            rename_map = {
                "open": "Open",
                "high": "High",
                "low": "Low",
                "close": "Close",
                "volume": "Volume",
                "adj_close": "Adj Close",
            }
            keep = [c for c in ["timestamp", "open", "high", "low", "close", "volume", "adj_close"] if c in df.columns]
            out = df[keep].rename(columns=rename_map).copy()
            out = out.set_index("timestamp")
            out.index.name = "timestamp"
            return out

        if schema == "backtrader":
            out = df[["timestamp", "open", "high", "low", "close", "volume"]].copy()
            out["openinterest"] = 0.0
            out = out.set_index("timestamp")
            out.index.name = "datetime"
            return out

        raise ValueError(f"Unsupported backtest schema: {schema}")

    def export_backtest(
        self,
        *,
        path: str | Path,
        schema: Literal["generic", "vectorbt", "backtrader"] = "generic",
        file_format: Literal["csv", "json", "parquet"] = "csv",
    ) -> Path:
        output_path = Path(path)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        frame = self.to_backtest_frame(schema=schema)

        if file_format == "csv":
            frame.to_csv(output_path, index=schema != "generic")
            return output_path

        if file_format == "json":
            frame.reset_index().to_json(output_path, orient="records", force_ascii=False, indent=2, date_format="iso")
            return output_path

        if file_format == "parquet":
            try:
                frame.to_parquet(output_path, index=schema != "generic")
            except Exception as exc:  # noqa: BLE001
                raise RuntimeError(
                    "Parquet export requires an engine such as pyarrow or fastparquet"
                ) from exc
            return output_path

        raise ValueError(f"Unsupported export format: {file_format}")


class MarketDataClient:
    """Reusable, no-token-first market data client for quant research."""

    def __init__(self, *, timeout: int = 20, user_agent: str | None = None) -> None:
        self.timeout = timeout
        self.session = requests.Session()
        self.session.headers.update(
            {
                "User-Agent": user_agent
                or "financial-data-fetching/1.0 (+https://local.skill)"
            }
        )

    @staticmethod
    def _now_utc() -> str:
        return datetime.now(timezone.utc).isoformat()

    @staticmethod
    def _normalize_index_code(index_code: str) -> str:
        key = index_code.strip().lower()
        mapping = {
            "000300": "000300",
            "sh000300": "000300",
            "sz399300": "000300",
            "csi300": "000300",
            "hs300": "000300",
            "沪深300": "000300",
            "000905": "000905",
            "sh000905": "000905",
            "sz399905": "000905",
            "csi500": "000905",
            "zz500": "000905",
            "中证500": "000905",
        }
        if key not in mapping:
            raise ValueError(f"Unsupported China index code: {index_code}")
        return mapping[key]

    @staticmethod
    def _normalize_yf_columns(df: pd.DataFrame) -> pd.DataFrame:
        if isinstance(df.columns, pd.MultiIndex):
            cols = []
            for col in df.columns:
                parts = [str(x) for x in col if x not in (None, "")]
                cols.append("_".join(parts).strip("_"))
            df = df.copy()
            df.columns = cols
        return df

    @staticmethod
    def _ensure_monotonic(df: pd.DataFrame, column: str) -> pd.DataFrame:
        out = df.sort_values(column).drop_duplicates(subset=[column]).reset_index(drop=True)
        return out

    def fetch_ohlcv(
        self,
        *,
        market: Literal["crypto", "us", "cn-index"],
        symbol: str,
        interval: str = "1d",
        source: str = "auto",
        exchange: str = "binance",
        limit: int = 200,
        period: str = "1mo",
        adjusted: bool = False,
    ) -> FetchResult:
        if market == "crypto":
            return self.fetch_crypto_ohlcv(
                exchange=exchange,
                symbol=symbol,
                interval=interval,
                limit=limit,
            )
        if market == "us":
            return self.fetch_us_ohlcv(
                symbol=symbol,
                interval=interval,
                period=period,
                source=source,  # type: ignore[arg-type]
                adjusted=adjusted,
            )
        if market == "cn-index":
            return self.fetch_cn_index_ohlcv(index_code=symbol)
        raise ValueError(f"Unsupported market: {market}")

    def fetch_crypto_ohlcv(
        self,
        *,
        exchange: str,
        symbol: str,
        interval: str = "1h",
        limit: int = 200,
    ) -> FetchResult:
        if exchange.lower() in {"binance", "coinbase", "kraken"}:
            return self._fetch_rust_ohlcv(
                market="crypto",
                symbol=symbol,
                exchange=exchange,
                interval=interval,
                limit=limit,
                period="1mo",
                source="auto",
                adjusted=False,
            )

        ccxt = require_dependency("ccxt", feature="crypto market data fetching")

        exchange_cls = getattr(ccxt, exchange)
        client = exchange_cls({"enableRateLimit": True})
        markets = client.load_markets()
        if symbol not in markets:
            raise ValueError(f"{symbol} not found on {exchange}")
        if not client.has.get("fetchOHLCV"):
            raise ValueError(f"{exchange} does not support fetchOHLCV")
        if interval not in (client.timeframes or {}):
            raise ValueError(f"{exchange} does not expose timeframe {interval}")

        rows = client.fetch_ohlcv(symbol, timeframe=interval, limit=limit)
        df = pd.DataFrame(rows, columns=["timestamp", "open", "high", "low", "close", "volume"])
        df["timestamp"] = pd.to_datetime(df["timestamp"], unit="ms", utc=True)
        df["symbol"] = symbol
        df["market"] = "crypto"
        df["source"] = f"ccxt:{exchange}"
        df = self._ensure_monotonic(df, "timestamp")
        return FetchResult(
            dataset="ohlcv",
            source=f"ccxt:{exchange}",
            market="crypto",
            symbol=symbol,
            interval=interval,
            timezone="UTC",
            adjusted=False,
            fetched_at_utc=self._now_utc(),
            data=df,
            notes=["exchange-native OHLCV via CCXT", f"market_count={len(markets)}"],
        )

    def fetch_us_ohlcv(
        self,
        *,
        symbol: str,
        interval: str = "1d",
        period: str = "1mo",
        source: Literal["auto", "yfinance", "stooq"] = "auto",
        adjusted: bool = False,
    ) -> FetchResult:
        if not adjusted:
            return self._fetch_rust_ohlcv(
                market="us",
                symbol=symbol,
                exchange="binance",
                interval=interval,
                limit=200,
                period=period,
                source=source,
                adjusted=adjusted,
            )

        attempts = [source] if source != "auto" else ["yfinance", "stooq"]
        last_error: Exception | None = None
        for candidate in attempts:
            try:
                if candidate == "yfinance":
                    return self._fetch_us_yfinance(symbol=symbol, interval=interval, period=period, adjusted=adjusted)
                if candidate == "stooq":
                    return self._fetch_us_stooq(symbol=symbol)
                raise ValueError(f"Unsupported U.S. source: {candidate}")
            except Exception as exc:  # noqa: BLE001
                last_error = exc
                if source != "auto":
                    raise
        raise RuntimeError(f"All U.S. data sources failed for {symbol}") from last_error

    def _fetch_rust_ohlcv(
        self,
        *,
        market: Literal["crypto", "us"],
        symbol: str,
        exchange: str,
        interval: str,
        limit: int,
        period: str,
        source: Literal["auto", "yfinance", "stooq"],
        adjusted: bool,
    ) -> FetchResult:
        rust_source = "yahoo" if source == "yfinance" else source
        cmd = [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(RUST_MANIFEST),
            "--",
            "ohlcv",
            "--market",
            market,
            "--symbol",
            symbol,
            "--exchange",
            exchange,
            "--interval",
            interval,
            "--limit",
            str(limit),
            "--period",
            period,
            "--source",
            rust_source,
            "--format",
            "json",
        ]
        if adjusted:
            cmd.append("--adjusted")

        completed = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            check=False,
            cwd=REPO_ROOT,
        )
        if completed.returncode != 0:
            message = completed.stderr.strip() or completed.stdout.strip()
            raise RuntimeError(message or f"Rust financial data CLI exited {completed.returncode}")

        payload = json.loads(completed.stdout)
        metadata = payload["metadata"]
        df = pd.DataFrame(payload["records"])
        if df.empty:
            raise ValueError(f"Rust financial data CLI returned empty OHLCV data for {symbol}")
        if "timestamp" in df.columns:
            df["timestamp"] = pd.to_datetime(df["timestamp"], utc=True)
        df = self._ensure_monotonic(df, "timestamp")
        notes = list(metadata.get("notes", []))
        notes.append("fetched by Rust core")
        return FetchResult(
            dataset=metadata["dataset"],
            source=metadata["source"],
            market=metadata["market"],
            symbol=metadata["symbol"],
            interval=metadata.get("interval"),
            timezone=metadata.get("timezone"),
            adjusted=metadata.get("adjusted"),
            fetched_at_utc=metadata["fetched_at_utc"],
            data=df,
            notes=notes,
        )

    def _fetch_us_yfinance(
        self,
        *,
        symbol: str,
        interval: str,
        period: str,
        adjusted: bool,
    ) -> FetchResult:
        yf = require_dependency("yfinance", feature="US equity market data fetching")

        raw = yf.download(
            symbol,
            period=period,
            interval=interval,
            auto_adjust=adjusted,
            progress=False,
            threads=False,
        )
        if raw is None or raw.empty:
            raise ValueError(f"yfinance returned empty data for {symbol}")
        df = self._normalize_yf_columns(raw).reset_index()
        time_col = "Datetime" if "Datetime" in df.columns else "Date"
        rename_map = {
            time_col: "timestamp",
            f"Open_{symbol}": "open",
            f"High_{symbol}": "high",
            f"Low_{symbol}": "low",
            f"Close_{symbol}": "close",
            f"Volume_{symbol}": "volume",
        }
        if not adjusted and f"Adj Close_{symbol}" in df.columns:
            rename_map[f"Adj Close_{symbol}"] = "adj_close"
        df = df.rename(columns=rename_map)
        keep = [c for c in ["timestamp", "open", "high", "low", "close", "adj_close", "volume"] if c in df.columns]
        df = df[keep]
        df["timestamp"] = pd.to_datetime(df["timestamp"], utc=True)
        df["symbol"] = symbol
        df["market"] = "us"
        df["source"] = "yfinance"
        df = self._ensure_monotonic(df, "timestamp")
        return FetchResult(
            dataset="ohlcv",
            source="yfinance",
            market="us",
            symbol=symbol,
            interval=interval,
            timezone="UTC",
            adjusted=adjusted,
            fetched_at_utc=self._now_utc(),
            data=df,
            notes=["no-token source", f"period={period}"],
        )

    def _fetch_us_stooq(self, *, symbol: str) -> FetchResult:
        normalized = f"{symbol.lower()}.us"
        url = f"https://stooq.com/q/d/l/?s={normalized}&i=d"
        response = self.session.get(url, timeout=self.timeout)
        response.raise_for_status()
        text = response.text.strip()
        if not text or text.lower().startswith("no data"):
            raise ValueError(f"stooq returned no data for {symbol}")
        df = pd.read_csv(StringIO(text))
        if df.empty:
            raise ValueError(f"stooq returned empty csv for {symbol}")
        df = df.rename(
            columns={
                "Date": "timestamp",
                "Open": "open",
                "High": "high",
                "Low": "low",
                "Close": "close",
                "Volume": "volume",
            }
        )
        df["timestamp"] = pd.to_datetime(df["timestamp"], utc=True)
        df["symbol"] = symbol.upper()
        df["market"] = "us"
        df["source"] = "stooq"
        df = self._ensure_monotonic(df, "timestamp")
        return FetchResult(
            dataset="ohlcv",
            source="stooq",
            market="us",
            symbol=symbol.upper(),
            interval="1d",
            timezone="UTC",
            adjusted=False,
            fetched_at_utc=self._now_utc(),
            data=df,
            notes=["no-token daily csv source", "daily-only"],
        )

    def fetch_cn_index_ohlcv(self, *, index_code: str) -> FetchResult:
        ak = require_dependency("akshare", feature="China index market data fetching")

        normalized = self._normalize_index_code(index_code)
        symbol = f"sh{normalized}"
        df = ak.stock_zh_index_daily(symbol=symbol).reset_index(drop=True)
        if df.empty:
            raise ValueError(f"AKShare returned empty index history for {index_code}")
        df = df.rename(
            columns={
                "date": "timestamp",
                "open": "open",
                "high": "high",
                "low": "low",
                "close": "close",
                "volume": "volume",
            }
        )
        df["timestamp"] = pd.to_datetime(df["timestamp"])
        df["symbol"] = normalized
        df["market"] = "cn-index"
        df["source"] = "akshare:stock_zh_index_daily"
        df = self._ensure_monotonic(df, "timestamp")
        return FetchResult(
            dataset="ohlcv",
            source="akshare:stock_zh_index_daily",
            market="cn-index",
            symbol=normalized,
            interval="1d",
            timezone="Asia/Shanghai",
            adjusted=False,
            fetched_at_utc=self._now_utc(),
            data=df,
            notes=["validated for CSI 300 / CSI 500 in current workspace"],
        )

    def fetch_cn_index_constituents(self, *, index_code: str) -> FetchResult:
        ak = require_dependency("akshare", feature="China index constituents fetching")

        normalized = self._normalize_index_code(index_code)
        df = ak.index_stock_cons_csindex(symbol=normalized).reset_index(drop=True)
        if df.empty:
            raise ValueError(f"AKShare returned empty constituents for {index_code}")
        df = df.rename(
            columns={
                "日期": "date",
                "指数代码": "index_code",
                "指数名称": "index_name",
                "成分券代码": "constituent_code",
                "成分券名称": "constituent_name",
                "交易所": "exchange",
            }
        )
        keep = [c for c in ["date", "index_code", "index_name", "constituent_code", "constituent_name", "exchange"] if c in df.columns]
        df = df[keep]
        expected = 300 if normalized == "000300" else 500
        if len(df) != expected:
            raise ValueError(f"Expected {expected} constituents for {normalized}, got {len(df)}")
        return FetchResult(
            dataset="constituents",
            source="akshare:index_stock_cons_csindex",
            market="cn-index",
            symbol=normalized,
            interval=None,
            timezone="Asia/Shanghai",
            adjusted=None,
            fetched_at_utc=self._now_utc(),
            data=df,
            notes=[f"expected_count={expected}", "validated public source"],
        )

    def fetch_cn_index_weights(self, *, index_code: str) -> FetchResult:
        ak = require_dependency("akshare", feature="China index weights fetching")

        normalized = self._normalize_index_code(index_code)
        df = ak.index_stock_cons_weight_csindex(symbol=normalized).reset_index(drop=True)
        if df.empty:
            raise ValueError(f"AKShare returned empty weights for {index_code}")
        df = df.rename(
            columns={
                "日期": "date",
                "指数代码": "index_code",
                "指数名称": "index_name",
                "成分券代码": "constituent_code",
                "成分券名称": "constituent_name",
                "交易所": "exchange",
                "权重": "weight",
            }
        )
        keep = [c for c in ["date", "index_code", "index_name", "constituent_code", "constituent_name", "exchange", "weight"] if c in df.columns]
        df = df[keep]
        df["date"] = pd.to_datetime(df["date"])
        latest_date = df["date"].max()
        latest = df[df["date"] == latest_date].copy().reset_index(drop=True)
        expected = 300 if normalized == "000300" else 500
        if len(latest) != expected:
            raise ValueError(f"Expected {expected} latest weights for {normalized}, got {len(latest)}")
        weight_sum = float(latest["weight"].astype(float).sum())
        if abs(weight_sum - 100.0) > 0.2:
            raise ValueError(f"Weight sum out of range for {normalized}: {weight_sum}")
        latest["date"] = latest["date"].dt.date.astype(str)
        return FetchResult(
            dataset="weights",
            source="akshare:index_stock_cons_weight_csindex",
            market="cn-index",
            symbol=normalized,
            interval=None,
            timezone="Asia/Shanghai",
            adjusted=None,
            fetched_at_utc=self._now_utc(),
            data=latest,
            notes=[f"expected_count={expected}", f"weight_sum={weight_sum:.3f}"],
        )

    # ── Fundamentals ────────────────────────────────────────────────

    def fetch_fundamentals(
        self,
        *,
        market: Literal["us", "cn"],
        symbol: str,
        report: Literal["income", "balance", "cashflow", "key_metrics"] = "key_metrics",
        freq: Literal["yearly", "quarterly"] = "yearly",
    ) -> FetchResult:
        """Fetch financial statement or key metrics for a single stock.

        Args:
            market: "us" or "cn".
            symbol: Ticker symbol (e.g. "AAPL" or "600519").
            report: Report type – income / balance / cashflow / key_metrics.
            freq: Yearly or quarterly (ignored for key_metrics).

        Returns:
            FetchResult with dataset="fundamentals".
        """
        if market == "us":
            return self._fetch_us_fundamentals(symbol=symbol, report=report, freq=freq)
        if market == "cn":
            return self._fetch_cn_fundamentals(symbol=symbol, report=report, freq=freq)
        raise ValueError(f"Unsupported market for fundamentals: {market}")

    def _fetch_us_fundamentals(
        self,
        *,
        symbol: str,
        report: str,
        freq: str,
    ) -> FetchResult:
        yf = require_dependency("yfinance", feature="US equity fundamentals fetching")

        ticker = yf.Ticker(symbol)

        if report == "key_metrics":
            info = ticker.info or {}
            metrics = {
                "symbol": symbol,
                "returnOnEquity": info.get("returnOnEquity"),
                "returnOnAssets": info.get("returnOnAssets"),
                "grossMargins": info.get("grossMargins"),
                "operatingMargins": info.get("operatingMargins"),
                "profitMargins": info.get("profitMargins"),
                "revenueGrowth": info.get("revenueGrowth"),
                "earningsGrowth": info.get("earningsGrowth"),
                "debtToEquity": info.get("debtToEquity"),
                "currentRatio": info.get("currentRatio"),
                "quickRatio": info.get("quickRatio"),
                "bookValue": info.get("bookValue"),
                "earningsPerShare": info.get("trailingEps"),
                "forwardEps": info.get("forwardEps"),
                "pegRatio": info.get("pegRatio"),
            }
            df = pd.DataFrame([metrics])
        elif report == "income":
            df = ticker.get_income_stmt(freq=freq)
            if df is None or df.empty:
                raise ValueError(f"yfinance returned empty income statement for {symbol}")
            df = df.T.reset_index().rename(columns={"index": "period"})
        elif report == "balance":
            df = ticker.get_balance_sheet(freq=freq)
            if df is None or df.empty:
                raise ValueError(f"yfinance returned empty balance sheet for {symbol}")
            df = df.T.reset_index().rename(columns={"index": "period"})
        elif report == "cashflow":
            df = ticker.get_cashflow(freq=freq)
            if df is None or df.empty:
                raise ValueError(f"yfinance returned empty cashflow for {symbol}")
            df = df.T.reset_index().rename(columns={"index": "period"})
        else:
            raise ValueError(f"Unsupported report type: {report}")

        return FetchResult(
            dataset="fundamentals",
            source="yfinance",
            market="us",
            symbol=symbol,
            interval=None,
            timezone=None,
            adjusted=None,
            fetched_at_utc=self._now_utc(),
            data=df,
            notes=[f"report={report}", f"freq={freq}"],
        )

    def _fetch_cn_fundamentals(
        self,
        *,
        symbol: str,
        report: str,
        freq: str,
    ) -> FetchResult:
        ak = require_dependency("akshare", feature="China fundamentals fetching")

        # Normalize symbol: strip exchange prefix if present
        code = symbol.strip()
        for prefix in ("sh", "sz", "SH", "SZ"):
            if code.startswith(prefix) and len(code) > 2:
                code = code[2:]
                break

        if report == "key_metrics":
            df = ak.stock_financial_analysis_indicator(symbol=code)
            if df is None or df.empty:
                raise ValueError(f"AKShare returned empty financial indicators for {symbol}")
            df = df.head(8)  # recent 8 periods
        elif report == "income":
            df = ak.stock_financial_report_sina(stock=code, symbol="利润表")
            if df is None or df.empty:
                raise ValueError(f"AKShare returned empty income statement for {symbol}")
        elif report == "balance":
            df = ak.stock_financial_report_sina(stock=code, symbol="资产负债表")
            if df is None or df.empty:
                raise ValueError(f"AKShare returned empty balance sheet for {symbol}")
        elif report == "cashflow":
            df = ak.stock_financial_report_sina(stock=code, symbol="现金流量表")
            if df is None or df.empty:
                raise ValueError(f"AKShare returned empty cashflow for {symbol}")
        else:
            raise ValueError(f"Unsupported report type: {report}")

        return FetchResult(
            dataset="fundamentals",
            source="akshare",
            market="cn",
            symbol=code,
            interval=None,
            timezone="Asia/Shanghai",
            adjusted=None,
            fetched_at_utc=self._now_utc(),
            data=df.reset_index(drop=True),
            notes=[f"report={report}", f"freq={freq}"],
        )

    # ── Holders ─────────────────────────────────────────────────────

    def fetch_holders(
        self,
        *,
        market: Literal["us", "cn"],
        symbol: str,
        holder_type: Literal["major", "institutional", "top10"] = "major",
    ) -> FetchResult:
        """Fetch shareholder / institutional holder data.

        Args:
            market: "us" or "cn".
            symbol: Ticker symbol.
            holder_type: "major" (US insider/institution %), "institutional" (US
                institution detail), "top10" (CN top-10 circulating shareholders).

        Returns:
            FetchResult with dataset="holders".
        """
        if market == "us":
            return self._fetch_us_holders(symbol=symbol, holder_type=holder_type)
        if market == "cn":
            return self._fetch_cn_holders(symbol=symbol, holder_type=holder_type)
        raise ValueError(f"Unsupported market for holders: {market}")

    def _fetch_us_holders(
        self,
        *,
        symbol: str,
        holder_type: str,
    ) -> FetchResult:
        yf = require_dependency("yfinance", feature="US equity holders fetching")

        ticker = yf.Ticker(symbol)

        if holder_type == "major":
            df = ticker.major_holders
            if df is None or (isinstance(df, pd.DataFrame) and df.empty):
                raise ValueError(f"yfinance returned empty major holders for {symbol}")
            if isinstance(df, pd.DataFrame):
                df = df.reset_index(drop=True)
                if df.shape[1] == 2:
                    df.columns = ["value", "description"]
        elif holder_type == "institutional":
            df = ticker.institutional_holders
            if df is None or (isinstance(df, pd.DataFrame) and df.empty):
                raise ValueError(f"yfinance returned empty institutional holders for {symbol}")
            df = df.reset_index(drop=True)
        elif holder_type == "top10":
            raise ValueError("top10 holder_type is only available for cn market")
        else:
            raise ValueError(f"Unsupported holder_type: {holder_type}")

        return FetchResult(
            dataset="holders",
            source="yfinance",
            market="us",
            symbol=symbol,
            interval=None,
            timezone=None,
            adjusted=None,
            fetched_at_utc=self._now_utc(),
            data=df,
            notes=[f"holder_type={holder_type}"],
        )

    def _fetch_cn_holders(
        self,
        *,
        symbol: str,
        holder_type: str,
    ) -> FetchResult:
        ak = require_dependency("akshare", feature="China holders fetching")

        code = symbol.strip()
        for prefix in ("sh", "sz", "SH", "SZ"):
            if code.startswith(prefix) and len(code) > 2:
                code = code[2:]
                break

        if holder_type in ("major", "top10"):
            df = ak.stock_gdfx_free_holding_detail_em(symbol=code)
            if df is None or (isinstance(df, pd.DataFrame) and df.empty):
                raise ValueError(f"AKShare returned empty holder data for {symbol}")
            df = df.reset_index(drop=True)
        elif holder_type == "institutional":
            df = ak.stock_gdfx_institution_holding_detail_em(symbol=code)
            if df is None or (isinstance(df, pd.DataFrame) and df.empty):
                raise ValueError(f"AKShare returned empty institutional holder data for {symbol}")
            df = df.reset_index(drop=True)
        else:
            raise ValueError(f"Unsupported holder_type for cn: {holder_type}")

        return FetchResult(
            dataset="holders",
            source="akshare",
            market="cn",
            symbol=code,
            interval=None,
            timezone="Asia/Shanghai",
            adjusted=None,
            fetched_at_utc=self._now_utc(),
            data=df,
            notes=[f"holder_type={holder_type}"],
        )

    # ── Capital Metrics ─────────────────────────────────────────────

    def fetch_capital_metrics(
        self,
        *,
        market: Literal["us", "cn"],
        symbol: str,
    ) -> FetchResult:
        """Fetch capital / valuation metrics for a single stock.

        Args:
            market: "us" or "cn".
            symbol: Ticker symbol.

        Returns:
            FetchResult with dataset="capital_metrics".
        """
        if market == "us":
            return self._fetch_us_capital(symbol=symbol)
        if market == "cn":
            return self._fetch_cn_capital(symbol=symbol)
        raise ValueError(f"Unsupported market for capital metrics: {market}")

    def _fetch_us_capital(self, *, symbol: str) -> FetchResult:
        yf = require_dependency("yfinance", feature="US capital metrics fetching")

        info = yf.Ticker(symbol).info or {}
        if not info:
            raise ValueError(f"yfinance returned empty info for {symbol}")

        metrics = {
            "symbol": symbol,
            "market_cap": info.get("marketCap"),
            "enterprise_value": info.get("enterpriseValue"),
            "pe_trailing": info.get("trailingPE"),
            "pe_forward": info.get("forwardPE"),
            "pb_ratio": info.get("priceToBook"),
            "ps_ratio": info.get("priceToSalesTrailing12Months"),
            "dividend_yield": info.get("dividendYield"),
            "dividend_rate": info.get("dividendRate"),
            "payout_ratio": info.get("payoutRatio"),
            "beta": info.get("beta"),
            "52w_high": info.get("fiftyTwoWeekHigh"),
            "52w_low": info.get("fiftyTwoWeekLow"),
            "50d_avg": info.get("fiftyDayAverage"),
            "200d_avg": info.get("twoHundredDayAverage"),
            "avg_volume": info.get("averageVolume"),
            "avg_volume_10d": info.get("averageDailyVolume10Day"),
            "shares_outstanding": info.get("sharesOutstanding"),
            "float_shares": info.get("floatShares"),
            "short_ratio": info.get("shortRatio"),
        }
        df = pd.DataFrame([metrics])

        return FetchResult(
            dataset="capital_metrics",
            source="yfinance",
            market="us",
            symbol=symbol,
            interval=None,
            timezone=None,
            adjusted=None,
            fetched_at_utc=self._now_utc(),
            data=df,
            notes=["snapshot from yfinance Ticker.info"],
        )

    def _fetch_cn_capital(self, *, symbol: str) -> FetchResult:
        ak = require_dependency("akshare", feature="China capital metrics fetching")

        code = symbol.strip()
        for prefix in ("sh", "sz", "SH", "SZ"):
            if code.startswith(prefix) and len(code) > 2:
                code = code[2:]
                break

        spot = ak.stock_zh_a_spot_em()
        if spot is None or spot.empty:
            raise ValueError("AKShare returned empty A-share spot data")

        row = spot[spot["代码"] == code]
        if row.empty:
            raise ValueError(f"Symbol {code} not found in A-share spot data")

        row = row.iloc[0]
        metrics = {
            "symbol": code,
            "name": row.get("名称"),
            "price": row.get("最新价"),
            "change_pct": row.get("涨跌幅"),
            "volume": row.get("成交量"),
            "turnover": row.get("成交额"),
            "amplitude": row.get("振幅"),
            "high": row.get("最高"),
            "low": row.get("最低"),
            "open": row.get("今开"),
            "prev_close": row.get("昨收"),
            "turnover_rate": row.get("换手率"),
            "pe_ratio": row.get("市盈率-动态"),
            "pb_ratio": row.get("市净率"),
            "total_market_cap": row.get("总市值"),
            "circulating_market_cap": row.get("流通市值"),
            "volume_ratio": row.get("量比"),
            "52w_high": row.get("60日涨跌幅"),
        }
        df = pd.DataFrame([metrics])

        return FetchResult(
            dataset="capital_metrics",
            source="akshare:stock_zh_a_spot_em",
            market="cn",
            symbol=code,
            interval=None,
            timezone="Asia/Shanghai",
            adjusted=None,
            fetched_at_utc=self._now_utc(),
            data=df,
            notes=["realtime snapshot filtered from stock_zh_a_spot_em"],
        )
