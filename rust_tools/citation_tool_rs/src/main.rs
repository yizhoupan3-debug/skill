use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use regex::Regex;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Rust-first citation audit, lint, and reference rendering CLI"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Audit(AuditArgs),
    ClaimLint(ClaimLintArgs),
    Render(RenderArgs),
}

#[derive(Args)]
struct AuditArgs {
    #[arg(long)]
    bib: PathBuf,
    #[arg(long)]
    manuscript: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Markdown)]
    format: OutputFormat,
    #[arg(long, default_value_t = 3)]
    cluster_threshold: usize,
    #[arg(long, value_enum, default_value_t = FailOn::Never)]
    fail_on: FailOn,
}

#[derive(Args)]
struct ClaimLintArgs {
    #[arg(long)]
    manuscript: PathBuf,
    #[arg(long, default_value_t = 3)]
    threshold: usize,
    #[arg(long, value_enum, default_value_t = OutputFormat::Markdown)]
    format: OutputFormat,
    #[arg(long, default_value_t = false)]
    fail_on_findings: bool,
}

#[derive(Args)]
struct RenderArgs {
    #[arg(long)]
    bib: PathBuf,
    #[arg(long, value_enum)]
    style: ReferenceStyle,
    #[arg(long)]
    only: Option<String>,
}

#[derive(Clone, Copy, ValueEnum)]
enum OutputFormat {
    Markdown,
    Json,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum FailOn {
    Never,
    Blocking,
    Warnings,
}

#[derive(Clone, Copy, ValueEnum)]
enum ReferenceStyle {
    Apa,
    Ieee,
    Acm,
    Gbt7714,
}

#[derive(Clone, Debug)]
struct BibEntry {
    entry_type: String,
    key: String,
    fields: BTreeMap<String, String>,
}

impl BibEntry {
    fn get(&self, name: &str) -> String {
        self.fields
            .get(&name.to_ascii_lowercase())
            .map(|value| value.trim().to_string())
            .unwrap_or_default()
    }

    fn doi(&self) -> String {
        normalize_doi(&self.get("doi"))
    }

    fn title_norm(&self) -> String {
        normalize_title(&self.get("title"))
    }

    fn first_author_norm(&self) -> String {
        split_authors(&self.get("author"))
            .first()
            .map(|author| normalize_person(author))
            .unwrap_or_default()
    }

    fn year(&self) -> String {
        self.get("year")
    }
}

#[derive(Serialize)]
struct AuditReport {
    summary: AuditSummary,
    duplicates: Vec<Vec<String>>,
    missing_required_fields: BTreeMap<String, Vec<String>>,
    likely_preprints: Vec<String>,
    missing_doi: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    manuscript_consistency: Option<ManuscriptConsistency>,
}

#[derive(Serialize)]
struct AuditSummary {
    total_entries: usize,
    duplicate_groups: usize,
    entries_with_missing_required_fields: usize,
    likely_preprints: usize,
    article_or_conference_entries_missing_doi: usize,
    blocking_issue_count: usize,
    warning_issue_count: usize,
}

#[derive(Serialize)]
struct ManuscriptConsistency {
    total_in_text_citation_mentions: usize,
    unique_cited_keys: usize,
    missing_in_bibliography: Vec<String>,
    uncited_reference_entries: Vec<String>,
    repeated_citation_keys: BTreeMap<String, usize>,
    dense_citation_sentences: Vec<DenseCitationSentence>,
}

#[derive(Serialize, Clone)]
struct DenseCitationSentence {
    citation_count: usize,
    sentence: String,
    reason: String,
}

#[derive(Serialize)]
struct ClaimFinding {
    sentence: String,
    citation_count: usize,
    reasons: Vec<String>,
}

fn main() -> Result<()> {
    run_cli(Cli::parse())
}

fn run_cli(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Audit(args) => run_audit(args),
        Commands::ClaimLint(args) => run_claim_lint(args),
        Commands::Render(args) => run_render(args),
    }
}

fn run_audit(args: AuditArgs) -> Result<()> {
    let entries = parse_bibtex(&read_text(&args.bib)?)?;
    let manuscript_text = args
        .manuscript
        .as_ref()
        .map(read_text)
        .transpose()
        .with_context(|| "failed to read manuscript")?;
    let report = make_report(&entries, manuscript_text.as_deref(), args.cluster_threshold)?;
    match args.format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
        OutputFormat::Markdown => print!("{}", audit_report_to_markdown(&report)),
    }
    enforce_audit_fail_on(&report, args.fail_on)?;
    Ok(())
}

fn run_claim_lint(args: ClaimLintArgs) -> Result<()> {
    let findings = lint_claims(&read_text(&args.manuscript)?, args.threshold)?;
    match args.format {
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&findings)?),
        OutputFormat::Markdown => print!("{}", claim_findings_to_markdown(&findings)),
    }
    if args.fail_on_findings && !findings.is_empty() {
        bail!("claim citation lint found {} issue(s)", findings.len());
    }
    Ok(())
}

fn run_render(args: RenderArgs) -> Result<()> {
    let entries = parse_bibtex(&read_text(&args.bib)?)?;
    let selected = args.only.map(|keys| {
        keys.split(',')
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty())
            .collect::<BTreeSet<_>>()
    });
    for entry in entries {
        if selected
            .as_ref()
            .is_some_and(|keys| !keys.contains(&entry.key))
        {
            continue;
        }
        println!("[{}] {}", entry.key, render_entry(&entry, args.style));
    }
    Ok(())
}

fn read_text(path: &PathBuf) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
}

fn normalize_whitespace(text: &str) -> String {
    let re = Regex::new(r"\s+").expect("static regex");
    re.replace_all(text, " ").trim().to_string()
}

fn normalize_title(title: &str) -> String {
    normalize_whitespace(&title.replace(['{', '}'], "")).to_lowercase()
}

fn normalize_doi(doi: &str) -> String {
    let mut value = doi.trim().to_string();
    let resolver = Regex::new(r"(?i)^https?://(?:dx\.)?doi\.org/").expect("static regex");
    value = resolver.replace(&value, "").to_string();
    let prefix = Regex::new(r"(?i)^doi:\s*").expect("static regex");
    prefix.replace(&value, "").trim().to_lowercase()
}

fn normalize_person(name: &str) -> String {
    normalize_whitespace(&name.replace(['{', '}'], "")).to_lowercase()
}

fn split_authors(author_field: &str) -> Vec<String> {
    if author_field.trim().is_empty() {
        return Vec::new();
    }
    let re = Regex::new(r"\s+and\s+").expect("static regex");
    re.split(author_field)
        .map(normalize_whitespace)
        .filter(|part| !part.is_empty())
        .collect()
}

fn find_matching_delim(
    text: &str,
    start: usize,
    open_char: char,
    close_char: char,
) -> Result<usize> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut index = chars
        .iter()
        .position(|(offset, _)| *offset == start)
        .ok_or_else(|| anyhow!("invalid delimiter offset {start}"))?;
    let mut depth = 0usize;
    while index < chars.len() {
        let (offset, ch) = chars[index];
        if ch == open_char {
            depth += 1;
        } else if ch == close_char {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Ok(offset);
            }
        } else if ch == '"' {
            index += 1;
            while index < chars.len() {
                let (_, quoted) = chars[index];
                if quoted == '"'
                    && chars.get(index.wrapping_sub(1)).map(|(_, prev)| *prev) != Some('\\')
                {
                    break;
                }
                index += 1;
            }
        }
        index += 1;
    }
    bail!("unmatched delimiter starting at offset {start}")
}

fn parse_value(body: &str, mut pos: usize) -> Result<(String, usize)> {
    while pos < body.len() && body.as_bytes()[pos].is_ascii_whitespace() {
        pos += 1;
    }
    if pos >= body.len() {
        return Ok((String::new(), pos));
    }

    let current = body.as_bytes()[pos] as char;
    if current == '{' {
        let end = find_matching_delim(body, pos, '{', '}')?;
        return Ok((body[pos + 1..end].to_string(), end + 1));
    }
    if current == '"' {
        let mut end = pos + 1;
        let mut escaped = false;
        let mut chunks = String::new();
        while end < body.len() {
            let ch = body.as_bytes()[end] as char;
            if ch == '"' && !escaped {
                return Ok((chunks, end + 1));
            }
            if ch == '\\' && !escaped {
                escaped = true;
            } else {
                chunks.push(ch);
                escaped = false;
            }
            end += 1;
        }
        bail!("unterminated quoted BibTeX value");
    }

    let mut end = pos;
    while end < body.len() {
        let ch = body.as_bytes()[end] as char;
        if ch == ',' || ch == '\n' || ch == '\r' {
            break;
        }
        end += 1;
    }
    Ok((body[pos..end].trim().to_string(), end))
}

fn parse_fields(body: &str) -> Result<BTreeMap<String, String>> {
    let mut fields = BTreeMap::new();
    let mut pos = 0usize;
    while pos < body.len() {
        while pos < body.len()
            && matches!(body.as_bytes()[pos] as char, ' ' | '\t' | '\r' | '\n' | ',')
        {
            pos += 1;
        }
        if pos >= body.len() {
            break;
        }

        let name_start = pos;
        while pos < body.len() {
            let ch = body.as_bytes()[pos] as char;
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':' | '-') {
                pos += 1;
            } else {
                break;
            }
        }
        let field_name = body[name_start..pos].trim().to_ascii_lowercase();
        if field_name.is_empty() {
            break;
        }

        while pos < body.len() && body.as_bytes()[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos >= body.len() || body.as_bytes()[pos] as char != '=' {
            bail!("expected '=' after field '{field_name}'");
        }
        pos += 1;
        let (value, new_pos) = parse_value(body, pos)?;
        pos = new_pos;
        fields.insert(field_name, normalize_whitespace(&value));

        while pos < body.len() && body.as_bytes()[pos] as char != ',' {
            if !body.as_bytes()[pos].is_ascii_whitespace() {
                break;
            }
            pos += 1;
        }
        if pos < body.len() && body.as_bytes()[pos] as char == ',' {
            pos += 1;
        }
    }
    Ok(fields)
}

fn parse_bibtex(text: &str) -> Result<Vec<BibEntry>> {
    let mut entries = Vec::new();
    let mut pos = 0usize;
    let type_re = Regex::new(r"(?is)^@([A-Za-z]+)\s*([({])").expect("static regex");
    while let Some(relative_at) = text[pos..].find('@') {
        let at = pos + relative_at;
        let Some(captures) = type_re.captures(&text[at..]) else {
            pos = at + 1;
            continue;
        };
        let whole = captures.get(0).expect("whole match");
        let entry_type = captures[1].to_ascii_lowercase();
        let opener = captures[2].chars().next().expect("opener");
        let closer = if opener == '(' { ')' } else { '}' };
        let body_start = at + whole.end();
        let body_end = find_matching_delim(text, body_start - 1, opener, closer)?;
        let raw_body = text[body_start..body_end].trim();
        if let Some(comma) = raw_body.find(',') {
            let key = raw_body[..comma].trim().to_string();
            let fields_body = &raw_body[comma + 1..];
            entries.push(BibEntry {
                entry_type,
                key,
                fields: parse_fields(fields_body)?,
            });
        }
        pos = body_end + 1;
    }
    Ok(entries)
}

fn required_fields(entry_type: &str) -> &'static [&'static str] {
    match entry_type {
        "article" => &["author", "title", "journal", "year"],
        "inproceedings" | "conference" => &["author", "title", "booktitle", "year"],
        "book" | "inbook" => &["author", "title", "publisher", "year"],
        "incollection" => &["author", "title", "booktitle", "publisher", "year"],
        "phdthesis" | "mastersthesis" => &["author", "title", "school", "year"],
        "techreport" => &["author", "title", "institution", "year"],
        "misc" | "unpublished" => &["author", "title", "year"],
        _ => &["author", "title", "year"],
    }
}

fn detect_missing_fields(entry: &BibEntry) -> Vec<String> {
    required_fields(&entry.entry_type)
        .iter()
        .filter(|field| entry.get(field).is_empty())
        .map(|field| (*field).to_string())
        .collect()
}

fn is_preprint(entry: &BibEntry) -> bool {
    let haystack = [
        entry.entry_type.clone(),
        entry.get("journal"),
        entry.get("booktitle"),
        entry.get("archiveprefix"),
        entry.get("eprinttype"),
        entry.get("note"),
        entry.get("publisher"),
    ]
    .join(" ")
    .to_lowercase();
    ["arxiv", "biorxiv", "medrxiv", "ssrn", "preprint"]
        .iter()
        .any(|token| haystack.contains(token))
        || entry.entry_type == "unpublished"
}

fn group_duplicates(entries: &[BibEntry]) -> Vec<Vec<BibEntry>> {
    let mut buckets: BTreeMap<String, Vec<BibEntry>> = BTreeMap::new();
    for entry in entries {
        let signature = if !entry.doi().is_empty() {
            format!("doi:{}", entry.doi())
        } else {
            format!(
                "title:{}|year:{}|author:{}",
                entry.title_norm(),
                entry.year(),
                entry.first_author_norm()
            )
        };
        buckets.entry(signature).or_default().push(entry.clone());
    }
    buckets
        .into_values()
        .filter(|group| group.len() > 1)
        .collect()
}

fn extract_manuscript_citation_keys(text: &str) -> Result<Vec<String>> {
    let latex_re = Regex::new(r"\\cite[a-zA-Z*]*\s*(?:\[[^\]]*\]\s*){0,2}\{([^}]*)\}")?;
    let pandoc_re = Regex::new(r"(?m)(?:^|[^\w:-])@([A-Za-z0-9_:.+\-/]+)")?;
    let mut keys = Vec::new();
    for capture in latex_re.captures_iter(text) {
        keys.extend(
            capture[1]
                .split(',')
                .map(normalize_whitespace)
                .filter(|part| !part.is_empty()),
        );
    }
    for capture in pandoc_re.captures_iter(text) {
        keys.push(capture[1].trim().to_string());
    }
    Ok(keys)
}

fn sentence_split(text: &str) -> Result<Vec<String>> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        current.push(ch);
        if matches!(ch, '。' | '！' | '？' | '!' | '?' | '.') {
            let mut saw_boundary_space = false;
            while matches!(chars.peek(), Some(next) if next.is_whitespace()) {
                saw_boundary_space = true;
                chars.next();
            }
            if saw_boundary_space {
                let sentence = normalize_whitespace(&current);
                if !sentence.is_empty() {
                    sentences.push(sentence);
                }
                current.clear();
            }
        }
    }
    for part in current.split("\n\n") {
        let sentence = normalize_whitespace(part);
        if !sentence.is_empty() {
            sentences.push(sentence);
        }
    }
    Ok(sentences)
}

fn count_numeric_items(payload: &str) -> Result<usize> {
    let re = Regex::new(r"\s*,\s*")?;
    Ok(re
        .split(payload)
        .filter(|item| !item.trim().is_empty())
        .count())
}

fn count_author_year_items(payload: &str) -> Result<usize> {
    if !payload.contains(';') {
        return Ok(0);
    }
    let year_re = Regex::new(r"(?:19|20)\d{2}[a-z]?")?;
    Ok(payload
        .split(';')
        .filter(|item| year_re.is_match(item))
        .count())
}

fn count_citations_in_sentence(sentence: &str) -> Result<usize> {
    let latex_re = Regex::new(r"\\cite[a-zA-Z*]*\s*(?:\[[^\]]*\]\s*){0,2}\{([^}]*)\}")?;
    let numeric_re = Regex::new(r"\[(\d+(?:\s*[-,]\s*\d+)*)\]")?;
    let author_year_end_re = Regex::new(r"\(([^()]*(?:19|20)\d{2}[a-z]?[^()]*)\)$")?;
    let mut count = 0usize;
    for capture in latex_re.captures_iter(sentence) {
        count += capture[1]
            .split(',')
            .filter(|part| !part.trim().is_empty())
            .count();
    }
    for capture in numeric_re.captures_iter(sentence) {
        count += count_numeric_items(&capture[1])?;
    }
    for capture in author_year_end_re.captures_iter(sentence) {
        count += count_author_year_items(&capture[1])?;
    }
    Ok(count)
}

fn sentence_cluster_flags(text: &str, threshold: usize) -> Result<Vec<DenseCitationSentence>> {
    let mut flagged = Vec::new();
    for sentence in sentence_split(text)? {
        let count = count_citations_in_sentence(&sentence)?;
        if count >= threshold {
            flagged.push(DenseCitationSentence {
                citation_count: count,
                sentence,
                reason: format!(
                    "sentence ends or contains a dense citation cluster ({count} citations detected)"
                ),
            });
        }
    }
    Ok(flagged)
}

fn make_report(
    entries: &[BibEntry],
    manuscript_text: Option<&str>,
    cluster_threshold: usize,
) -> Result<AuditReport> {
    let missing_required_fields: BTreeMap<String, Vec<String>> = entries
        .iter()
        .filter_map(|entry| {
            let fields = detect_missing_fields(entry);
            (!fields.is_empty()).then(|| (entry.key.clone(), fields))
        })
        .collect();
    let duplicate_groups = group_duplicates(entries);
    let likely_preprints: Vec<String> = entries
        .iter()
        .filter(|entry| is_preprint(entry))
        .map(|entry| entry.key.clone())
        .collect();
    let missing_doi: Vec<String> = entries
        .iter()
        .filter(|entry| {
            entry.doi().is_empty()
                && matches!(
                    entry.entry_type.as_str(),
                    "article" | "inproceedings" | "conference"
                )
        })
        .map(|entry| entry.key.clone())
        .collect();

    let manuscript_consistency = if let Some(text) = manuscript_text {
        let cited_keys = extract_manuscript_citation_keys(text)?;
        let mut cited_counter = BTreeMap::<String, usize>::new();
        for key in &cited_keys {
            *cited_counter.entry(key.clone()).or_default() += 1;
        }
        let cited_set = cited_keys.iter().cloned().collect::<BTreeSet<_>>();
        let bib_keys = entries
            .iter()
            .map(|entry| entry.key.clone())
            .collect::<BTreeSet<_>>();
        Some(ManuscriptConsistency {
            total_in_text_citation_mentions: cited_keys.len(),
            unique_cited_keys: cited_set.len(),
            missing_in_bibliography: cited_set.difference(&bib_keys).cloned().collect(),
            uncited_reference_entries: bib_keys.difference(&cited_set).cloned().collect(),
            repeated_citation_keys: cited_counter
                .into_iter()
                .filter(|(_, count)| *count > 1)
                .collect(),
            dense_citation_sentences: sentence_cluster_flags(text, cluster_threshold)?,
        })
    } else {
        None
    };

    let duplicates: Vec<Vec<String>> = duplicate_groups
        .into_iter()
        .map(|group| group.into_iter().map(|entry| entry.key).collect())
        .collect();
    let blocking_issue_count = duplicates.len()
        + missing_required_fields.len()
        + manuscript_consistency
            .as_ref()
            .map(|consistency| consistency.missing_in_bibliography.len())
            .unwrap_or(0);
    let warning_issue_count = likely_preprints.len()
        + missing_doi.len()
        + manuscript_consistency
            .as_ref()
            .map(|consistency| {
                consistency.uncited_reference_entries.len()
                    + consistency.dense_citation_sentences.len()
            })
            .unwrap_or(0);

    Ok(AuditReport {
        summary: AuditSummary {
            total_entries: entries.len(),
            duplicate_groups: duplicates.len(),
            entries_with_missing_required_fields: missing_required_fields.len(),
            likely_preprints: likely_preprints.len(),
            article_or_conference_entries_missing_doi: missing_doi.len(),
            blocking_issue_count,
            warning_issue_count,
        },
        duplicates,
        missing_required_fields,
        likely_preprints,
        missing_doi,
        manuscript_consistency,
    })
}

fn enforce_audit_fail_on(report: &AuditReport, fail_on: FailOn) -> Result<()> {
    match fail_on {
        FailOn::Never => Ok(()),
        FailOn::Blocking if report.summary.blocking_issue_count > 0 => bail!(
            "citation audit found {} blocking issue(s)",
            report.summary.blocking_issue_count
        ),
        FailOn::Warnings
            if report.summary.blocking_issue_count + report.summary.warning_issue_count > 0 =>
        {
            bail!(
                "citation audit found {} blocking issue(s) and {} warning issue(s)",
                report.summary.blocking_issue_count,
                report.summary.warning_issue_count
            )
        }
        _ => Ok(()),
    }
}

fn print_audit_recommendation(report: &AuditReport, lines: &mut Vec<String>) {
    lines.extend([String::new(), "## Recommended next action".to_string()]);
    if report.summary.blocking_issue_count > 0 {
        lines.push(
            "- Fix blocking citation issues first: duplicates, missing required fields, or cited keys missing from the bibliography.".to_string(),
        );
    } else if report.summary.warning_issue_count > 0 {
        lines.push(
            "- No blocking issues found; review warnings such as preprints, missing DOI, uncited references, or dense citation clusters.".to_string(),
        );
    } else {
        lines.push("- No citation hygiene issues were flagged.".to_string());
    }
}

fn audit_report_to_markdown(report: &AuditReport) -> String {
    let mut lines = vec![
        "# Citation Audit Report".to_string(),
        String::new(),
        "## Summary".to_string(),
    ];
    lines.push(format!(
        "- **total entries**: {}",
        report.summary.total_entries
    ));
    lines.push(format!(
        "- **duplicate groups**: {}",
        report.summary.duplicate_groups
    ));
    lines.push(format!(
        "- **entries with missing required fields**: {}",
        report.summary.entries_with_missing_required_fields
    ));
    lines.push(format!(
        "- **likely preprints**: {}",
        report.summary.likely_preprints
    ));
    lines.push(format!(
        "- **article or conference entries missing doi**: {}",
        report.summary.article_or_conference_entries_missing_doi
    ));
    lines.push(format!(
        "- **blocking issues**: {}",
        report.summary.blocking_issue_count
    ));
    lines.push(format!(
        "- **warning issues**: {}",
        report.summary.warning_issue_count
    ));

    lines.extend([String::new(), "## Duplicate groups".to_string()]);
    if report.duplicates.is_empty() {
        lines.push("- None".to_string());
    } else {
        for (idx, group) in report.duplicates.iter().enumerate() {
            lines.push(format!("- Group {}: {}", idx + 1, group.join(", ")));
        }
    }

    lines.extend([
        String::new(),
        "## Entries missing required fields".to_string(),
    ]);
    if report.missing_required_fields.is_empty() {
        lines.push("- None".to_string());
    } else {
        for (key, fields) in &report.missing_required_fields {
            lines.push(format!("- `{key}`: missing {}", fields.join(", ")));
        }
    }

    lines.extend([String::new(), "## Likely preprints".to_string()]);
    if report.likely_preprints.is_empty() {
        lines.push("- None".to_string());
    } else {
        for key in &report.likely_preprints {
            lines.push(format!("- `{key}`"));
        }
    }

    lines.extend([String::new(), "## Entries missing DOI".to_string()]);
    if report.missing_doi.is_empty() {
        lines.push("- None".to_string());
    } else {
        for key in &report.missing_doi {
            lines.push(format!("- `{key}`"));
        }
    }

    if let Some(consistency) = &report.manuscript_consistency {
        lines.extend([String::new(), "## Manuscript consistency".to_string()]);
        lines.push(format!(
            "- **total in-text citation mentions**: {}",
            consistency.total_in_text_citation_mentions
        ));
        lines.push(format!(
            "- **unique cited keys**: {}",
            consistency.unique_cited_keys
        ));
        lines.extend([String::new(), "### Missing in bibliography".to_string()]);
        if consistency.missing_in_bibliography.is_empty() {
            lines.push("- None".to_string());
        } else {
            for key in &consistency.missing_in_bibliography {
                lines.push(format!("- `{key}`"));
            }
        }
        lines.extend([String::new(), "### Uncited reference entries".to_string()]);
        if consistency.uncited_reference_entries.is_empty() {
            lines.push("- None".to_string());
        } else {
            for key in &consistency.uncited_reference_entries {
                lines.push(format!("- `{key}`"));
            }
        }
        lines.extend([String::new(), "### Dense citation sentences".to_string()]);
        if consistency.dense_citation_sentences.is_empty() {
            lines.push("- None".to_string());
        } else {
            for item in &consistency.dense_citation_sentences {
                lines.push(format!(
                    "- ({} cites) {}",
                    item.citation_count, item.sentence
                ));
            }
        }
    }
    print_audit_recommendation(report, &mut lines);
    lines.join("\n") + "\n"
}

fn flag_sentence(sentence: &str, threshold: usize) -> Result<Option<ClaimFinding>> {
    let latex_re = Regex::new(r"\\cite[a-zA-Z*]*\s*(?:\[[^\]]*\]\s*){0,2}\{([^}]*)\}")?;
    let numeric_re = Regex::new(r"\[(\d+(?:\s*[-,]\s*\d+)*)\]")?;
    let author_year_re = Regex::new(r"\(([^()]*(?:19|20)\d{2}[a-z]?[^()]*)\)")?;
    let mut reasons = Vec::new();
    let mut citation_count = 0usize;
    let mut ending_cluster = false;
    let trimmed_len = sentence
        .trim_end_matches([' ', '.', '。', '!', '！', '?', '？'])
        .len();

    for capture in latex_re.captures_iter(sentence) {
        let matched = capture.get(0).expect("whole match");
        let count = capture[1]
            .split(',')
            .filter(|item| !item.trim().is_empty())
            .count();
        citation_count += count;
        if matched.end() >= trimmed_len {
            ending_cluster = true;
        }
    }
    for capture in numeric_re.captures_iter(sentence) {
        let matched = capture.get(0).expect("whole match");
        let count = count_numeric_items(&capture[1])?;
        citation_count += count;
        if matched.end() >= trimmed_len {
            ending_cluster = true;
        }
    }
    for capture in author_year_re.captures_iter(sentence) {
        let matched = capture.get(0).expect("whole match");
        let count = count_author_year_items(&capture[1])?;
        citation_count += count;
        if count > 0 && matched.end() >= trimmed_len {
            ending_cluster = true;
        }
    }

    if citation_count >= threshold {
        reasons.push(format!(
            "dense citation cluster detected ({citation_count} citations)"
        ));
    }
    if ending_cluster && citation_count >= 2 {
        reasons.push(
            "sentence ends with a stacked citation cluster; consider claim-level placement"
                .to_string(),
        );
    }
    if reasons.is_empty() {
        return Ok(None);
    }
    Ok(Some(ClaimFinding {
        sentence: sentence.to_string(),
        citation_count,
        reasons,
    }))
}

fn lint_claims(text: &str, threshold: usize) -> Result<Vec<ClaimFinding>> {
    let mut findings = Vec::new();
    for sentence in sentence_split(text)? {
        if let Some(finding) = flag_sentence(&sentence, threshold)? {
            findings.push(finding);
        }
    }
    Ok(findings)
}

fn claim_findings_to_markdown(findings: &[ClaimFinding]) -> String {
    let mut lines = vec!["# Claim-to-Citation Lint".to_string(), String::new()];
    if findings.is_empty() {
        lines.push("- No dense or sentence-ending citation clusters were flagged.".to_string());
        return lines.join("\n") + "\n";
    }
    for (idx, finding) in findings.iter().enumerate() {
        lines.push(format!("## Finding {}", idx + 1));
        lines.push(format!("- **citation_count**: {}", finding.citation_count));
        for reason in &finding.reasons {
            lines.push(format!("- **reason**: {reason}"));
        }
        lines.push(format!("- **sentence**: {}", finding.sentence));
        lines.push(String::new());
    }
    lines.join("\n").trim_end().to_string() + "\n"
}

fn strip_tex(text: &str) -> String {
    text.replace(['{', '}'], "").trim().to_string()
}

fn parse_person(name: &str) -> (String, String) {
    let name = strip_tex(name);
    if let Some((last, given)) = name.split_once(',') {
        return (given.trim().to_string(), last.trim().to_string());
    }
    let parts = name.split_whitespace().collect::<Vec<_>>();
    match parts.len() {
        0 => (String::new(), String::new()),
        1 => (String::new(), parts[0].to_string()),
        _ => (
            parts[..parts.len() - 1].join(" "),
            parts[parts.len() - 1].to_string(),
        ),
    }
}

fn initials(given: &str) -> String {
    given
        .replace('-', " ")
        .split_whitespace()
        .filter_map(|part| part.chars().next())
        .map(|ch| format!("{ch}."))
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_authors(entry: &BibEntry, style: ReferenceStyle) -> String {
    let people = split_authors(&entry.get("author"))
        .into_iter()
        .map(|author| parse_person(&author))
        .collect::<Vec<_>>();
    if people.is_empty() {
        return "[Missing author]".to_string();
    }

    match style {
        ReferenceStyle::Apa => {
            let formatted = people
                .iter()
                .map(|(given, last)| {
                    format!("{}, {}", last, initials(given))
                        .trim()
                        .trim_end_matches(',')
                        .to_string()
                })
                .collect::<Vec<_>>();
            join_authors(&formatted, ", & ", ", & ")
        }
        ReferenceStyle::Ieee => people
            .iter()
            .map(|(given, last)| format!("{} {}", initials(given), last).trim().to_string())
            .collect::<Vec<_>>()
            .join(", "),
        ReferenceStyle::Acm => {
            let formatted = people
                .iter()
                .map(|(given, last)| {
                    format!("{}, {}", last, given)
                        .trim()
                        .trim_end_matches(',')
                        .to_string()
                })
                .collect::<Vec<_>>();
            join_authors(&formatted, " and ", ", and ")
        }
        ReferenceStyle::Gbt7714 => {
            let mut formatted = people
                .iter()
                .take(3)
                .map(|(given, last)| {
                    format!("{} {}", last, initials(given).replace(' ', ""))
                        .trim()
                        .to_string()
                })
                .collect::<Vec<_>>();
            if people.len() > 3 {
                formatted.push("et al".to_string());
            }
            formatted.join(", ")
        }
    }
}

fn join_authors(formatted: &[String], two_sep: &str, final_sep: &str) -> String {
    match formatted.len() {
        0 => String::new(),
        1 => formatted[0].clone(),
        2 => format!("{}{}{}", formatted[0], two_sep, formatted[1]),
        _ => format!(
            "{}{}{}",
            formatted[..formatted.len() - 1].join(", "),
            final_sep,
            formatted.last().expect("last author")
        ),
    }
}

fn format_container(entry: &BibEntry) -> String {
    ["journal", "booktitle", "publisher", "school", "institution"]
        .iter()
        .map(|field| strip_tex(&entry.get(field)))
        .find(|value| !value.is_empty())
        .unwrap_or_else(|| "[Missing venue]".to_string())
}

fn format_pages(entry: &BibEntry) -> String {
    strip_tex(&entry.get("pages")).replace("--", "-")
}

fn format_doi(entry: &BibEntry, style: ReferenceStyle) -> String {
    let doi = strip_tex(&entry.get("doi"));
    if doi.is_empty() {
        return String::new();
    }
    if matches!(style, ReferenceStyle::Ieee) {
        format!("doi: {doi}")
    } else {
        format!("https://doi.org/{doi}")
    }
}

fn render_entry(entry: &BibEntry, style: ReferenceStyle) -> String {
    match entry.entry_type.as_str() {
        "article" => format_article(entry, style),
        "inproceedings" | "conference" => format_inproceedings(entry, style),
        "book" => format_book(entry, style),
        _ => {
            let doi = format_doi(entry, style);
            format!(
                "{}. {}. {}. {}. {}",
                format_authors(entry, style),
                strip_tex(&entry.get("year")).if_empty("[Missing year]"),
                strip_tex(&entry.get("title")).if_empty("[Missing title]"),
                format_container(entry),
                doi
            )
            .trim()
            .to_string()
        }
    }
}

fn format_article(entry: &BibEntry, style: ReferenceStyle) -> String {
    let authors = format_authors(entry, style);
    let title = strip_tex(&entry.get("title")).if_empty("[Missing title]");
    let journal = strip_tex(&entry.get("journal")).if_empty("[Missing journal]");
    let year = strip_tex(&entry.get("year")).if_empty("[Missing year]");
    let volume = strip_tex(&entry.get("volume"));
    let number = strip_tex(&entry.get("number")).if_empty(&strip_tex(&entry.get("issue")));
    let pages = format_pages(entry);
    let doi = format_doi(entry, style);

    match style {
        ReferenceStyle::Apa => {
            let mut text = format!("{authors} ({year}). {title}. {journal}");
            if !volume.is_empty() {
                text.push_str(&format!(", {volume}"));
                if !number.is_empty() {
                    text.push_str(&format!("({number})"));
                }
            }
            if !pages.is_empty() {
                text.push_str(&format!(", {pages}"));
            }
            text.push('.');
            append_optional(text, &doi)
        }
        ReferenceStyle::Ieee => {
            let mut parts = vec![format!("{authors}, \"{title},\" {journal}")];
            if !volume.is_empty() {
                parts.push(format!("vol. {volume}"));
            }
            if !number.is_empty() {
                parts.push(format!("no. {number}"));
            }
            if !pages.is_empty() {
                parts.push(format!("pp. {pages}"));
            }
            parts.push(year);
            if !doi.is_empty() {
                parts.push(doi);
            }
            parts.join(", ") + "."
        }
        ReferenceStyle::Acm => {
            let mut text = format!("{authors}. {year}. {title}. {journal}");
            if !volume.is_empty() {
                text.push_str(&format!(" {volume}"));
                if !number.is_empty() {
                    text.push_str(&format!(", {number}"));
                }
            }
            if !pages.is_empty() {
                text.push_str(&format!(", {pages}"));
            }
            text.push('.');
            append_optional(text, &doi)
        }
        ReferenceStyle::Gbt7714 => {
            let mut text = format!("{authors}. {title}[J]. {journal}, {year}");
            if !volume.is_empty() {
                text.push_str(&format!(", {volume}"));
                if !number.is_empty() {
                    text.push_str(&format!("({number})"));
                }
            }
            if !pages.is_empty() {
                text.push_str(&format!(": {pages}"));
            }
            text.push('.');
            append_optional(text, &doi)
        }
    }
}

fn format_inproceedings(entry: &BibEntry, style: ReferenceStyle) -> String {
    let authors = format_authors(entry, style);
    let title = strip_tex(&entry.get("title")).if_empty("[Missing title]");
    let booktitle = strip_tex(&entry.get("booktitle")).if_empty("[Missing proceedings]");
    let year = strip_tex(&entry.get("year")).if_empty("[Missing year]");
    let pages = format_pages(entry);
    let doi = format_doi(entry, style);

    match style {
        ReferenceStyle::Apa => {
            let mut text = format!("{authors} ({year}). {title}. In {booktitle}");
            if !pages.is_empty() {
                text.push_str(&format!(" (pp. {pages})"));
            }
            text.push('.');
            append_optional(text, &doi)
        }
        ReferenceStyle::Ieee => {
            let mut parts = vec![format!("{authors}, \"{title},\" in {booktitle}")];
            if !pages.is_empty() {
                parts.push(format!("pp. {pages}"));
            }
            parts.push(year);
            if !doi.is_empty() {
                parts.push(doi);
            }
            parts.join(", ") + "."
        }
        ReferenceStyle::Acm => {
            let mut text = format!("{authors}. {year}. {title}. In {booktitle}");
            if !pages.is_empty() {
                text.push_str(&format!(", {pages}"));
            }
            text.push('.');
            append_optional(text, &doi)
        }
        ReferenceStyle::Gbt7714 => {
            let mut text = format!("{authors}. {title}[C]//{booktitle}, {year}");
            if !pages.is_empty() {
                text.push_str(&format!(": {pages}"));
            }
            text.push('.');
            append_optional(text, &doi)
        }
    }
}

fn format_book(entry: &BibEntry, style: ReferenceStyle) -> String {
    let authors = format_authors(entry, style);
    let title = strip_tex(&entry.get("title")).if_empty("[Missing title]");
    let publisher = strip_tex(&entry.get("publisher")).if_empty("[Missing publisher]");
    let year = strip_tex(&entry.get("year")).if_empty("[Missing year]");
    let doi = format_doi(entry, style);
    let text = match style {
        ReferenceStyle::Apa => format!("{authors} ({year}). {title}. {publisher}."),
        ReferenceStyle::Ieee => format!("{authors}, {title}. {publisher}, {year}."),
        ReferenceStyle::Acm => format!("{authors}. {year}. {title}. {publisher}."),
        ReferenceStyle::Gbt7714 => format!("{authors}. {title}[M]. {publisher}, {year}."),
    };
    append_optional(text, &doi)
}

fn append_optional(text: String, suffix: &str) -> String {
    if suffix.is_empty() {
        text
    } else {
        format!("{text} {suffix}")
    }
}

trait IfEmpty {
    fn if_empty(self, fallback: &str) -> String;
}

impl IfEmpty for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use std::path::Path;
    use tempfile::tempdir;

    fn fixture_bib() -> String {
        r#"
@article{smith2024,
  author = {Smith, Jane and Doe, John},
  title = {Useful Citation Tooling},
  journal = {Journal of Tools},
  year = {2024},
  doi = {10.1000/tools}
}

@article{smithdup,
  author = {Smith, Jane and Doe, John},
  title = {Useful Citation Tooling},
  journal = {Journal of Tools},
  year = {2024},
  doi = {https://doi.org/10.1000/tools}
}

@inproceedings{missingdoi,
  author = {Roe, Richard},
  title = {Conference Entry},
  booktitle = {Proceedings of Tests},
  year = {2023}
}
"#
        .to_string()
    }

    #[test]
    fn parses_bibtex_and_audits_consistency() {
        let entries = parse_bibtex(&fixture_bib()).expect("parse fixture");
        let report = make_report(
            &entries,
            Some(r"This claim is overpacked \cite{smith2024,missingdoi,missingkey}."),
            3,
        )
        .expect("audit report");

        assert_eq!(report.summary.total_entries, 3);
        assert_eq!(report.summary.duplicate_groups, 1);
        assert_eq!(report.summary.article_or_conference_entries_missing_doi, 1);
        assert_eq!(report.summary.blocking_issue_count, 2);
        assert_eq!(report.summary.warning_issue_count, 3);
        assert_eq!(
            report
                .manuscript_consistency
                .expect("manuscript consistency")
                .missing_in_bibliography,
            vec!["missingkey".to_string()]
        );
    }

    #[test]
    fn claim_lint_flags_dense_sentence() {
        let findings = lint_claims("A broad sentence ends with too many sources [1, 2, 3].", 3)
            .expect("lint findings");

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].citation_count, 3);
        assert!(findings[0].reasons[0].contains("dense citation cluster detected"));
    }

    #[test]
    fn fail_closed_modes_return_errors_when_requested() {
        let entries = parse_bibtex(&fixture_bib()).expect("parse fixture");
        let report = make_report(
            &entries,
            Some(r"This claim is overpacked \cite{smith2024,missingdoi,missingkey}."),
            3,
        )
        .expect("audit report");

        assert!(enforce_audit_fail_on(&report, FailOn::Never).is_ok());
        assert!(enforce_audit_fail_on(&report, FailOn::Blocking).is_err());
        assert!(enforce_audit_fail_on(&report, FailOn::Warnings).is_err());
    }

    #[test]
    fn renders_ieee_reference() {
        let entries = parse_bibtex(&fixture_bib()).expect("parse fixture");
        let rendered = render_entry(&entries[0], ReferenceStyle::Ieee);

        assert!(
            rendered.contains(r#"J. Smith, J. Doe, "Useful Citation Tooling," Journal of Tools"#)
        );
    }

    #[test]
    fn cli_help_lists_migrated_commands() {
        let mut help = Vec::new();
        Cli::command()
            .write_long_help(&mut help)
            .expect("write help");
        let help = String::from_utf8(help).expect("help is utf8");

        assert!(help.contains("audit"));
        assert!(help.contains("claim-lint"));
        assert!(help.contains("render"));
    }

    #[test]
    fn skill_entrypoint_points_to_rust_only() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("rust_tools")
            .parent()
            .expect("repo root");
        let skill_root = repo_root.join("skills/citation-management");
        let skill = fs::read_to_string(skill_root.join("SKILL.md")).expect("read skill doc");

        assert!(!skill_root.join("scripts").exists());
        assert!(skill.contains("rust_tools/citation_tool_rs"));
        assert!(skill.contains("cargo run"));
        assert!(!skill.contains("python3"));
        assert!(!skill.contains(".py"));
    }

    #[test]
    fn file_backed_audit_and_render_work_without_python() {
        let dir = tempdir().expect("temp dir");
        let bib = dir.path().join("refs.bib");
        let manuscript = dir.path().join("draft.md");
        fs::write(&bib, fixture_bib()).expect("write bib");
        fs::write(
            &manuscript,
            r"This claim is overpacked \cite{smith2024,missingdoi,missingkey}.",
        )
        .expect("write manuscript");

        let entries = parse_bibtex(&read_text(&bib).expect("read bib")).expect("parse bib");
        let report = make_report(
            &entries,
            Some(&read_text(&manuscript).expect("read manuscript")),
            3,
        )
        .expect("audit report");
        let audit_stdout = serde_json::to_string_pretty(&report).expect("serialize report");
        assert!(audit_stdout.contains(r#""duplicate_groups": 1"#));
        assert!(audit_stdout.contains(r#""missingkey""#));

        let render_stdout = format!(
            "[{}] {}",
            entries[0].key,
            render_entry(&entries[0], ReferenceStyle::Ieee)
        );
        assert!(
            render_stdout.contains(r#"[smith2024] J. Smith, J. Doe, "Useful Citation Tooling,""#)
        );
    }
}
