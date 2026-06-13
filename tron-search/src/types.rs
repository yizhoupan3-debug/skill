use serde::{Deserialize, Serialize};

/// Aggregated search report for an address
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchReport {
    pub address: String,
    pub network: String,
    pub account: Option<AccountInfo>,
    pub transactions: Vec<Transaction>,
    pub trc20_transfers: Vec<Trc20Transfer>,
    pub internal_transactions: Vec<InternalTransaction>,
    pub blocks_produced: Vec<BlockProduced>,
}

/// Account information from TronScan
#[derive(Debug, Serialize, Deserialize)]
pub struct AccountInfo {
    pub address: String,
    #[serde(default)]
    pub balance: i64,
    #[serde(default)]
    pub create_time: Option<i64>,
    #[serde(default)]
    pub latest_operation_time: Option<i64>,
    #[serde(default)]
    pub bandwidth: Option<i64>,
    #[serde(default)]
    pub energy: Option<i64>,
    #[serde(default)]
    pub trc20: Vec<serde_json::Map<String, serde_json::Value>>,
}

/// TRX transfer transaction
#[derive(Debug, Serialize, Deserialize)]
pub struct Transaction {
    pub hash: String,
    pub owner_address: String,
    pub to_address: String,
    #[serde(default)]
    pub amount: Option<i64>,
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub contract_ret: Option<String>,
    #[serde(default)]
    pub confirmed: bool,
}

/// TRC20 token transfer
#[derive(Debug, Serialize, Deserialize)]
pub struct Trc20Transfer {
    pub transaction_id: String,
    pub from: String,
    pub to: String,
    pub value: String,
    #[serde(default)]
    pub block_ts: i64,
    #[serde(default)]
    pub token_info: TokenInfo,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TokenInfo {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub decimals: Option<i64>,
}

/// Internal (contract-to-contract) transaction
#[derive(Debug, Serialize, Deserialize)]
pub struct InternalTransaction {
    pub hash: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub amount: Option<i64>,
    #[serde(default)]
    pub note: Option<String>,
}

/// Block produced by address (for SR/witness addresses)
#[derive(Debug, Serialize, Deserialize)]
pub struct BlockProduced {
    #[serde(default)]
    pub number: i64,
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub tx_count: Option<i64>,
}
