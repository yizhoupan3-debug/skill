use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Instant;
use super::search_engines::Topic;
static CACHE: OnceLock<std::sync::Mutex<HashMap<String, (String, Instant, Topic)>>> = OnceLock::new();
const MAX: usize = 500;
fn key(q: &str, t: Topic) -> String { format!("{}\0{}", t.as_str(), q) }
pub fn check_search_cache(q: &str, t: Topic) -> Option<String> {
    let c = CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let g = c.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((body, ts, topic)) = g.get(&key(q, t)) { if ts.elapsed() < topic.cache_ttl() { return Some(body.clone()); } }
    None
}
pub fn store_search_cache(q: &str, t: Topic, val: &str) {
    let c = CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let mut g = c.lock().unwrap_or_else(|e| e.into_inner());
    if g.len() >= MAX { if let Some(k) = g.iter().min_by_key(|(_,(_,ts,_))| ts).map(|(k,_)| k.clone()) { g.remove(&k); } }
    g.insert(key(q, t), (val.to_string(), Instant::now(), t));
}
