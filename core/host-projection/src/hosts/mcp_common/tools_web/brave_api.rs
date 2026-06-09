use super::search_engines::{SearchResult, SearchSource};
pub struct BraveSearchSource;
impl SearchSource for BraveSearchSource {
    fn name(&self) -> &'static str { "brave" }
    fn search(&self, q: &str) -> Result<Vec<SearchResult>, String> {
        let key = std::env::var("BRAVE_API_KEY").map_err(|_| "no key")?;
        let c = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(5)).user_agent("router-rs-framework/0.7").build().map_err(|e| format!("c: {e}"))?;
        let d: serde_json::Value = c.get(&format!("https://api.search.brave.com/res/v1/web/search?q={}&count=8", urlencoding::encode(q)))
            .header("X-Subscription-Token", &key).header("Accept", "application/json").send().map_err(|e| format!("r: {e}"))?.json().map_err(|e| format!("j: {e}"))?;
        Ok(d["web"]["results"].as_array().unwrap_or(&vec![]).iter().filter_map(|r| { let t=r["title"].as_str()?; let u=r["url"].as_str()?;
            Some(SearchResult{title:t.into(),url:u.into(),snippet:r["description"].as_str().unwrap_or("").chars().take(300).collect(),source:"brave",score:0.8}) }).collect())
    }
}
