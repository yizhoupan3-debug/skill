use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

const DATE_FORMAT: &str = "%Y-%m-%d";
const MONTH_FORMAT: &str = "%Y-%m";

/// Ensure artifact directory tree exists.
pub fn ensure_log_dirs(log_root: &Path) -> Result<()> {
    let tags_dir = log_root.join("tags");
    fs::create_dir_all(&tags_dir).context("create tags dir")?;
    Ok(())
}

/// Get the month subdirectory path.
pub fn month_dir(log_root: &Path, date: &chrono::NaiveDate) -> PathBuf {
    log_root.join(date.format(MONTH_FORMAT).to_string())
}

/// Generate daily log file path.
pub fn daily_log_path(log_root: &Path, date: &chrono::NaiveDate, direction: &str) -> PathBuf {
    let safe_name = direction
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '-', "-")
        .trim_matches('-')
        .to_string();
    month_dir(log_root, date).join(format!(
        "{}_{}.md",
        date.format(DATE_FORMAT),
        safe_name
    ))
}

const TEXT_HEADER: &str = r#"# {date}: {direction}

## 初始问题

{question}

## 探索路径

## 关键发现

## 未解决的问题

## 关联 claim / hypothesis

## 下次切入建议
"#;

/// Write a new daily log entry to the text layer.
pub fn write_daily_log(
    log_root: &Path,
    date: &chrono::NaiveDate,
    direction: &str,
    question: &str,
    _log_id: &str,
) -> Result<PathBuf> {
    let dir = month_dir(log_root, date);
    fs::create_dir_all(&dir).context("create month dir")?;
    let path = daily_log_path(log_root, date, direction);

    let content = TEXT_HEADER
        .replace("{date}", &date.format(DATE_FORMAT).to_string())
        .replace("{direction}", direction)
        .replace("{question}", question);

    fs::write(&path, content)
        .with_context(|| format!("write daily log: {}", path.display()))?;

    Ok(path)
}

/// Append insight to an existing log file.
pub fn append_insight(log_path: &Path, text: &str) -> Result<()> {
    let mut content = fs::read_to_string(log_path)
        .with_context(|| format!("read log: {}", log_path.display()))?;
    content.push_str(&format!("\n### Insight\n\n{}\n", text));
    fs::write(log_path, content)
        .with_context(|| format!("write log: {}", log_path.display()))?;
    Ok(())
}

/// Update INDEX.md with a table row for the new log entry.
///
/// Format (per spec §19.5.1): | date | direction | status | tags | barrier |
pub fn update_index(log_root: &Path, direction: &str, date: &chrono::NaiveDate) -> Result<()> {
    let index_path = log_root.join("INDEX.md");

    // Initialize with header and column definitions if not present
    let header = "\
# Research Log Index

| 日期 | 方向 | 状态 | 标签 | 关联 barrier |
|------|------|------|------|-------------|
";
    let existing = if index_path.exists() {
        fs::read_to_string(&index_path)?
    } else {
        header.to_string()
    };

    let row = format!(
        "| {} | {} | active | — | — |\n",
        date.format(DATE_FORMAT),
        direction
    );

    if !existing.contains(&row) {
        fs::write(&index_path, format!("{}{}", existing, row))?;
    }
    Ok(())
}
