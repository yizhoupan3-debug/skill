use ref_corpus_tool_rs::db::open;
use ref_corpus_tool_rs::search::search_corpus;
use rusqlite::params;
use tempfile::tempdir;

#[test]
fn fts_search_returns_indexed_chunk() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("index.sqlite");
    let conn = open(&db_path).unwrap();
    conn.execute(
        "INSERT INTO documents(path, sha256, title, page_count, indexed_at) VALUES (?1,?2,?3,?4,?5)",
        params!["paper_ref/pdf/demo.pdf", "abc", "Demo Paper", 3, "2026-01-01T00:00:00Z"],
    )
    .unwrap();
    let doc_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO chunks(doc_id, chunk_index, page_hint, body) VALUES (?1,?2,?3,?4)",
        params![
            doc_id,
            0,
            1,
            "Transformer attention maps support the baseline comparison."
        ],
    )
    .unwrap();
    drop(conn);

    let result = search_corpus(&db_path, "attention baseline", 5).unwrap();
    assert_eq!(result.hits.len(), 1);
    assert_eq!(result.hits[0].title, "Demo Paper");
    assert!(result.hits[0].snippet.contains("attention"));
}
