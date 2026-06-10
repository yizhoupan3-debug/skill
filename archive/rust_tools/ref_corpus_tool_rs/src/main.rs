use anyhow::Result;
use clap::{Parser, Subcommand};
use ref_corpus_tool_rs::{corpus_stats, default_db_path, index_corpus, search_corpus, IndexOptions};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "ref-corpus", about = "Local PDF ref corpus FTS index + search")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Index PDFs under --corpus into SQLite FTS
    Index {
        #[arg(long)]
        corpus: PathBuf,
        #[arg(long)]
        db: Option<PathBuf>,
        #[arg(long, default_value_t = 1800)]
        max_chars: usize,
        #[arg(long, default_value_t = 200)]
        overlap: usize,
        #[arg(long, default_value = ".")]
        project_root: PathBuf,
        #[arg(long)]
        resume: bool,
    },
    /// BM25 search over indexed chunks
    Search {
        #[arg(long)]
        query: String,
        #[arg(long)]
        db: Option<PathBuf>,
        #[arg(long, default_value_t = 12)]
        limit: usize,
        #[arg(long, default_value = ".")]
        project_root: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Index document/chunk counts
    Stats {
        #[arg(long)]
        db: Option<PathBuf>,
        #[arg(long, default_value = ".")]
        project_root: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

fn resolve_db(db: Option<PathBuf>, project_root: &PathBuf) -> PathBuf {
    db.unwrap_or_else(|| default_db_path(project_root))
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Index {
            corpus,
            db,
            max_chars,
            overlap,
            project_root,
            resume,
        } => {
            let db_path = resolve_db(db, &project_root);
            let stats = index_corpus(&IndexOptions {
                corpus_dir: corpus,
                db_path,
                max_chars,
                overlap,
                resume,
            })?;
            println!("{}", serde_json::to_string_pretty(&stats)?);
        }
        Commands::Search {
            query,
            db,
            limit,
            project_root,
            json,
        } => {
            let db_path = resolve_db(db, &project_root);
            let result = search_corpus(&db_path, &query, limit)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else if result.hits.is_empty() {
                println!("no hits for: {}", result.query);
            } else {
                for hit in &result.hits {
                    println!(
                        "[{:.3}] {} (p~{}) #{} — {}",
                        hit.rank,
                        hit.title,
                        hit.page_hint,
                        hit.chunk_index,
                        hit.snippet
                    );
                }
            }
        }
        Commands::Stats {
            db,
            project_root,
            json,
        } => {
            let db_path = resolve_db(db, &project_root);
            let stats = corpus_stats(&db_path)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&stats)?);
            } else {
                println!(
                    "documents={} chunks={} db={}",
                    stats.documents, stats.chunks, stats.db_path
                );
            }
        }
    }
    Ok(())
}
