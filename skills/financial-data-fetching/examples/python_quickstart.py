from pathlib import Path
import sys

ROOT = Path('/Users/joe/Documents/skill/skills/financial-data-fetching')
sys.path.insert(0, str(ROOT))

from financial_data import MarketDataClient


client = MarketDataClient()

# 1) U.S. stocks, no token
us_result = client.fetch_ohlcv(
    market='us',
    symbol='AAPL',
    interval='1h',
    period='5d',
    source='yfinance',
)
print('US metadata:', us_result.metadata())
print(us_result.to_backtest_frame(schema='vectorbt').head())

# 2) Crypto
crypto_result = client.fetch_ohlcv(
    market='crypto',
    exchange='binance',
    symbol='BTC/USDT',
    interval='1h',
    limit=10,
)
print('Crypto metadata:', crypto_result.metadata())

# 3) China index weights
cn_weights = client.fetch_cn_index_weights(index_code='000300')
print('CN weights metadata:', cn_weights.metadata())

# 4) Export
out = us_result.export_backtest(
    path='/tmp/aapl_vectorbt_example.parquet',
    schema='vectorbt',
    file_format='parquet',
)
print('Exported to:', out)
