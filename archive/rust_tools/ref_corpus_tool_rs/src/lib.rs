pub mod chunk;
pub mod db;
pub mod index;
pub mod search;

pub use index::{default_db_path, index_corpus, corpus_stats, IndexOptions, IndexStats};
pub use search::{search_corpus, SearchHit, SearchResult};
