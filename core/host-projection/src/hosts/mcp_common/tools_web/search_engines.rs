use serde_json::{json, Value};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Topic { General, Tech, News, Knowledge, Academic }

impl Topic {
    pub fn as_str(&self) -> &'static str {
        match self { Self::General=>"general",Self::Tech=>"tech",Self::News=>"news",Self::Knowledge=>"knowledge",Self::Academic=>"academic" }
    }
    pub fn from_str(s: &str) -> Self {
        match s { "tech"=>Self::Tech,"news"=>Self::News,"knowledge"=>Self::Knowledge,"academic"=>Self::Academic,_=>Self::General }
    }
    pub fn cache_ttl(&self) -> Duration {
        match self { Self::News=>Duration::from_secs(60),Self::Academic=>Duration::from_secs(600),Self::Knowledge=>Duration::from_secs(1800),_=>Duration::from_secs(300) }
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult { pub title: String, pub url: String, pub snippet: String, pub source: &'static str, pub score: f64 }

pub trait SearchSource: Send + Sync { fn name(&self) -> &'static str; fn search(&self, query: &str) -> Result<Vec<SearchResult>, String>; }

pub fn auto_detect_topic(q: &str) -> Topic {
    let l = q.to_lowercase();
    if ["paper","research","study","arxiv","methodology","survey","thesis","doi"].iter().any(|k| l.contains(k)) { return Topic::Academic; }
    if ["latest","today","news","announced","released","breaking","2025","2026"].iter().any(|k| l.contains(k)) { return Topic::News; }
    if ["error","bug","function","api","library","code","rust","python","javascript","typescript","docker","git","benchmark","compile","crate","trait","async"].iter().any(|k| l.contains(k)) { return Topic::Tech; }
    if ["what is","who is","history of","definition","explain","meaning"].iter().any(|k| l.contains(k)) { return Topic::Knowledge; }
    Topic::General
}

pub fn search_concurrent(query: &str, topic: Topic) -> (Vec<SearchResult>, Vec<&'static str>, Vec<&'static str>) {
    use super::{searxng::SearxngSource, vertical_apis, brave_api::BraveSearchSource};
    let mut sources: Vec<Box<dyn SearchSource>> = Vec::new();
    if let Ok(u) = std::env::var("SEARXNG_URL") { if !u.trim().is_empty() { sources.push(Box::new(SearxngSource::new(u))); } }
    match topic {
        Topic::Tech => { sources.push(Box::new(vertical_apis::StackOverflowSource)); sources.push(Box::new(vertical_apis::GithubSource)); }
        Topic::News => { sources.push(Box::new(vertical_apis::HnAlgoliaSource)); }
        Topic::Knowledge => { sources.push(Box::new(vertical_apis::WikipediaSource)); }
        Topic::Academic => { sources.push(Box::new(vertical_apis::ArxivSource)); }
        Topic::General => { sources.push(Box::new(vertical_apis::StackOverflowSource)); sources.push(Box::new(vertical_apis::WikipediaSource)); sources.push(Box::new(vertical_apis::HnAlgoliaSource)); }
    }
    if std::env::var("BRAVE_API_KEY").is_ok() { sources.push(Box::new(BraveSearchSource)); }
    if sources.is_empty() { return (vec![], vec![], vec!["no_sources"]); }
    let (tx, rx) = std::sync::mpsc::channel();
    let mut handles = Vec::new();
    for source in sources { let tx = tx.clone(); let q = query.to_string(); handles.push(std::thread::spawn(move || { let n = source.name(); let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| source.search(&q))); let _ = tx.send((n, r)); })); }
    drop(tx);
    let deadline = Instant::now() + Duration::from_secs(6);
    let mut all = Vec::new(); let mut ok = Vec::new(); let mut fail: Vec<&'static str> = Vec::new();
    loop { let rem = deadline.saturating_duration_since(Instant::now()); if rem.is_zero() { break; } match rx.recv_timeout(rem) { Ok((n, Ok(Ok(r)))) => { ok.push(n); all.extend(r); } Ok((n, _)) => { fail.push(n); } Err(_) => break, } }
    for h in handles { let _ = h.join(); }
    (all, ok, fail)
}

pub fn tool_web_search(args: &Value) -> router_rs::framework_error::FrameworkResult<String> {
    use router_rs::framework_error::FrameworkError;
    let query = args.get("query").and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()).ok_or_else(|| FrameworkError::validation("Missing required argument: query"))?;
    let max = args.get("max_results").and_then(Value::as_u64).map(|v| v as usize).unwrap_or(8).clamp(1, 15);
    let no_cache = args.get("no_cache").and_then(Value::as_bool).unwrap_or(false);
    let topic = if let Some(t) = args.get("topic").and_then(Value::as_str) { Topic::from_str(t) } else { auto_detect_topic(query) };
    if !no_cache { if let Some(c) = super::search_cache::check_search_cache(query, topic) { return Ok(c); } }
    let start = Instant::now();
    let (mut results, succeeded, failed) = search_concurrent(query, topic);
    super::result_merge::dedupe_and_sort(&mut results);
    let total = results.len(); results.truncate(max);
    let vals: Vec<Value> = results.iter().map(|r| json!({"title":r.title,"url":r.url,"snippet":r.snippet.chars().take(300).collect::<String>(),"source":r.source,"score":r.score})).collect();
    let payload = json!({"results":vals,"total":total,"detected_topic":topic.as_str(),"sources_queried":succeeded,"sources_failed":failed,"latency_ms":start.elapsed().as_millis() as u64,"cached":false});
    let s = serde_json::to_string_pretty(&payload)?;
    if !no_cache { super::search_cache::store_search_cache(query, topic, &s); }
    Ok(s)
}
