//! 引用审计 — 比对 BibTeX 与 LaTeX 正文的引用完整性。
//!
//! 从 citation_tool_rs 的 BibTeX 解析 + audit 逻辑适配而来。
//! 返回未被正文引用或在 BibTeX 中缺失的条目列表。

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use anyhow::{Context, Result};
use regex::Regex;

static BIB_KEY_RE: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"(?m)^@\w+\s*\{\s*([^,\s]+)").expect("invalid BIB_KEY_RE regex")
});
static LATEX_CITE_RE: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"\\cite[a-zA-Z*]*\s*(?:\[[^\]]*\]\s*){0,2}\{([^}]*)\}")
        .expect("invalid LATEX_CITE_RE regex")
});
static PANDOC_CITE_RE: LazyLock<Regex> = LazyLock::new(|| {
    #[allow(clippy::expect_used)]
    Regex::new(r"(?m)(?:^|[^\w:-])@([A-Za-z0-9_:.+\-/]+)").expect("invalid PANDOC_CITE_RE regex")
});

/// 审计 BibTeX 与 LaTeX 文件的引用交叉比对。
/// 返回未被正文引用或在 BibTeX 中缺失的条目列表。
pub fn audit_bibtex(bib_path: &Path, tex_path: &Path) -> Result<Vec<String>> {
    let bib_text = fs::read_to_string(bib_path)
        .with_context(|| format!("failed to read bib: {}", bib_path.display()))?;
    let tex_text = fs::read_to_string(tex_path)
        .with_context(|| format!("failed to read tex: {}", tex_path.display()))?;

    let bib_keys = extract_bib_keys(&bib_text);
    let cited_keys = extract_cited_keys(&tex_text)?;

    let bib_set: BTreeSet<&str> = bib_keys.iter().map(|s| s.as_str()).collect();
    let cite_set: BTreeSet<&str> = cited_keys.iter().map(|s| s.as_str()).collect();

    let mut issues = Vec::new();

    // Cited but missing from bib
    for key in cite_set.difference(&bib_set) {
        issues.push(format!("cited but missing from bib: {key}"));
    }

    // In bib but uncited
    for key in bib_set.difference(&cite_set) {
        issues.push(format!("in bib but uncited: {key}"));
    }

    Ok(issues)
}

/// Extract BibTeX entry keys from a .bib file.
fn extract_bib_keys(text: &str) -> Vec<String> {
    BIB_KEY_RE
        .captures_iter(text)
        .map(|cap| cap[1].trim().to_string())
        .collect()
}

/// Extract cited keys from LaTeX text (\cite{key1,key2} and @pandoc citations).
fn extract_cited_keys(text: &str) -> Result<Vec<String>> {
    let mut keys = Vec::new();
    for cap in LATEX_CITE_RE.captures_iter(text) {
        keys.extend(
            cap[1]
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        );
    }
    for cap in PANDOC_CITE_RE.captures_iter(text) {
        keys.push(cap[1].trim().to_string());
    }
    Ok(keys)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn audit_finds_missing_and_uncited() {
        let dir = tempfile::tempdir().unwrap();
        let bib = dir.path().join("refs.bib");
        let tex = dir.path().join("main.tex");

        fs::write(
            &bib,
            r#"@article{key1, author={A}, title={T}, year={2024}}
@article{key2, author={B}, title={U}, year={2023}}
"#,
        )
        .unwrap();
        fs::write(&tex, r"This is a claim \cite{key1,key3}.").unwrap();

        let issues = audit_bibtex(&bib, &tex).unwrap();
        let issue_text = issues.join("\n");
        assert!(issue_text.contains("missing from bib: key3"));
        assert!(issue_text.contains("uncited: key2"));
    }

    #[test]
    fn audit_clean_when_all_match() {
        let dir = tempfile::tempdir().unwrap();
        let bib = dir.path().join("refs.bib");
        let tex = dir.path().join("main.tex");

        fs::write(
            &bib,
            r#"@article{smith2024, author={Smith}, title={T}, year={2024}}"#,
        )
        .unwrap();
        fs::write(&tex, r"Citation here \cite{smith2024}.").unwrap();

        let issues = audit_bibtex(&bib, &tex).unwrap();
        assert!(issues.is_empty());
    }
}
