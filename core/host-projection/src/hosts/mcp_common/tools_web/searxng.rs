use super::search_engines::{SearchResult, SearchSource};
pub struct SearxngSource { base_url: String }
impl SearxngSource { pub fn new(u: String) -> Self { Self { base_url: u } } }
impl SearchSource for SearxngSource {
    fn name(&self) -> &'static str { "searxng" }
    fn search(&self, q: &str) -> Result<Vec<SearchResult>, String> {
        let url = format!("{}/search?q={}&format=json&language=auto", self.base_url.trim_end_matches('/'), urlencoding::encode(q));
        let c = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(5)).user_agent("router-rs-framework/0.7").build().map_err(|e| format!("c: {e}"))?;
        let d: serde_json::Value = c.get(&url).send().map_err(|e| format!("r: {e}"))?.json().map_err(|e| format!("j: {e}"))?;
        Ok(d["results"].as_array().unwrap_or(&vec![]).iter().filter_map(|r| { let t=r["title"].as_str()?; let u=r["url"].as_str()?;
            Some(SearchResult{title:t.into(),url:u.into(),snippet:r["content"].as_str().unwrap_or("").chars().take(300).collect(),source:"searxng",score:r["score"].as_f64().unwrap_or(0.5).min(1.0)}) }).collect())
    }
}
