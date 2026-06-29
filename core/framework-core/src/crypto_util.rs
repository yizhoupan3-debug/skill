use sha2::{Digest, Sha256};

/// 将字节切片编码为小写十六进制字符串。
pub fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = std::fmt::Write::write_fmt(&mut s, format_args!("{:02x}", byte));
    }
    s
}

/// SHA-256 前 16 字节的小写十六进制摘要（32 hex chars）。
pub fn short_hash(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    hex_lower(&digest[..16])
}
