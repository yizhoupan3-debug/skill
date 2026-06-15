use anyhow::{Context, Result};
use reqwest::Client;

use crate::types::*;

const TRONGRID_API: &str = "https://api.trongrid.io";

/// Base58 alphabet for TRON address decoding
const BASE58_ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// Search for all occurrences of an address across the TRON blockchain
pub async fn search_address(
    address: &str,
    limit: i64,
    testnet: bool,
    _json_mode: bool,
) -> Result<SearchReport> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let network = if testnet { "Shasta Testnet" } else { "Mainnet" };
    let base_url = if testnet {
        "https://api.shasta.trongrid.io"
    } else {
        TRONGRID_API
    };

    // Convert base58check address to hex for wallet API
    let hex_address = base58check_to_hex(address).ok();

    let mut report = SearchReport {
        address: address.to_string(),
        network: network.to_string(),
        account: None,
        transactions: vec![],
        trc20_transfers: vec![],
        internal_transactions: vec![],
        blocks_produced: vec![],
    };

    // 1. Account info via /wallet/getaccount (POST, hex address)
    eprintln!("  Fetching account info...");
    if let Some(ref hex) = hex_address {
        match fetch_account_info(&client, base_url, address, hex).await {
            Ok(info) => report.account = Some(info),
            Err(e) => eprintln!("  Warning: Could not fetch account info: {e}"),
        }
    }

    // 2. Transactions via /v1/accounts/{address}/transactions
    eprintln!("  Fetching transactions...");
    match fetch_transactions(&client, base_url, address, limit).await {
        Ok(txs) => report.transactions = txs,
        Err(e) => eprintln!("  Warning: Could not fetch transactions: {e}"),
    }

    // 3. TRC20 transfers via /v1/accounts/{address}/transactions/trc20
    eprintln!("  Fetching TRC20 transfers...");
    match fetch_trc20_transfers(&client, base_url, address, limit).await {
        Ok(transfers) => report.trc20_transfers = transfers,
        Err(e) => eprintln!("  Warning: Could not fetch TRC20 transfers: {e}"),
    }

    // 4. Internal transactions (extracted from transaction details)
    eprintln!("  Fetching internal transactions...");
    match fetch_internal_txs(&client, base_url, address, limit).await {
        Ok(txs) => report.internal_transactions = txs,
        Err(e) => eprintln!("  Warning: Could not fetch internal transactions: {e}"),
    }

    Ok(report)
}

/// Fetch account information via /wallet/getaccount
async fn fetch_account_info(
    client: &Client,
    base: &str,
    base58_address: &str,
    hex_address: &str,
) -> Result<AccountInfo> {
    let url = format!("{base}/wallet/getaccount");
    let body = serde_json::json!({
        "address": hex_address,
        "visible": false
    });

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .with_context(|| format!("POST {url}"))?;

    if !resp.status().is_success() {
        anyhow::bail!("HTTP {}", resp.status());
    }

    let data: serde_json::Value = resp.json().await?;

    // If empty object, account doesn't exist
    if data.as_object().is_none_or(|m| m.is_empty()) {
        return Ok(AccountInfo {
            address: base58_address.to_string(),
            balance: 0,
            create_time: None,
            latest_operation_time: None,
            bandwidth: None,
            energy: None,
            trc20: vec![],
        });
    }

    let balance = data["balance"].as_i64().unwrap_or(0);
    let create_time = data["create_time"].as_i64();
    let latest_op = data["latest_opration_time"]
        .as_i64()
        .or_else(|| data["latest_operation_time"].as_i64());

    // Extract bandwidth from frozen or free net
    let bandwidth = data["free_net_usage"]
        .as_i64()
        .or_else(|| data["net_usage"].as_i64());

    // Extract energy
    let energy = data["account_resource"]["energy_usage"].as_i64();

    // Extract TRC20 tokens
    let trc20 = data["assetV2"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|t| {
                    let key = t["key"].as_str()?;
                    let value = t["value"].as_i64()?;
                    let mut m = serde_json::Map::new();
                    m.insert(
                        "token_id".into(),
                        serde_json::Value::String(key.to_string()),
                    );
                    m.insert(
                        "balance".into(),
                        serde_json::Value::String(value.to_string()),
                    );
                    Some(m)
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(AccountInfo {
        address: base58_address.to_string(),
        balance,
        create_time,
        latest_operation_time: latest_op,
        bandwidth,
        energy,
        trc20,
    })
}

/// Fetch transactions via /v1/accounts/{address}/transactions
async fn fetch_transactions(
    client: &Client,
    base: &str,
    address: &str,
    limit: i64,
) -> Result<Vec<Transaction>> {
    let url = format!(
        "{base}/v1/accounts/{address}/transactions?limit={limit}&order_by=block_timestamp,desc"
    );
    let resp = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;

    if !resp.status().is_success() {
        anyhow::bail!("HTTP {}", resp.status());
    }

    let body: serde_json::Value = resp.json().await?;

    if body["success"] == serde_json::Value::Bool(false) {
        let err = body["error"].as_str().unwrap_or("unknown error");
        anyhow::bail!("API error: {err}");
    }

    let empty = vec![];
    let data = body["data"].as_array().unwrap_or(&empty);

    let mut txs = Vec::new();
    for item in data {
        let hash = item["txID"]
            .as_str()
            .or_else(|| item["hash"].as_str())
            .unwrap_or("")
            .to_string();

        // Extract owner and to addresses from raw_data contract
        let contract = &item["raw_data"]["contract"][0];
        let params = &contract["parameter"]["value"];

        let owner = try_hex_to_base58(params["owner_address"].as_str().unwrap_or(""));

        // For TransferContract, to_address and amount are direct
        // For TriggerSmartContract, extract from data field
        let contract_type = contract["type"].as_str().unwrap_or("");
        let (to, amount) = match contract_type {
            "TransferContract" => {
                let to = try_hex_to_base58(params["to_address"].as_str().unwrap_or(""));
                let amt = params["amount"].as_i64();
                (to, amt)
            }
            "TriggerSmartContract" => {
                let to = try_hex_to_base58(params["contract_address"].as_str().unwrap_or(""));
                // Try to decode amount from data field (transfer function)
                let data_hex = params["data"].as_str().unwrap_or("");
                let amt = decode_trc20_amount(data_hex);
                (to, amt)
            }
            _ => {
                let to = try_hex_to_base58(
                    params["to_address"]
                        .as_str()
                        .or_else(|| params["contract_address"].as_str())
                        .unwrap_or(""),
                );
                let amt = params["amount"].as_i64();
                (to, amt)
            }
        };

        let timestamp = item["block_timestamp"].as_i64().unwrap_or(0);
        let contract_ret = item["ret"][0]["contractRet"].as_str().map(String::from);

        txs.push(Transaction {
            hash,
            owner_address: owner,
            to_address: to,
            amount,
            timestamp,
            contract_ret,
            confirmed: true,
        });
    }

    Ok(txs)
}

/// Fetch TRC20 token transfers via /v1/accounts/{address}/transactions/trc20
async fn fetch_trc20_transfers(
    client: &Client,
    base: &str,
    address: &str,
    limit: i64,
) -> Result<Vec<Trc20Transfer>> {
    let url = format!(
        "{base}/v1/accounts/{address}/transactions/trc20?limit={limit}&order_by=block_timestamp,desc"
    );
    let resp = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;

    if !resp.status().is_success() {
        anyhow::bail!("HTTP {}", resp.status());
    }

    let body: serde_json::Value = resp.json().await?;

    if body["success"] == serde_json::Value::Bool(false) {
        let err = body["error"].as_str().unwrap_or("unknown error");
        anyhow::bail!("API error: {err}");
    }

    let empty = vec![];
    let data = body["data"].as_array().unwrap_or(&empty);

    let mut transfers = Vec::new();
    for item in data {
        let tx_id = item["transaction_id"].as_str().unwrap_or("").to_string();
        let from = item["from"].as_str().unwrap_or("").to_string();
        let to = item["to"].as_str().unwrap_or("").to_string();
        let value = item["value"].as_str().unwrap_or("0").to_string();
        let block_ts = item["block_timestamp"].as_i64().unwrap_or(0);

        let token_info = item
            .get("token_info")
            .map(|ti| TokenInfo {
                name: ti["name"].as_str().map(String::from),
                symbol: ti["symbol"].as_str().map(String::from),
                decimals: ti["decimals"].as_i64(),
            })
            .unwrap_or_default();

        transfers.push(Trc20Transfer {
            transaction_id: tx_id,
            from,
            to,
            value,
            block_ts,
            token_info,
        });
    }

    Ok(transfers)
}

/// Fetch internal transactions from transaction details
async fn fetch_internal_txs(
    client: &Client,
    base: &str,
    address: &str,
    limit: i64,
) -> Result<Vec<InternalTransaction>> {
    // Internal transactions are available via the transaction details
    // We fetch recent transactions and check for internal_transactions field
    let url = format!(
        "{base}/v1/accounts/{address}/transactions?limit={limit}&order_by=block_timestamp,desc"
    );
    let resp = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?;

    if !resp.status().is_success() {
        anyhow::bail!("HTTP {}", resp.status());
    }

    let body: serde_json::Value = resp.json().await?;
    let empty = vec![];
    let data = body["data"].as_array().unwrap_or(&empty);

    let mut internal_txs = Vec::new();
    for item in data {
        let hash = item["txID"]
            .as_str()
            .or_else(|| item["hash"].as_str())
            .unwrap_or("")
            .to_string();

        if let Some(internals) = item["internal_transactions"].as_array() {
            for it in internals {
                let from = it["from"].as_str().unwrap_or("").to_string();
                let to = it["to"].as_str().unwrap_or("").to_string();
                let amount = it["amount"].as_i64();
                let note = it["note"]
                    .as_str()
                    .or_else(|| it["extra"].as_str())
                    .map(String::from);

                internal_txs.push(InternalTransaction {
                    hash: hash.clone(),
                    from,
                    to,
                    amount,
                    note,
                });
            }
        }
    }

    Ok(internal_txs)
}

/// Decode TRC20 transfer amount from the data hex field
/// The transfer function signature is `a9059cbb` followed by:
/// - 32 bytes (64 hex chars) for the to address (padded)
/// - 32 bytes (64 hex chars) for the amount
fn decode_trc20_amount(data_hex: &str) -> Option<i64> {
    // Remove "0x" prefix if present
    let data = data_hex.strip_prefix("0x").unwrap_or(data_hex);

    // Must be at least 8 (function sig) + 64 (to addr) + 64 (amount) = 136 chars
    if data.len() < 136 {
        return None;
    }

    // Check function signature (transfer = a9059cbb)
    if !data.starts_with("a9059cbb") {
        return None;
    }

    // Extract amount (last 64 hex chars before any trailing data)
    let amount_hex = &data[72..136];
    i64::from_str_radix(amount_hex, 16).ok()
}

/// Convert a base58check TRON address to hex (41-prefixed)
fn base58check_to_hex(address: &str) -> Result<String> {
    let mut result: Vec<u8> = Vec::with_capacity(25);

    for &c in address.as_bytes() {
        let val = BASE58_ALPHABET
            .iter()
            .position(|&b| b == c)
            .ok_or_else(|| anyhow::anyhow!("Invalid base58 character: {}", c as char))?;

        let mut carry = val;
        for byte in result.iter_mut() {
            carry += (*byte as usize) * 58;
            *byte = (carry & 0xff) as u8;
            carry >>= 8;
        }
        while carry > 0 {
            result.push((carry & 0xff) as u8);
            carry >>= 8;
        }
    }

    // Reverse to big-endian
    result.reverse();

    // Add leading zeros for leading '1' characters
    for &c in address.as_bytes() {
        if c == b'1' {
            result.insert(0, 0);
        } else {
            break;
        }
    }

    // Should be 25 bytes: 1 byte version + 20 bytes hash + 4 bytes checksum
    if result.len() != 25 {
        anyhow::bail!(
            "Invalid address length: expected 25 bytes, got {}",
            result.len()
        );
    }

    // Verify checksum (last 4 bytes)
    let (payload, checksum) = result.split_at(21);
    let hash1 = sha256(payload);
    let hash2 = sha256(&hash1);
    if checksum != &hash2[..4] {
        anyhow::bail!("Invalid checksum");
    }

    // Convert to hex string (skip version byte 0x41 for mainnet)
    let hex_str: String = result[..21].iter().map(|b| format!("{b:02x}")).collect();
    Ok(hex_str)
}

/// Convert a hex-encoded TRON address (41-prefixed) to base58check
pub fn hex_to_base58(hex_str: &str) -> Result<String> {
    let bytes: Vec<u8> = (0..hex_str.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex_str[i..i + 2], 16))
        .collect::<std::result::Result<Vec<u8>, _>>()
        .map_err(|e| anyhow::anyhow!("Invalid hex: {e}"))?;

    if bytes.len() != 21 {
        anyhow::bail!(
            "Invalid hex address length: expected 21 bytes, got {}",
            bytes.len()
        );
    }

    // Compute double SHA-256 checksum
    let hash1 = sha256(&bytes);
    let hash2 = sha256(&hash1);
    let checksum = &hash2[..4];

    // Combine payload + checksum
    let mut full = bytes.to_vec();
    full.extend_from_slice(checksum);

    // Encode to base58
    let mut result = Vec::new();
    let mut num = full.clone();

    // Count leading zero bytes → leading '1's
    let leading_zeros = num.iter().take_while(|&&b| b == 0).count();

    // Convert from base256 to base58
    let mut start = 0;
    while start < num.len() {
        let mut remainder: u32 = 0;
        let mut new_start = num.len();

        for i in start..num.len() {
            let val = (remainder << 8) | num[i] as u32;
            num[i] = (val / 58) as u8;
            remainder = val % 58;

            if num[i] != 0 && new_start == num.len() {
                new_start = i;
            }
        }

        result.push(BASE58_ALPHABET[remainder as usize]);
        start = new_start;
    }

    // Add leading '1's for leading zero bytes
    result.resize(result.len() + leading_zeros, b'1');

    result.reverse();
    Ok(String::from_utf8(result)?)
}

/// Try to convert a hex address to base58, or return the original string
pub fn try_hex_to_base58(addr: &str) -> String {
    // If it looks like a hex address (41-prefixed, 42 chars)
    if addr.len() == 42 && addr.starts_with("41") {
        hex_to_base58(addr).unwrap_or_else(|_| addr.to_string())
    } else {
        addr.to_string()
    }
}

/// Simple SHA-256 implementation (double-hash needed for base58check)
fn sha256(data: &[u8]) -> Vec<u8> {
    sha256_inner(data)
}

fn sha256_inner(data: &[u8]) -> Vec<u8> {
    // Initial hash values
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    // Round constants
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    // Pre-processing: adding padding bits
    let mut msg = data.to_vec();
    let bit_len = (msg.len() as u64) * 8;
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    // Process each 512-bit (64-byte) chunk
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut result = Vec::with_capacity(32);
    for &word in &h {
        result.extend_from_slice(&word.to_be_bytes());
    }
    result
}
