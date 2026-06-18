//! 研究日志导出模块：JSON / CSV / Obsidian 格式

use crate::db;

/// 导出所有条目为 JSON 格式。
pub fn export_json(conn: &rusqlite::Connection) -> Result<Vec<serde_json::Value>, anyhow::Error> {
    let ids = db::list_entry_ids(conn)?;
    let mut entries = Vec::new();
    for id in &ids {
        if let Some(entry) = db::get_entry(conn, id)? {
            let findings = db::get_findings(conn, id)?;
            let tags = db::get_tags(conn, id)?;
            entries.push(serde_json::json!({
                "entry": entry,
                "findings": findings,
                "tags": tags,
            }));
        }
    }
    Ok(entries)
}

/// 导出所有条目为 CSV 字符串。
pub fn export_csv(conn: &rusqlite::Connection) -> Result<String, anyhow::Error> {
    let ids = db::list_entry_ids(conn)?;
    let mut csv = String::from("id,direction,question,status,created_at,tags\n");
    for id in &ids {
        if let Some(entry) = db::get_entry(conn, id)? {
            let tags = db::get_tags(conn, id)?.join(";");
            csv.push_str(&format!(
                "{},{},{},{},{},{}\n",
                entry.id,
                escape_csv(&entry.direction),
                escape_csv(&entry.question),
                entry.status,
                entry.created_at,
                escape_csv(&tags),
            ));
        }
    }
    Ok(csv)
}

/// 导出所有条目为 Obsidian Markdown 文件列表 (filename → content)。
pub fn export_obsidian(conn: &rusqlite::Connection) -> Result<Vec<(String, String)>, anyhow::Error> {
    let ids = db::list_entry_ids(conn)?;
    let mut files = Vec::new();
    for id in &ids {
        if let Some(entry) = db::get_entry(conn, id)? {
            let findings = db::get_findings(conn, id)?;
            let tags = db::get_tags(conn, id)?;
            let md = crate::text_layer::render_entry(&entry, &findings, &tags);
            let filename = format!("{}.md", id.replace(':', "-"));
            files.push((filename, md));
        }
    }
    Ok(files)
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::models::*;
    use std::path::Path;

    fn setup_test_db() -> rusqlite::Connection {
        let conn = db::init_database(&Path::new(":memory:")).unwrap();
        let entry = Entry {
            id: "exp-test-1".into(),
            direction: "quant".into(),
            question: "CSI300 momentum decay".into(),
            context: None,
            entry_point: ENTRY_POINT_MANUAL.into(),
            barrier_id: None,
            importance: 2,
            status: STATUS_ACTIVE.into(),
            created_at: "2026-06-18T10:00:00Z".into(),
            updated_at: "2026-06-18T10:00:00Z".into(),
        };
        db::insert_entry(&conn, &entry).unwrap();
        db::insert_tags(&conn, "exp-test-1", &["动量".into(), "因子".into()]).unwrap();
        let finding = Finding {
            id: 0,
            entry_id: "exp-test-1".into(),
            kind: FINDING_KIND_INSIGHT.into(),
            content: "衰减速度与波动率正相关".into(),
            confidence: Some(0.85),
            metadata: None,
            created_at: "2026-06-18T11:00:00Z".into(),
        };
        db::insert_finding(&conn, &finding).unwrap();
        conn
    }

    #[test]
    fn test_export_json_basic() {
        let conn = setup_test_db();
        let entries = export_json(&conn).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["entry"]["id"], "exp-test-1");
        assert_eq!(entries[0]["entry"]["direction"], "quant");
        assert_eq!(entries[0]["tags"].as_array().unwrap().len(), 2);
        assert_eq!(entries[0]["findings"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_export_json_empty_db() {
        let conn = db::init_database(&Path::new(":memory:")).unwrap();
        let entries = export_json(&conn).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_export_csv_basic() {
        let conn = setup_test_db();
        let csv = export_csv(&conn).unwrap();
        assert!(csv.starts_with("id,direction,"));
        assert!(csv.contains("exp-test-1"));
        assert!(csv.contains("quant"));
        assert!(csv.contains("动量;因子"));
    }

    #[test]
    fn test_export_csv_special_chars() {
        let conn = db::init_database(&Path::new(":memory:")).unwrap();
        let entry = Entry {
            id: "csv-special".into(),
            direction: "a,b".into(),     // contains comma
            question: r#"他说"你好""#.into(), // contains double quotes
            context: None,
            entry_point: ENTRY_POINT_MANUAL.into(),
            barrier_id: None,
            importance: 0,
            status: STATUS_ACTIVE.into(),
            created_at: "now".into(),
            updated_at: "now".into(),
        };
        db::insert_entry(&conn, &entry).unwrap();
        let csv = export_csv(&conn).unwrap();
        assert!(csv.contains("\"a,b\""), "direction with comma should be quoted");
        assert!(csv.contains(r#""""#), "double quotes should be escaped");
    }

    #[test]
    fn test_export_obsidian_basic() {
        let conn = setup_test_db();
        let files = export_obsidian(&conn).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "exp-test-1.md");
        assert!(files[0].1.contains("quant"));
        assert!(files[0].1.contains("CSI300"));
        assert!(files[0].1.contains("衰减速度"));
    }

    #[test]
    fn test_export_obsidian_empty_db() {
        let conn = db::init_database(&Path::new(":memory:")).unwrap();
        let files = export_obsidian(&conn).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_escape_csv() {
        assert_eq!(escape_csv("normal"), "normal");
        assert_eq!(escape_csv("has,both"), "\"has,both\"");
        assert_eq!(escape_csv("has\"quote"), "\"has\"\"quote\"");
        assert_eq!(escape_csv("has\nnewline"), "\"has\nnewline\"");
    }
}
