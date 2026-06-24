//! Columnar JSON registry parsing utilities.
//!
//! The skill routing registries (RUNTIME, MANIFEST, INDEX) use a compact
//! "columnar array-of-arrays" format: a `keys` array defines column names,
//! and a `skills` array contains rows where each row is a positional array.
//!
//! This module provides the shared parsing logic used by `registry.rs`,
//! `runtime-infra`, and potentially `routing-engine`.

use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// ---------------------------------------------------------------------------
// Key index
// ---------------------------------------------------------------------------

/// Parse the `keys` array from a columnar JSON document into a Vec of column names.
pub fn parse_columnar_keys(doc: &Value) -> Vec<String> {
    doc["keys"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

/// Find the positional index of a column by name.
pub fn key_index(keys: &[String], column: &str) -> Option<usize> {
    keys.iter().position(|k| k == column)
}

// ---------------------------------------------------------------------------
// Row extraction
// ---------------------------------------------------------------------------

/// Extract all rows from a columnar document as `slug → row_values`.
pub fn load_columnar_rows(doc: &Value) -> HashMap<String, Vec<Value>> {
    let keys = parse_columnar_keys(doc);
    let slug_idx = key_index(&keys, "slug");

    let mut rows = HashMap::new();
    if let Some(skills) = doc["skills"].as_array() {
        for row in skills {
            if let Some(idx) = slug_idx {
                if let Some(slug) = row.get(idx).and_then(|v| v.as_str()) {
                    let values: Vec<Value> = row
                        .as_array()
                        .map(|a| a.to_vec())
                        .unwrap_or_default();
                    rows.insert(slug.to_string(), values);
                }
            }
        }
    }
    rows
}

/// Extract just the slug set from a columnar document.
pub fn extract_slugs(doc: &Value) -> Vec<String> {
    let keys = parse_columnar_keys(doc);
    let slug_idx = key_index(&keys, "slug");

    doc["skills"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    slug_idx
                        .and_then(|i| row.get(i))
                        .and_then(|v| v.as_str())
                        .map(String::from)
                })
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Column accessors
// ---------------------------------------------------------------------------

/// Get a raw `Value` from a row by column name.
pub fn col_val(row: &[Value], keys: &[String], column: &str) -> Option<Value> {
    let idx = key_index(keys, column)?;
    row.get(idx).cloned()
}

/// Get a `&str` from a row by column name.
pub fn col_str<'a>(row: &'a [Value], keys: &[String], column: &str) -> Option<&'a str> {
    let idx = key_index(keys, column)?;
    row.get(idx)?.as_str()
}

/// Get a `String` from a row by column name.
pub fn col_string(row: &[Value], keys: &[String], column: &str) -> Option<String> {
    col_str(row, keys, column).map(String::from)
}

/// Get a `Vec<String>` from a JSON array cell by column name.
pub fn col_str_vec(row: &[Value], keys: &[String], column: &str) -> Vec<String> {
    col_val(row, keys, column)
        .and_then(|v| {
            v.as_array().map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
        })
        .unwrap_or_default()
}

/// Get an `f64` from a row by column name.
pub fn col_f64(row: &[Value], keys: &[String], column: &str) -> Option<f64> {
    col_val(row, keys, column).and_then(|v| v.as_f64())
}

/// Get a `u64` from a row by column name.
pub fn col_u64(row: &[Value], keys: &[String], column: &str) -> Option<u64> {
    col_val(row, keys, column).and_then(|v| v.as_u64())
}

/// Get a `bool` from a row by column name.
pub fn col_bool(row: &[Value], keys: &[String], column: &str) -> Option<bool> {
    col_val(row, keys, column).and_then(|v| v.as_bool())
}

// ---------------------------------------------------------------------------
// File-level loaders
// ---------------------------------------------------------------------------

/// Load a columnar JSON file from disk.
pub fn load_columnar_file(path: &Path) -> Result<(Vec<String>, HashMap<String, Vec<Value>>), std::io::Error> {
    let text = fs::read_to_string(path)?;
    let doc: Value = serde_json::from_str(&text).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e)
    })?;
    let keys = parse_columnar_keys(&doc);
    let rows = load_columnar_rows(&doc);
    Ok((keys, rows))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_doc() -> Value {
        serde_json::json!({
            "schema_version": "test-v1",
            "keys": ["slug", "layer", "owner", "tags"],
            "skills": [
                ["alpha", "L2", "owner", ["tag1", "tag2"]],
                ["beta", "L0", "gate", []]
            ]
        })
    }

    #[test]
    fn parse_keys() {
        let keys = parse_columnar_keys(&sample_doc());
        assert_eq!(keys, vec!["slug", "layer", "owner", "tags"]);
    }

    #[test]
    fn extract_slug_set() {
        let slugs = extract_slugs(&sample_doc());
        assert_eq!(slugs, vec!["alpha", "beta"]);
    }

    #[test]
    fn load_rows() {
        let rows = load_columnar_rows(&sample_doc());
        assert_eq!(rows.len(), 2);
        assert!(rows.contains_key("alpha"));
    }

    #[test]
    fn col_accessors() {
        let rows = load_columnar_rows(&sample_doc());
        let keys = parse_columnar_keys(&sample_doc());
        let alpha = rows.get("alpha").unwrap();
        assert_eq!(col_str(alpha, &keys, "layer"), Some("L2"));
        assert_eq!(col_str(alpha, &keys, "owner"), Some("owner"));
        assert_eq!(col_str_vec(alpha, &keys, "tags"), vec!["tag1", "tag2"]);
        assert_eq!(col_str(alpha, &keys, "nonexistent"), None);
    }

    #[test]
    fn load_from_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.json");
        fs::write(&path, serde_json::to_string(&sample_doc()).unwrap()).unwrap();
        let (keys, rows) = load_columnar_file(&path).unwrap();
        assert_eq!(keys.len(), 4);
        assert_eq!(rows.len(), 2);
    }
}
