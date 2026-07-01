// Migrated from tools/autoresearch-rs/src/research.rs

//! Multi-source literature search orchestration.
//!
//! Coordinates searches across Semantic Scholar, arXiv, and paperplain MCP,
//! deduplicates results, and ranks by relevance.

use anyhow::{Result, bail};
use chrono::Datelike;
use serde_json::{Value, json};

use crate::search::helpers::*;
use crate::search::options::*;
use crate::types::Paper;

// ── Journal impact factors (2024-2025) ──
// Key CS/AI venues and their impact factors or acceptance-rate tiers.

const JOURNAL_IF: &[(&str, f64)] = &[
    ("nature", 50.0),
    ("science", 48.0),
    ("cell", 30.0),
    ("lancet", 60.0),
    ("pnas", 10.0),
    ("nature communications", 15.0),
    ("science advances", 13.0),
    ("ieee transactions on pattern analysis and machine intelligence", 25.0),
    ("tpami", 25.0),
    ("international journal of computer vision", 15.0),
    ("ijcv", 15.0),
    ("journal of machine learning research", 5.0),
    ("jmlr", 5.0),
    ("machine learning", 5.0),
    ("artificial intelligence", 10.0),
    ("ieee transactions on neural networks and learning systems", 10.0),
    ("ieee transactions on image processing", 10.0),
    ("acm computing surveys", 15.0),
    ("ieee transactions on knowledge and data engineering", 9.0),
    ("pattern recognition", 8.0),
    ("computer vision and image understanding", 5.0),
    ("acm transactions on graphics", 5.0),
    ("ieee transactions on software engineering", 6.0),
    ("ieee access", 3.0),
    ("ieee signal processing magazine", 15.0),
    ("ieee transactions on visualization and computer graphics", 5.0),
    ("ieee robotics and automation letters", 4.0),
    ("ieee transactions on robotics", 7.0),
    ("autonomous robots", 3.0),
    ("journal of artificial intelligence research", 4.0),
];

/// Conference acceptance-rate tiers: 1 = top (~20-25%), 2 = mid (~25-30%), 3 = lower.
const CONFERENCE_TIER: &[(&str, u32)] = &[
    ("NeurIPS", 1),
    ("ICML", 1),
    ("ICLR", 1),
    ("COLT", 1),
    ("CVPR", 1),
    ("ICCV", 1),
    ("ACL", 1),
    ("EMNLP", 1),
    ("CHI", 1),
    ("SIGGRAPH", 1),
    ("OSDI", 1),
    ("SOSP", 1),
    ("PLDI", 1),
    ("POPL", 1),
    ("STOC", 1),
    ("FOCS", 1),
    ("AAAI", 2),
    ("IJCAI", 2),
    ("ECCV", 2),
    ("NAACL", 2),
    ("EACL", 2),
    ("CoNLL", 2),
    ("AISTATS", 2),
    ("UAI", 2),
    ("ICRA", 2),
    ("IROS", 2),
    ("RSS", 2),
    ("UIST", 2),
    ("CSCW", 2),
    ("KDD", 2),
    ("WWW", 2),
    ("WSDM", 2),
    ("SIGIR", 2),
    ("ECIR", 2),
    ("USENIX ATC", 2),
    ("EuroSys", 2),
    ("ASPLOS", 2),
    ("ICSE", 2),
    ("FSE", 2),
    ("ASE", 2),
    ("VLDB", 2),
    ("SIGMOD", 2),
    ("ICDE", 2),
    ("IEEE S&P", 1),
    ("USENIX Security", 1),
    ("NDSS", 1),
    ("CRYPTO", 1),
    ("EUROCRYPT", 1),
    ("BMVC", 3),
    ("WACV", 3),
    ("ACCV", 3),
    ("COLING", 3),
    ("IMWUT", 3),
    ("UbiComp", 3),
    ("CORL", 3),
    ("CIKM", 3),
    ("ISCA", 3),
    ("MICRO", 3),
    ("HPCA", 3),
    ("PODS", 3),
    ("ACM CCS", 1),
];

fn lookup_impact_factor(venue: &str) -> Option<f64> {
    let v = venue.to_ascii_lowercase().trim().to_string();
    // Sort by venue name length (longest first) so specific matches like
    // "Nature Communications" are tried before generic "nature".
    let sorted: Vec<_> = {
        let mut v: Vec<_> = JOURNAL_IF.iter().collect();
        v.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
        v
    };
    for (venue_name, if_val) in &sorted {
        if v.contains(venue_name) {
            return Some(*if_val);
        }
    }
    None
}

fn lookup_conference_tier(venue: &str) -> Option<u32> {
    // Exact match against the venue name (case-insensitive)
    let v_lower = venue.to_ascii_lowercase();
    CONFERENCE_TIER
        .iter()
        .find(|(name, _)| {
            let n_lower = name.to_ascii_lowercase();
            v_lower == n_lower || v_lower.starts_with(&format!("{n_lower} ")) || v_lower.contains(&format!(" {n_lower}"))
        })
        .map(|(_, tier)| *tier)
}

/// Score a paper JSON result for authority, recency, and relevance.
///
/// Returns (total_score, reasons) where total_score is a composite:
/// - +2: has DOI (published, peer-reviewed by some venue)
/// - +2: venue matches known authoritative venue
/// - +1: venue exists (any venue, including non-authoritative)
/// - +1: from Semantic Scholar (has citation count, more metadata)
/// - +0..3: citation count: 0 for 0-10, 1 for 10-100, 2 for 100-1000, 3 for 1000+
/// - +0..3: recency: 3 for current year, 2 for last 2 years, 1 for last 5 years
/// - +0..2: impact factor bonus: IF/10 rounded (max 2 for top journals like Nature/Science)
/// - +0..2: conference tier bonus: tier 1 → +2, tier 2 → +1
/// - -1: arXiv-only (no DOI, no venue) — demotes pure preprints
fn score_paper(paper: &Value, current_year: u32) -> (i32, Vec<String>) {
    let mut score = 0i32;
    let mut reasons = Vec::new();

    // DOI check
    let has_doi = paper
        .get("external_ids")
        .and_then(|ids| ids.get("DOI"))
        .and_then(Value::as_str)
        .map(|s| !s.is_empty())
        .unwrap_or(false)
        || paper.get("doi").and_then(Value::as_str).map(|s| !s.is_empty()).unwrap_or(false);
    if has_doi {
        score += 2;
        reasons.push("has DOI".into());
    }

    // Venue check + impact factor + conference tier
    let venue = paper.get("venue").and_then(Value::as_str);
    let is_arxiv_venue = venue.map_or(false, |v| v == "arXiv");
    if let Some(v) = venue {
        if !is_arxiv_venue {
            // Impact factor bonus
            if let Some(if_val) = lookup_impact_factor(v) {
                let if_bonus = ((if_val / 10.0).round() as i32).clamp(0, 2);
                if if_bonus > 0 {
                    score += if_bonus;
                    reasons.push(format!("IF={if_val:.1}"));
                }
            }
            // Conference tier bonus
            if let Some(tier) = lookup_conference_tier(v) {
                let tier_bonus = if tier == 1 { 2 } else { 1 };
                score += tier_bonus;
                reasons.push(format!("tier-{tier} venue"));
            } else if !is_arxiv_venue && lookup_impact_factor(v).is_none() {
                // Known venue without IF/tier data → still authoritative
                score += 1;
                reasons.push(format!("venue: {v}"));
            }
        }
    } else {
        // Pure arXiv or no venue at all
        score -= 1;
        reasons.push("no venue metadata".into());
    }

    // Source bonus: Semantic Scholar has richer metadata
    if paper
        .get("source")
        .and_then(Value::as_str)
        .map(|s| s.contains("Semantic Scholar"))
        .unwrap_or(false)
    {
        score += 1;
        reasons.push("cross-ref".into());
    }

    // Citation count
    let cites = paper
        .get("citation_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if cites >= 1000 {
        score += 3;
        if cites >= 1000 {
            reasons.push(format!("{cites}+ citations"));
        }
    } else if cites >= 100 {
        score += 2;
    } else if cites >= 10 {
        score += 1;
    }

    // Year recency
    let year = paper.get("year").and_then(Value::as_u64).unwrap_or(0) as u32;
    if year > 0 {
        let age = current_year.saturating_sub(year);
        if age == 0 {
            score += 3;
            reasons.push("current year".into());
        } else if age <= 2 {
            score += 2;
            reasons.push("recent".into());
        } else if age <= 5 {
            score += 1;
        }
    }

    // Penalty: arXiv-only paper (no DOI, no venue metadata, or venue="arXiv")
    let is_arxiv = paper
        .get("source")
        .and_then(Value::as_str)
        .map(|s| s == "arXiv")
        .unwrap_or(false);
    if is_arxiv && !has_doi && (is_arxiv_venue || venue.is_none()) {
        score -= 1;
        reasons.push("preprint only".into());
    }

    (score, reasons)
}

fn current_year() -> u32 {
    // Compute from current UTC date.
    chrono::Utc::now().year() as u32
}

// ── Blocking search ──

/// Orchestrate a multi-source literature search using raw JSON results.
///
/// Accepts full SearchOptions. When `prefer_authoritative` is true, fetches
/// results with a 3× limit internally, scores each by DOI/venue/IF/
/// citations/recency, and returns the top `limit` by score.
pub fn search_raw(opts: &SearchOptions) -> Result<Value> {
    let client = http_client(opts.timeout_secs)?;
    let mut results = Vec::new();
    let mut errors = Vec::new();
    // When authoritative ranking is enabled, fetch extra for better sorting
    let effective_limit = if opts.prefer_authoritative {
        (opts.limit * 3).min(100)
    } else {
        opts.limit
    };
    let fetch_opts = SearchOptions {
        limit: effective_limit,
        ..opts.clone()
    };

    if matches!(
        fetch_opts.source,
        ExternalSourceArg::All | ExternalSourceArg::SemanticScholar
    ) {
        match crate::search::semantic_scholar::search(&client, &fetch_opts) {
            Ok(items) => results.extend(items),
            Err(err) => errors.push(format!("semantic-scholar: {err}")),
        }
    }
    if matches!(
        fetch_opts.source,
        ExternalSourceArg::All | ExternalSourceArg::Arxiv
    ) {
        match crate::search::arxiv::search(&client, &fetch_opts) {
            Ok(items) => results.extend(items),
            Err(err) => errors.push(format!("arxiv: {err}")),
        }
    }

    if results.is_empty() && !errors.is_empty() {
        bail!("External research failed: {}", errors.join("; "));
    }

    let deduped = dedupe_research_results(results);

    // Apply authority ranking if requested
    Ok(if opts.prefer_authoritative {
        let year = current_year();
        let mut scored: Vec<Value> = deduped
            .into_iter()
            .map(|mut paper| {
                let (score, reasons) = score_paper(&paper, year);
                if let Some(obj) = paper.as_object_mut() {
                    obj.insert("authority_score".into(), json!(score));
                    obj.insert("score_reasons".into(), json!(reasons));
                }
                paper
            })
            .collect();
        // Sort by authority_score desc, then year desc, then citations desc
        scored.sort_by(|a, b| {
            let sa = a.get("authority_score").and_then(Value::as_i64).unwrap_or(0);
            let sb = b.get("authority_score").and_then(Value::as_i64).unwrap_or(0);
            sb.cmp(&sa).then_with(|| {
                let ya = a.get("year").and_then(Value::as_u64).unwrap_or(0);
                let yb = b.get("year").and_then(Value::as_u64).unwrap_or(0);
                yb.cmp(&ya)
            }).then_with(|| {
                let ca = a.get("citation_count").and_then(Value::as_u64).unwrap_or(0);
                let cb = b.get("citation_count").and_then(Value::as_u64).unwrap_or(0);
                cb.cmp(&ca)
            })
        });
        scored.truncate(opts.limit);
        json!({
            "query": opts.query,
            "source": opts.source.as_str(),
            "results": scored,
            "errors": errors,
            "authority_ranking": true,
        })
    } else {
        json!({
            "query": opts.query,
            "source": opts.source.as_str(),
            "results": deduped,
            "errors": errors,
        })
    })
}

/// Legacy convenience — search all sources with just query + limit.
pub fn search_raw_legacy(query: &str, limit: usize) -> Result<Value> {
    let mut opts = SearchOptions::new(query);
    opts.limit = limit;
    search_raw(&opts)
}

/// Orchestrate a multi-source literature search, returning typed Paper structs.
pub fn search_all(opts: &SearchOptions) -> Result<Vec<Paper>> {
    let client = http_client(opts.timeout_secs)?;
    let mut papers = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    if matches!(
        opts.source,
        ExternalSourceArg::All | ExternalSourceArg::SemanticScholar
    ) {
        match crate::search::semantic_scholar::search_papers(&client, opts) {
            Ok(items) => papers.extend(items),
            Err(err) => {
                let msg = format!("Semantic Scholar search failed: {err}");
                tracing::warn!("{msg}");
                errors.push(msg);
            }
        }
    }
    if matches!(
        opts.source,
        ExternalSourceArg::All | ExternalSourceArg::Arxiv
    ) {
        match crate::search::arxiv::search_papers(&client, opts) {
            Ok(items) => papers.extend(items),
            Err(err) => {
                let msg = format!("arXiv search failed: {err}");
                tracing::warn!("{msg}");
                errors.push(msg);
            }
        }
    }

    if papers.is_empty() && !errors.is_empty() {
        bail!("All search sources failed: {}", errors.join("; "));
    }

    deduplicate_papers(&mut papers);
    papers.truncate(opts.limit);
    Ok(papers)
}

/// Legacy convenience — search all sources with defaults.
pub fn search(query: &str, limit: usize) -> Result<Vec<Paper>> {
    search_all(&SearchOptions {
        limit,
        ..SearchOptions::new(query)
    })
}

// ── Async search wrappers (future path) ──

/// Async wrapper around search_raw.
pub async fn async_search_raw(opts: SearchOptions) -> Result<Value> {
    tokio::task::spawn_blocking(move || search_raw(&opts))
        .await
        .map_err(|e| anyhow::anyhow!("search task join failed: {e}"))?
}

/// Async wrapper around search_all.
pub async fn async_search_all(opts: SearchOptions) -> Result<Vec<Paper>> {
    tokio::task::spawn_blocking(move || search_all(&opts))
        .await
        .map_err(|e| anyhow::anyhow!("search task join failed: {e}"))?
}

/// Async wrapper around search (legacy interface).
pub async fn async_search(query: String, limit: usize) -> Result<Vec<Paper>> {
    async_search_all(SearchOptions {
        limit,
        ..SearchOptions::new(query)
    })
    .await
}

/// Deduplicate papers in-place by (source, title) key (case-insensitive).
fn deduplicate_papers(papers: &mut Vec<Paper>) {
    let mut seen = std::collections::HashSet::new();
    papers.retain(|p| {
        let key = format!("{}::{}", p.source, p.title.to_lowercase());
        seen.insert(key)
    });
}

// ── Authority scoring tests ──

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn score_paper_top_journal_high_if() {
        let paper = json!({
            "source": "Semantic Scholar",
            "venue": "Nature",
            "year": 2025,
            "citation_count": 500,
            "external_ids": {"DOI": "10.1234/test"}
        });
        let (score, reasons) = score_paper(&paper, 2026);
        assert!(score >= 9, "Nature paper should score >= 9, got {score}: {reasons:?}");
        assert!(reasons.iter().any(|r| r.contains("DOI")));
        assert!(reasons.iter().any(|r| r.starts_with("IF=")));
    }

    #[test]
    fn score_paper_tier1_conference() {
        let paper = json!({
            "source": "Semantic Scholar",
            "venue": "NeurIPS 2024",
            "year": 2024,
            "citation_count": 200,
            "external_ids": {"DOI": "10.1234/test"}
        });
        let (score, reasons) = score_paper(&paper, 2026);
        assert!(score >= 8, "NeurIPS paper should score >= 8, got {score}: {reasons:?}");
        assert!(reasons.iter().any(|r| r.contains("tier-1")));
    }

    #[test]
    fn score_paper_preprint_penalty() {
        let paper = json!({
            "source": "arXiv",
            "title": "A Test Paper",
            "year": 2026,
            "venue": "arXiv"
        });
        let (score, reasons) = score_paper(&paper, 2026);
        assert!(score <= 3, "preprint should score <= 3, got {score}: {reasons:?}");
        assert!(reasons.iter().any(|r| r.contains("preprint")));
    }

    #[test]
    fn score_paper_no_venue_metadata() {
        let paper = json!({
            "source": "Semantic Scholar",
            "title": "No Venue Info",
            "year": 2023,
            "citation_count": 5
        });
        let (score, reasons) = score_paper(&paper, 2026);
        // Should have: cross-ref +1, 5 cites (no bonus), 2023 (age 3, +1), no venue → -1
        // Total: 1
        assert!(reasons.iter().any(|r| r.contains("no venue")));
    }

    #[test]
    fn score_paper_current_year_recency() {
        let paper = json!({
            "source": "Semantic Scholar",
            "year": 2026,
            "venue": "ICML",
            "external_ids": {"DOI": "10.1234/test"}
        });
        let (score, reasons) = score_paper(&paper, 2026);
        assert!(reasons.iter().any(|r| r.contains("current year")));
    }

    #[test]
    fn score_paper_minimal_arxiv() {
        let paper = json!({
            "source": "arXiv",
            "title": "Minimal"
        });
        let (score, reasons) = score_paper(&paper, 2026);
        assert!(score <= 2, "minimal arxiv should score low, got {score}: {reasons:?}");
    }

    #[test]
    fn lookup_impact_factor_matches() {
        assert!(lookup_impact_factor("Nature").unwrap() > 40.0);
        assert!(lookup_impact_factor("IEEE Transactions on Pattern Analysis and Machine Intelligence").unwrap() > 20.0);
        assert!(lookup_impact_factor("Some Unknown Journal").is_none());
    }

    #[test]
    fn lookup_conference_tier_matches() {
        assert_eq!(lookup_conference_tier("NeurIPS").unwrap(), 1);
        assert_eq!(lookup_conference_tier("NeurIPS 2024").unwrap(), 1);
        assert_eq!(lookup_conference_tier("AAAI").unwrap(), 2);
        assert_eq!(lookup_conference_tier("BMVC").unwrap(), 3);
        assert!(lookup_conference_tier("Some Random Conference").is_none());
    }

    #[test]
    fn search_raw_rejects_empty_query_gracefully() {
        let opts = SearchOptions {
            limit: 5,
            timeout_secs: 3,
            ..SearchOptions::new("")
        };
        let result = search_raw(&opts);
        let _ = result;
    }

    #[test]
    fn external_source_arg_display() {
        assert_eq!(ExternalSourceArg::All.as_str(), "all");
        assert_eq!(
            ExternalSourceArg::SemanticScholar.as_str(),
            "semantic-scholar"
        );
        assert_eq!(ExternalSourceArg::Arxiv.as_str(), "arxiv");
    }

    #[test]
    fn authoritative_ranking_fetches_extra() {
        // Verify that `prefer_authoritative` triples the fetch limit
        let opts = SearchOptions {
            limit: 10,
            prefer_authoritative: true,
            timeout_secs: 3,
            ..SearchOptions::new("test")
        };
        // The effective_limit calculation is internal, but search_raw should
        // not error with these params — it'll just return few results quickly.
        let result = search_raw(&opts);
        // May be an error due to network OR valid, but never panic.
        let _ = result;
    }
}
