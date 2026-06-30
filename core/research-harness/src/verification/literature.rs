//! 文献验证 — DOI 可达性检查与声明覆盖率计算。

use anyhow::Result;
use std::collections::HashSet;

/// 验证 DOI 是否可解析（网络可达）。
///
/// 通过向 https://doi.org/<doi> 发送 HEAD 请求，检查 3xx/2xx 响应。
/// 每请求构建一次 DNS-pinned client 以防止 DNS rebinding TOCTOU。
pub async fn verify_doi_reachable(doi: &str) -> Result<bool> {
    let doi = doi.trim();
    if doi.is_empty() {
        anyhow::bail!("empty DOI");
    }
    let url = if doi.starts_with("http") {
        doi.to_string()
    } else {
        format!("https://doi.org/{doi}")
    };
    // SSRF validation + DNS resolution in one pass, returns pinned addresses.
    let (host, addrs) = crate::util::validate_and_resolve_for_fetch(&url)?;

    // Build a per-request client with DNS pinning to prevent DNS rebinding.
    let client = reqwest::Client::builder()
        .user_agent("research-harness/0.1")
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(5));
    let client = addrs
        .iter()
        .fold(client, |b, addr| b.resolve(&host, *addr))
        .build()?;
    let resp = client.head(&url).send().await?;
    Ok(resp.status().is_success() || resp.status().is_redirection())
}

/// 计算 claims 被 references 覆盖的比率（0.0-1.0）。
///
/// 基于关键词重叠：对每个 claim 提取内容词，检查是否有至少 2 个内容词
/// 超过 threshold 30% 出现在 reference 中。
pub fn verify_claim_coverage(claims: &[String], references: &[String]) -> Result<f64> {
    if claims.is_empty() {
        return Ok(1.0); // 没有 claim → 100% 覆盖（空集）
    }

    // 提取所有 reference 的内容词集合
    let ref_words: HashSet<String> = references
        .iter()
        .flat_map(|r| extract_content_words(r))
        .collect();

    if ref_words.is_empty() && !references.is_empty() {
        return Ok(0.0);
    }

    let mut covered = 0;
    for claim in claims {
        let claim_words = extract_content_words(claim);
        if claim_words.is_empty() {
            covered += 1; // 空 claim 自动覆盖
            continue;
        }
        // 要求至少 2 个词重叠或 30% 以上重叠，避免单高频词假阳性
        let overlap_count = claim_words
            .iter()
            .filter(|w| ref_words.contains(*w as &str))
            .count();
        let has_overlap =
            overlap_count >= 2 || (overlap_count as f64 / claim_words.len() as f64) >= 0.3;
        if has_overlap {
            covered += 1;
        }
    }

    Ok(covered as f64 / claims.len() as f64)
}

/// 提取文本中的内容词（≥3 字符，小写化，去停用词）。
fn extract_content_words(text: &str) -> HashSet<String> {
    crate::text::extract_content_words(text)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn full_coverage_when_all_match() {
        let claims = vec!["transformer improves accuracy".into()];
        let refs = vec!["transformer model achieves state-of-the-art accuracy".into()];
        let coverage = verify_claim_coverage(&claims, &refs).unwrap();
        assert!(coverage >= 0.9);
    }

    #[test]
    fn no_coverage_when_no_overlap() {
        let claims = vec!["quantum computing breakthrough".into()];
        let refs = vec!["cooking recipe for pasta".into()];
        let coverage = verify_claim_coverage(&claims, &refs).unwrap();
        assert!(coverage < 0.5);
    }

    #[test]
    fn empty_claims_full_coverage() {
        let coverage = verify_claim_coverage(&[], &["anything".into()]).unwrap();
        assert!((coverage - 1.0).abs() < 0.01);
    }

    #[test]
    fn verify_doi_reachable_rejects_invalid_doi() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        // A clearly invalid DOI should fail URL validation before making a request.
        let result = rt.block_on(verify_doi_reachable(""));
        assert!(result.is_err(), "empty DOI should be rejected: {:?}", result);
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("empty") || err.contains("invalid") || err.contains("host") || err.contains("URL"),
            "error should mention reason (empty/invalid/host/URL): {err}"
        );
    }
}
