//! DOI 验证与解析。
//!
//! DOI 格式验证和通过 doi.org API 解析论文元数据。

use std::sync::LazyLock;

use anyhow::Result;
use regex::Regex;

use crate::types::{Paper, PaperSource};

static DOI_PATTERN_RE: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"^10\.\d{4,}/.+").expect("invalid DOI_PATTERN_RE regex")
});

/// 验证 DOI 格式是否合法。
///
/// DOI 格式：10.NNNN/... (数字前缀/后缀)
/// 参考：https://www.doi.org/doi_handbook/2_Numbering.html
pub fn validate_doi(doi: &str) -> Result<bool> {
    let doi = doi.trim();

    // 去掉常见前缀
    let stripped = doi
        .strip_prefix("https://doi.org/")
        .or_else(|| doi.strip_prefix("http://doi.org/"))
        .or_else(|| doi.strip_prefix("https://dx.doi.org/"))
        .or_else(|| doi.strip_prefix("http://dx.doi.org/"))
        .or_else(|| doi.strip_prefix("doi:"))
        .unwrap_or(doi)
        .trim();

    // 基本格式检查：10.NNNN/...
    Ok(DOI_PATTERN_RE.is_match(stripped))
}

/// 通过 DOI 解析论文元数据。
///
/// 调用 https://doi.org/<doi> 的 Content Negotiation API 获取 JSON-LD 元数据。
/// 若网络不可用，返回仅有 DOI 信息的最小 Paper。
pub async fn resolve_doi(doi: &str) -> Result<Paper> {
    let clean_doi = doi
        .strip_prefix("https://doi.org/")
        .or_else(|| doi.strip_prefix("http://doi.org/"))
        .or_else(|| doi.strip_prefix("doi:"))
        .unwrap_or(doi)
        .trim();

    // 尝试通过 CrossRef API 获取元数据
    let url = format!("https://api.crossref.org/works/{}", clean_doi);
    let client = reqwest::Client::builder()
        .user_agent("research-harness/0.1 (CrossRef API; research assistant)")
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let json: serde_json::Value = resp.json().await?;
            let msg = json.get("message").unwrap_or(&json);

            let title = msg
                .get("title")
                .and_then(|t| t.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled")
                .to_string();

            let authors: Vec<String> = msg
                .get("author")
                .and_then(|a| a.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|a| {
                            let given = a.get("given").and_then(|v| v.as_str()).unwrap_or("");
                            let family = a.get("family").and_then(|v| v.as_str()).unwrap_or("");
                            format!("{given} {family}").trim().to_string()
                        })
                        .collect()
                })
                .unwrap_or_default();

            let year = msg
                .get("published-print")
                .or_else(|| msg.get("published-online"))
                .and_then(|p| p.get("date-parts"))
                .and_then(|d| d.as_array())
                .and_then(|d| d.first())
                .and_then(|d| d.as_array())
                .and_then(|d| d.first())
                .and_then(|v| v.as_u64())
                .map(|y| y as u32);

            let url_str = msg.get("URL").and_then(|v| v.as_str()).map(String::from);

            Ok(Paper {
                id: clean_doi.to_string(),
                title,
                authors,
                abstract_text: msg
                    .get("abstract")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                year,
                venue: msg
                    .get("container-title")
                    .and_then(|t| t.as_array())
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
                    .map(String::from),
                doi: Some(clean_doi.to_string()),
                url: url_str,
                source: PaperSource::Manual,
            })
        }
        _ => {
            // Fallback: minimal paper with DOI info
            Ok(Paper {
                id: clean_doi.to_string(),
                title: format!("(unresolved) {clean_doi}"),
                authors: vec![],
                abstract_text: String::new(),
                year: None,
                venue: None,
                doi: Some(clean_doi.to_string()),
                url: Some(format!("https://doi.org/{clean_doi}")),
                source: PaperSource::Manual,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn validate_standard_doi() {
        assert!(validate_doi("10.1000/tools").unwrap());
        assert!(validate_doi("10.1145/3290605.3300857").unwrap());
    }

    #[test]
    fn validate_doi_with_prefix() {
        assert!(validate_doi("https://doi.org/10.1000/tools").unwrap());
        assert!(validate_doi("doi:10.1000/tools").unwrap());
    }

    #[test]
    fn reject_invalid_doi() {
        assert!(!validate_doi("not-a-doi").unwrap());
        assert!(!validate_doi("10.12/short").unwrap()); // prefix too short
        assert!(!validate_doi("").unwrap());
    }
}
