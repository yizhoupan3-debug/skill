use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

const EUTILS_BASE: &str = "https://eutils.ncbi.nlm.nih.gov";
const PMC_BIOC_BASE: &str = "https://www.ncbi.nlm.nih.gov/research/bionlp/RESTful/pmcoa.cgi/BioC_json";

#[derive(Parser)]
#[command(name = "pubmed")]
#[command(about = "Pure Rust PubMed API Client & Utility")]
struct Cli {
    #[arg(long, help = "Output JSON file path")]
    out: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Search PubMed and return matching PMIDs")]
    Search {
        #[arg(help = "Search query term")]
        query: String,
        #[arg(long, default_value_t = 10, help = "Maximum results to return")]
        max_results: usize,
        #[arg(long, default_value = "relevance", help = "Sort field (relevance, pub_date, etc.)")]
        sort: String,
    },
    #[command(about = "Fetch abstracts and metadata for given PMIDs")]
    Fetch {
        #[arg(help = "Comma-separated list of PMIDs")]
        pmids: String,
    },
    #[command(about = "Retrieve full text for an article from PMC")]
    Fulltext {
        #[arg(help = "PMID of the article")]
        pmid: String,
    },
}

#[derive(Serialize, Deserialize, Debug)]
struct SearchResponse {
    esearchresult: ESearchResult,
}

#[derive(Serialize, Deserialize, Debug)]
struct ESearchResult {
    idlist: Vec<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("Failed to build HTTP client")?;

    let result = match cli.command {
        Commands::Search { query, max_results, sort } => {
            search_pubmed(&client, &query, max_results, &sort)?
        }
        Commands::Fetch { pmids } => {
            fetch_abstracts(&client, &pmids)?
        }
        Commands::Fulltext { pmid } => {
            fetch_pmc_fulltext(&client, &pmid)?
        }
    };

    let mut out_file = File::create(&cli.out)
        .with_context(|| format!("Failed to create output file: {}", cli.out.display()))?;
    
    let formatted = serde_json::to_string_pretty(&result)
        .context("Failed to serialize output JSON")?;
    
    out_file.write_all(formatted.as_bytes())
        .with_context(|| format!("Failed to write output file: {}", cli.out.display()))?;

    eprintln!("Operation completed successfully. Output written to {}", cli.out.display());
    Ok(())
}

fn search_pubmed(client: &Client, query: &str, max_results: usize, sort: &str) -> Result<Value> {
    let url = format!("{}/entrez/eutils/esearch.fcgi", EUTILS_BASE);
    
    let max_results_str = max_results.to_string();
    let mut params = vec![
        ("db", "pubmed"),
        ("term", query),
        ("retmax", &max_results_str),
        ("sort", sort),
        ("retmode", "json"),
    ];

    let api_key = std::env::var("NCBI_API_KEY");
    if let Ok(ref key) = api_key {
        params.push(("api_key", key));
    }

    let response = client.get(&url)
        .query(&params)
        .send()
        .context("Failed to send PubMed search request")?;

    if !response.status().is_success() {
        bail!("NCBI Search API returned error status: {}", response.status());
    }

    let parsed: SearchResponse = response.json()
        .context("Failed to parse NCBI Search response JSON")?;

    Ok(json!(parsed.esearchresult.idlist))
}

fn fetch_abstracts(client: &Client, pmids: &str) -> Result<Value> {
    let url = format!("{}/entrez/eutils/efetch.fcgi", EUTILS_BASE);
    
    let mut params = vec![
        ("db", "pubmed"),
        ("id", pmids),
        ("rettype", "abstract"),
        ("retmode", "xml"),
    ];

    let api_key = std::env::var("NCBI_API_KEY");
    if let Ok(ref key) = api_key {
        params.push(("api_key", key));
    }

    let response = client.get(&url)
        .query(&params)
        .send()
        .context("Failed to send PubMed efetch request")?;

    if !response.status().is_success() {
        bail!("NCBI Fetch API returned error status: {}", response.status());
    }

    let xml_content = response.text()
        .context("Failed to read efetch response body as text")?;

    // As a robust pure Rust blueprint, we parse the basic elements of the XML via a robust regex
    // or standard string parsing to extract PMID, ArticleTitle and Abstract without heavy XML parsing overhead.
    // For a fully-fledged production tool, this can be expanded to full XML node parsing.
    let mut articles = Vec::new();
    
    let mut cursor = 0;
    while let Some(start_idx) = xml_content[cursor..].find("<PubmedArticle>") {
        let actual_start = cursor + start_idx;
        let end_idx = match xml_content[actual_start..].find("</PubmedArticle>") {
            Some(idx) => actual_start + idx + "</PubmedArticle>".len(),
            None => break,
        };
        
        let chunk = &xml_content[actual_start..end_idx];
        cursor = end_idx;

        let pmid = extract_tag_content(chunk, "PMID").unwrap_or_default();
        let title = extract_tag_content(chunk, "ArticleTitle").unwrap_or_default();
        let abstract_text = extract_tag_content(chunk, "AbstractText").unwrap_or_default();

        articles.push(json!({
            "pmid": pmid,
            "title": title,
            "abstract": abstract_text,
        }));
    }

    Ok(json!(articles))
}

fn fetch_pmc_fulltext(client: &Client, pmid: &str) -> Result<Value> {
    let url = format!("{}/{}/unicode", PMC_BIOC_BASE, pmid);
    
    let response = client.get(&url)
        .send()
        .context("Failed to send PMC BioC request")?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        bail!("Article {} not found in PMC Open Access corpus.", pmid);
    }

    if !response.status().is_success() {
        bail!("PMC BioC API returned error status: {}", response.status());
    }

    let data: Value = response.json()
        .context("Failed to parse PMC BioC response JSON")?;

    let mut passages = Vec::new();
    if let Some(docs) = data.as_array() {
        for doc in docs {
            if let Some(documents) = doc.get("documents").and_then(Value::as_array) {
                for document in documents {
                    if let Some(ps) = document.get("passages").and_then(Value::as_array) {
                        for passage in ps {
                            if let Some(text) = passage.get("text").and_then(Value::as_str) {
                                if !text.is_empty() {
                                    passages.push(text.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    } else if let Some(documents) = data.get("documents").and_then(Value::as_array) {
        for document in documents {
            if let Some(ps) = document.get("passages").and_then(Value::as_array) {
                for passage in ps {
                    if let Some(text) = passage.get("text").and_then(Value::as_str) {
                        if !text.is_empty() {
                            passages.push(text.to_string());
                        }
                    }
                }
            }
        }
    }

    Ok(json!({
        "pmid": pmid,
        "full_text": passages.join("\n"),
    }))
}

fn extract_tag_content(xml: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{}", tag);
    let end_tag = format!("</{}>", tag);
    
    let start_idx_raw = xml.find(&start_tag)?;
    let closing_bracket_idx = xml[start_idx_raw..].find('>')?;
    let start_idx = start_idx_raw + closing_bracket_idx + 1;
    
    let end_idx = xml[start_idx..].find(&end_tag)?;
    let content = &xml[start_idx..start_idx + end_idx];
    
    // Clean up any inner HTML/XML tags in e.g. abstracts
    let mut cleaned = String::new();
    let mut in_tag = false;
    for ch in content.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            cleaned.push(ch);
        }
    }
    Some(cleaned.trim().to_string())
}
