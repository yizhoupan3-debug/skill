//! Bounded UTF-8 file reads for hook hot paths (avoid full-file allocation before truncation).

use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Read at most `max_bytes` (UTF-8 **byte** budget) from `path`. Returns `None` on IO failure or empty.
///
/// Reads a small slack past `max_bytes` so a multibyte code point split at the budget does not drop
/// the entire prefix (unlike raw `take(max).read_to_string()`).
pub fn read_utf8_file_prefix(path: &Path, max_bytes: usize) -> Option<String> {
    if max_bytes == 0 {
        return None;
    }
    let mut file = File::open(path).ok()?;
    let read_cap = max_bytes.saturating_add(4);
    let mut raw = vec![0u8; read_cap];
    let n = file.read(&mut raw).ok()?;
    if n == 0 {
        return None;
    }
    raw.truncate(n);
    let mut cut = max_bytes.min(raw.len());
    while cut > 0 && std::str::from_utf8(&raw[..cut]).is_err() {
        cut -= 1;
    }
    if cut == 0 {
        return None;
    }
    let text = std::str::from_utf8(&raw[..cut])
        .expect("cut chosen on valid utf8 boundary")
        .to_string();
    if text.is_empty() { None } else { Some(text) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn read_utf8_file_prefix_respects_cap() {
        let dir =
            std::env::temp_dir().join(format!("router-rs-read-bounded-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("big.txt");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "{}", "x".repeat(10_000)).unwrap();
        let got = read_utf8_file_prefix(&path, 100).expect("read");
        assert_eq!(got.len(), 100);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_utf8_file_prefix_multibyte_boundary() {
        let dir =
            std::env::temp_dir().join(format!("router-rs-read-bounded-mb-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("mixed.txt");
        let body = format!("{}{}", "a".repeat(98), "中文更多");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "{body}").unwrap();
        let got = read_utf8_file_prefix(&path, 100).expect("must not drop summary at boundary");
        assert!(!got.is_empty());
        assert!(got.len() <= 100);
        assert!(got.starts_with(&"a".repeat(98)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_utf8_file_prefix_splits_on_char_boundary_at_cjk() {
        let dir =
            std::env::temp_dir().join(format!("router-rs-read-bounded-cjk-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("cjk.txt");
        let body = "你".repeat(20);
        std::fs::write(&path, body.as_bytes()).expect("write");
        let one_char = "你".len();
        let cap = one_char * 3 + 1;
        let got = read_utf8_file_prefix(&path, cap).expect("prefix at utf8 boundary");
        assert_eq!(got.chars().count(), 3);
        assert!(got.starts_with('你'));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
