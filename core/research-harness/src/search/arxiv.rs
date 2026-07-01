// Migrated from tools/autoresearch-rs/src/research.rs

//! arXiv API client.
//!
//! Searches arXiv via the Atom feed API and parses results using quick-xml.
//! Supports field-prefixed queries, date range, category filters, and
//! date-sorted results.

use anyhow::{Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use serde_json::{Value, json};

use crate::search::helpers::*;
use crate::search::options::*;

/// Search arXiv with full SearchOptions support.
///
/// Uses `advanced_query` if set (native arXiv query syntax), otherwise wraps
/// `query` in `all:"keyword"`.
///
/// Supports: year range (submittedDate), sort by date, category filter (cat:),
/// and up to 100 results.
pub fn search(client: &Client, opts: &SearchOptions) -> Result<Vec<Value>> {
    let mut first_non_transient: Option<anyhow::Error> = None;
    for attempt in 0..3 {
        match try_search(client, opts) {
            Ok(results) => return Ok(results),
            Err(e) => {
                let msg = e.to_string();
                let is_transient = msg.contains("503")
                    || msg.contains("502")
                    || msg.contains("429")
                    || msg.contains("timeout")
                    || msg.contains("connection");
                if is_transient && attempt < 2 {
                    std::thread::sleep(std::time::Duration::from_millis(500 * (1 << attempt)));
                    continue;
                }
                if !is_transient && first_non_transient.is_none() {
                    first_non_transient = Some(anyhow::anyhow!("{msg}"));
                }
                if let Some(ctx) = first_non_transient {
                    return Err(ctx);
                }
                return Err(anyhow::anyhow!("arXiv search failed: {msg}"));
            }
        }
    }
    Err(anyhow::anyhow!("arXiv search failed after 3 retries"))
}

fn try_search(client: &Client, opts: &SearchOptions) -> Result<Vec<Value>> {
    crate::util::validate_url_for_fetch(ARXIV_BASE_URL)?;

    // Build search_query: use advanced_query if set, otherwise all:"keyword"
    // or raw query text (fuzzy mode).
    let search_query = if opts.query.trim().is_empty() {
        // Empty query with no advanced_query — use a catch-all to avoid
        // generating invalid arXiv query syntax.
        String::from("all:introduction")
    } else if let Some(ref adv) = opts.advanced_query {
        adv.clone()
    } else if opts.fuzzy_query {
        // Fuzzy mode: pass raw query text without quotes.
        // arXiv's default search does word-level AND across all fields,
        // which naturally acts as fuzzy matching.
        if opts.query.contains(&['(', ')', '"', '\''][..]) {
            // Contains operators — use as-is
            opts.query.clone()
        } else {
            // Simple text: wrap space-separated terms in OR for broader matching
            format!("({})", opts.query.split_whitespace().collect::<Vec<_>>().join(" OR "))
        }
    } else {
        format!("all:\"{}\"", opts.query.replace('"', "\\\""))
    };

    // Append category filter
    let search_query = if let Some(ref cats) = opts.categories {
        let cat_terms: Vec<String> = cats
            .split(',')
            .map(|c| format!("cat:{}", c.trim()))
            .collect();
        if cat_terms.is_empty() {
            search_query
        } else {
            format!("({search_query}) AND ({})", cat_terms.join(" OR "))
        }
    } else {
        search_query
    };

    // Append date range filter
    let search_query = match (opts.year_from, opts.year_to) {
        (Some(from), Some(to)) => {
            format!("{search_query} AND submittedDate:[{from}0101 TO {to}1231]")
        }
        (Some(from), None) => {
            format!("{search_query} AND submittedDate:[{from}0101 TO 99991231]")
        }
        (None, Some(to)) => {
            format!("{search_query} AND submittedDate:[00000101 TO {to}1231]")
        }
        (None, None) => search_query,
    };

    let (sort_by, sort_order) = if opts.sort_by == SortBy::Date {
        ("submittedDate", "descending")
    } else {
        ("relevance", "descending")
    };

    let raw = client
        .get(ARXIV_BASE_URL)
        .header(USER_AGENT, "research-harness/0.1")
        .query(&[
            ("search_query", search_query.as_str()),
            ("start", "0"),
            ("max_results", &normalize_limit(opts.limit).to_string()),
            ("sortBy", sort_by),
            ("sortOrder", sort_order),
        ])
        .send()
        .context("arXiv request failed")?
        .error_for_status()
        .context("arXiv returned an error")?
        .text()
        .context("arXiv returned invalid text")?;

    let mut results = Vec::new();
    let mut reader = Reader::from_str(&raw);
    reader.config_mut().trim_text(true);

    struct EntryData {
        title: String,
        authors: Vec<String>,
        published: String,
        id: String,
        summary: String,
    }

    enum ParseState {
        Outside,
        InEntry,
        InTitle,
        InAuthor,
        InName,
        InPublished,
        InId,
        InSummary,
    }

    let mut state = ParseState::Outside;
    let mut entry: Option<EntryData> = None;
    let mut text_buf = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let tag = e.name().as_ref().to_ascii_lowercase();
                match state {
                    ParseState::Outside if tag == b"entry" => {
                        entry = Some(EntryData {
                            title: String::new(),
                            authors: Vec::new(),
                            published: String::new(),
                            id: String::new(),
                            summary: String::new(),
                        });
                        state = ParseState::InEntry;
                    }
                    ParseState::InEntry if tag == b"title" => {
                        text_buf.clear();
                        state = ParseState::InTitle;
                    }
                    ParseState::InEntry if tag == b"author" => {
                        text_buf.clear();
                        state = ParseState::InAuthor;
                    }
                    ParseState::InAuthor if tag == b"name" => {
                        text_buf.clear();
                        state = ParseState::InName;
                    }
                    ParseState::InEntry if tag == b"published" => {
                        text_buf.clear();
                        state = ParseState::InPublished;
                    }
                    ParseState::InEntry if tag == b"id" => {
                        text_buf.clear();
                        state = ParseState::InId;
                    }
                    ParseState::InEntry if tag == b"summary" => {
                        text_buf.clear();
                        state = ParseState::InSummary;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if let Ok(text) = e.unescape() {
                    match state {
                        ParseState::InTitle
                        | ParseState::InPublished
                        | ParseState::InId
                        | ParseState::InSummary => {
                            text_buf.push_str(text.as_ref());
                        }
                        ParseState::InName => {
                            text_buf.push_str(text.as_ref());
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = e.name().as_ref().to_ascii_lowercase();
                match state {
                    ParseState::InTitle if tag == b"title" => {
                        if let Some(ref mut ent) = entry {
                            ent.title.clone_from(&text_buf);
                        }
                        state = ParseState::InEntry;
                    }
                    ParseState::InName if tag == b"name" => {
                        if !text_buf.trim().is_empty() {
                            if let Some(ref mut ent) = entry {
                                ent.authors.push(text_buf.trim().to_string());
                            }
                        }
                        state = ParseState::InAuthor;
                    }
                    ParseState::InAuthor if tag == b"author" => {
                        state = ParseState::InEntry;
                    }
                    ParseState::InPublished if tag == b"published" => {
                        if let Some(ref mut ent) = entry {
                            ent.published.clone_from(&text_buf);
                        }
                        state = ParseState::InEntry;
                    }
                    ParseState::InId if tag == b"id" => {
                        if let Some(ref mut ent) = entry {
                            ent.id.clone_from(&text_buf);
                        }
                        state = ParseState::InEntry;
                    }
                    ParseState::InSummary if tag == b"summary" => {
                        if let Some(ref mut ent) = entry {
                            ent.summary.clone_from(&text_buf);
                        }
                        state = ParseState::InEntry;
                    }
                    ParseState::InEntry if tag == b"entry" => {
                        if let Some(ent) = entry.take() {
                            let authors_slice = ent
                                .authors
                                .iter()
                                .take(4)
                                .cloned()
                                .collect::<Vec<_>>()
                                .join(", ");
                            let year = if ent.published.len() >= 4 {
                                ent.published[..4].to_string()
                            } else {
                                String::new()
                            };
                            results.push(json!({
                                "source": "arXiv",
                                "title": ent.title,
                                "authors": authors_slice,
                                "year": year,
                                "venue": "arXiv",
                                "url": ent.id,
                                "abstract": ent.summary,
                                "citation_count": Value::Null,
                                "external_ids": Value::Null,
                            }));
                        }
                        state = ParseState::Outside;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                tracing::warn!("[arxiv] XML parse error (skipping remaining entries): {e}");
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(results)
}

// ── Convenience wrapper returning crate::types::Paper ──

/// Search and convert to typed Paper structs.
pub fn search_papers(
    client: &Client,
    opts: &SearchOptions,
) -> Result<Vec<crate::types::Paper>> {
    let raw = search(client, opts)?;
    raw.into_iter().map(json_to_paper).collect()
}

fn json_to_paper(v: Value) -> Result<crate::types::Paper> {
    let title = str_field_default(&v, "title", "_untitled_");
    let url = v.get("url").and_then(Value::as_str).unwrap_or("");
    let id = url
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string();
    let id = if id.is_empty() {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        title.hash(&mut hasher);
        format!("arxiv-hash-{:016x}", hasher.finish())
    } else {
        id
    };
    let authors: Vec<String> = v
        .get("authors")
        .and_then(Value::as_str)
        .unwrap_or("")
        .split(", ")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    let year = v
        .get("year")
        .and_then(Value::as_str)
        .and_then(|y| y.parse::<u32>().ok());
    Ok(crate::types::Paper {
        id,
        title,
        authors,
        abstract_text: v
            .get("abstract")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        year,
        venue: Some("arXiv".to_string()),
        doi: None,
        url: Some(url.to_string()),
        source: crate::types::PaperSource::ArXiv,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;

    #[test]
    fn json_to_paper_normal_url() {
        let v = json!({
            "title": "A Great Paper",
            "url": "http://arxiv.org/abs/2301.07041",
            "authors": "Smith, Jane",
            "year": "2023",
            "abstract": "This is a great paper about AI."
        });
        let paper = json_to_paper(v).unwrap();
        assert_eq!(paper.id, "2301.07041");
        assert_eq!(paper.title, "A Great Paper");
        assert_eq!(paper.authors, vec!["Smith", "Jane"]);
        assert_eq!(paper.year, Some(2023));
        assert_eq!(paper.venue, Some("arXiv".to_string()));
        assert_eq!(paper.doi, None);
    }

    #[test]
    fn json_to_paper_empty_url_fallback_to_hash() {
        let v = json!({
            "title": "Untitled Work",
            "url": "",
            "authors": "",
            "year": "",
            "abstract": ""
        });
        let paper = json_to_paper(v).unwrap();
        assert!(paper.id.starts_with("arxiv-hash-"));
        assert_eq!(paper.authors.len(), 0);
        assert_eq!(paper.year, None);
    }

    #[test]
    fn json_to_paper_url_with_trailing_slash() {
        let v = json!({
            "title": "Test",
            "url": "http://arxiv.org/abs/2301.07042/",
            "authors": "",
            "year": "",
            "abstract": ""
        });
        let paper = json_to_paper(v).unwrap();
        assert_eq!(paper.id, "2301.07042");
    }

    #[test]
    fn normalize_limit_clamps() {
        assert_eq!(normalize_limit(0), 1);
        assert_eq!(normalize_limit(1), 1);
        assert_eq!(normalize_limit(100), 100);
        assert_eq!(normalize_limit(200), 100);
    }
}
