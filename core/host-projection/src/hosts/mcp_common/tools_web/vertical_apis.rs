use super::search_engines::{SearchResult, SearchSource};
const UA: &str = "router-rs-framework/0.7";
fn get_json(url: &str) -> Result<serde_json::Value, String> {
    let c = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(10)).user_agent(UA).build().map_err(|e| format!("c: {e}"))?;
    let r = c.get(url).send().map_err(|e| format!("r: {e}"))?; if !r.status().is_success() { return Err(format!("HTTP {}", r.status())); }
    r.json().map_err(|e| format!("j: {e}"))
}
fn un(s: &str) -> String { s.replace("&amp;","&").replace("&lt;","<").replace("&gt;",">").replace("&quot;","\"").replace("&#39;","'") }
fn strip(s: &str) -> String { s.chars().fold((String::new(),false),|(mut o,t),c| match c{'<' =>(o,true),'>' =>(o,false),_ if !t=>{o.push(c);(o,t)},_=>(o,t)}).0 }
fn xml_t(x: &str, tag: &str) -> Option<String> { let s=format!("<{}>",tag); let e=format!("</{}>",tag); let a=x.find(&s)?+s.len(); let b=x.find(&e)?; Some(x[a..b].to_string()) }

pub struct StackOverflowSource;
impl SearchSource for StackOverflowSource {
    fn name(&self) -> &'static str { "stackoverflow" }
    fn search(&self, q: &str) -> Result<Vec<SearchResult>, String> {
        let d = get_json(&format!("https://api.stackexchange.com/2.3/search?order=desc&sort=relevance&intitle={}&site=stackoverflow&pagesize=8", urlencoding::encode(q)))?;
        Ok(d["items"].as_array().unwrap_or(&vec![]).iter().filter_map(|i| { let t=i["title"].as_str()?; let l=i["link"].as_str()?; let s=i["score"].as_i64().unwrap_or(0) as f64;
            Some(SearchResult{title:un(t),url:l.into(),snippet:format!("Score: {}, Answers: {}",s,i["answer_count"].as_i64().unwrap_or(0)),source:"stackoverflow",score:s.min(1.0)}) }).collect())
    }
}
pub struct GithubSource;
impl SearchSource for GithubSource {
    fn name(&self) -> &'static str { "github" }
    fn search(&self, q: &str) -> Result<Vec<SearchResult>, String> {
        let d = get_json(&format!("https://api.github.com/search/repositories?q={}&sort=stars&order=desc&per_page=8", urlencoding::encode(q)))?;
        Ok(d["items"].as_array().unwrap_or(&vec![]).iter().filter_map(|i| { let n=i["full_name"].as_str()?; let u=i["html_url"].as_str()?; let st=i["stargazers_count"].as_u64().unwrap_or(0);
            Some(SearchResult{title:n.into(),url:u.into(),snippet:format!("{} stars -- {}",st,i["description"].as_str().unwrap_or("")),source:"github",score:(st as f64).log10().min(1.0)}) }).collect())
    }
}
pub struct HnAlgoliaSource;
impl SearchSource for HnAlgoliaSource {
    fn name(&self) -> &'static str { "hn" }
    fn search(&self, q: &str) -> Result<Vec<SearchResult>, String> {
        let d = get_json(&format!("https://hn.algolia.com/api/v1/search?query={}&tags=story&hitsPerPage=8", urlencoding::encode(q)))?;
        Ok(d["hits"].as_array().unwrap_or(&vec![]).iter().filter_map(|h| { let t=h["title"].as_str()?; let def=format!("https://news.ycombinator.com/item?id={}",h["objectID"].as_str().unwrap_or("")); let u=h["url"].as_str().unwrap_or(&def); let p=h["points"].as_u64().unwrap_or(0);
            Some(SearchResult{title:t.into(),url:u.into(),snippet:format!("{} points",p),source:"hn",score:(p as f64/1000.0).min(1.0)}) }).collect())
    }
}
pub struct WikipediaSource;
impl SearchSource for WikipediaSource {
    fn name(&self) -> &'static str { "wikipedia" }
    fn search(&self, q: &str) -> Result<Vec<SearchResult>, String> {
        let d = get_json(&format!("https://en.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&format=json&srlimit=8", urlencoding::encode(q)))?;
        Ok(d["query"]["search"].as_array().unwrap_or(&vec![]).iter().filter_map(|r| { let t=r["title"].as_str()?;
            Some(SearchResult{title:t.into(),url:format!("https://en.wikipedia.org/wiki/{}",urlencoding::encode(t)),snippet:strip(r["snippet"].as_str().unwrap_or("")),source:"wikipedia",score:0.7}) }).collect())
    }
}
pub struct ArxivSource;
impl SearchSource for ArxivSource {
    fn name(&self) -> &'static str { "arxiv" }
    fn search(&self, q: &str) -> Result<Vec<SearchResult>, String> {
        let c = reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(10)).user_agent(UA).build().map_err(|e| format!("c: {e}"))?;
        let body = c.get(&format!("http://export.arxiv.org/api/query?search_query=all:{}&max_results=8",urlencoding::encode(q))).send().map_err(|e| format!("r: {e}"))?.text().map_err(|e| format!("b: {e}"))?;
        Ok(body.split("<entry>").skip(1).filter_map(|e| { let t=xml_t(e,"title")?; let id=xml_t(e,"id").unwrap_or_default(); let s=xml_t(e,"summary").unwrap_or_default();
            Some(SearchResult{title:t.trim().replace('\n'," "),url:id.trim().into(),snippet:s.trim().chars().take(300).collect(),source:"arxiv",score:0.6}) }).collect())
    }
}
