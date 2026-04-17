use anyhow::Result;
use clap::{Parser, Subcommand};
use reqwest::Client;

#[derive(Parser)]
#[command(author, version, about = "Financial Data Fetcher in Rust", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Fetch financial data
    Fetch {
        #[arg(long)]
        ticker: String,
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Validate data sources
    Validate {
        #[arg(long)]
        sources: Vec<String>,
    },
}

async fn fetch_data(ticker: &str, output: Option<String>) -> Result<()> {
    // Dummy URL for compilation completeness
    let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{}", ticker);
    let client = Client::new();
    let resp = client.get(&url).send().await?.text().await?;

    if let Some(out) = output {
        std::fs::write(&out, resp)?;
        println!("Data for {} saved to {}", ticker, out);
    } else {
        println!("Fetched {} data ({} bytes)", ticker, resp.len());
    }

    Ok(())
}

async fn validate_sources(sources: &[String]) -> Result<()> {
    let client = Client::new();
    let mut tasks = vec![];

    for source in sources {
        let client_clone = client.clone();
        let source_clone = source.clone();
        tasks.push(tokio::spawn(async move {
            let res = client_clone.get(&source_clone).send().await;
            (source_clone, res.is_ok())
        }));
    }

    for task in tasks {
        let (source, is_ok) = task.await?;
        if is_ok {
            println!("[OK] {}", source);
        } else {
            println!("[FAIL] {}", source);
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Fetch { ticker, output } => {
            fetch_data(&ticker, output).await?;
        }
        Commands::Validate { sources } => {
            validate_sources(&sources).await?;
        }
    }

    Ok(())
}
