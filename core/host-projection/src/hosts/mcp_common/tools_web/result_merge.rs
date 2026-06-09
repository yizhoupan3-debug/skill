use super::search_engines::SearchResult;
use std::collections::HashSet;
pub fn dedupe_and_sort(results: &mut Vec<SearchResult>) {
    let mut seen = HashSet::new();
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    let unique: Vec<_> = results.drain(..).filter(|r| {
        let k = r.url.trim_end_matches('/').to_lowercase();
        let k = k.strip_prefix("http://").or(k.strip_prefix("https://")).unwrap_or(&k);
        seen.insert(k.to_string())
    }).collect();
    *results = unique;
}
