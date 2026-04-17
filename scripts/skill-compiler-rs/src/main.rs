use clap::Parser;
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Parser, Debug)]
#[command(name = "skill-compiler-rs")]
#[command(about = "Compile skill routing artifacts with a Rust core")]
struct Cli {
    #[arg(long)]
    skills_root: PathBuf,
    #[arg(long)]
    source_manifest: PathBuf,
    #[arg(long)]
    health_manifest: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Clone)]
struct SkillDoc {
    slug: String,
    skill_dir: PathBuf,
    metadata: HashMap<String, Value>,
    body: String,
}

#[derive(Debug, Clone)]
struct SkillEntry {
    slug: String,
    path: String,
    source: String,
    source_priority: Option<i64>,
    source_position: i64,
    routing_layer: String,
    routing_owner: String,
    routing_gate: String,
    session_start: String,
}

#[derive(Debug, Clone, Serialize)]
struct SkillBundle {
    registry: String,
    index: String,
    manifest: Value,
    runtime_index: Value,
    shadow_map: Value,
    approval_policy: Value,
}

fn main() -> Result<(), String> {
    let args = Cli::parse();
    let source_manifest = load_source_manifest(&args.source_manifest)?;
    let health_data = load_health_data(&args.health_manifest)?;
    let docs = load_skill_documents(&args.skills_root)?;
    let skill_entries = collect_skill_entries(&args.skills_root, &docs, &source_manifest)?;
    let bundle = compile_bundle(&docs, &skill_entries, &source_manifest, &health_data)?;

    if args.json {
        println!(
            "{}",
            serde_json::to_string(&bundle)
                .map_err(|err| format!("serialize output failed: {err}"))?
        );
        return Ok(());
    }

    println!("{}", bundle.registry);
    Ok(())
}

fn compile_bundle(
    docs: &[SkillDoc],
    skill_entries: &[SkillEntry],
    source_manifest: &Value,
    health_data: &HashMap<String, Value>,
) -> Result<SkillBundle, String> {
    let (registry, manifest) = build_registry_and_manifest(docs, skill_entries, health_data)?;
    let index = build_index(&manifest);
    let runtime_index = build_runtime_index(&manifest);
    let shadow_map = build_shadow_map(skill_entries, source_manifest);
    let approval_policy = build_approval_policy(docs);
    Ok(SkillBundle {
        registry,
        index,
        manifest,
        runtime_index,
        shadow_map,
        approval_policy,
    })
}

fn load_skill_documents(skills_root: &Path) -> Result<Vec<SkillDoc>, String> {
    let mut docs = Vec::new();
    for (slug, skill_dir) in iter_skill_dirs(skills_root)? {
        let skill_file = skill_dir.join("SKILL.md");
        if !skill_file.is_file() {
            continue;
        }
        let text = fs::read_to_string(&skill_file)
            .map_err(|err| format!("failed reading {}: {err}", skill_file.display()))?;
        let (metadata, body) = parse_frontmatter(&skill_file, &text)?;
        docs.push(SkillDoc {
            slug,
            skill_dir,
            metadata,
            body,
        });
    }
    Ok(docs)
}

fn iter_skill_dirs(skills_root: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let mut entries = Vec::new();
    let mut top_level = fs::read_dir(skills_root)
        .map_err(|err| format!("failed reading {}: {err}", skills_root.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed reading {}: {err}", skills_root.display()))?;
    top_level.sort_by_key(|entry| entry.file_name());

    for entry in top_level {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "dist" {
            continue;
        }
        if name == ".system" {
            let mut system_entries = fs::read_dir(&path)
                .map_err(|err| format!("failed reading {}: {err}", path.display()))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|err| format!("failed reading {}: {err}", path.display()))?;
            system_entries.sort_by_key(|item| item.file_name());
            for system_entry in system_entries {
                let system_path = system_entry.path();
                if system_path.is_dir() {
                    let system_name = system_entry.file_name().to_string_lossy().to_string();
                    entries.push((system_name, system_path));
                }
            }
            continue;
        }
        entries.push((name, path));
    }

    Ok(entries)
}

fn parse_frontmatter(
    skill_file: &Path,
    text: &str,
) -> Result<(HashMap<String, Value>, String), String> {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return Err(format!(
            "{}: missing YAML frontmatter start delimiter",
            skill_file.display()
        ));
    }

    let mut frontmatter_lines = Vec::new();
    let mut found_end = false;
    for line in text.lines().skip(1) {
        if line.trim() == "---" {
            found_end = true;
            break;
        }
        frontmatter_lines.push(line);
    }
    if !found_end {
        return Err(format!(
            "{}: missing YAML frontmatter end delimiter",
            skill_file.display()
        ));
    }

    let frontmatter_text = frontmatter_lines.join("\n");
    let body = text
        .splitn(3, "\n---\n")
        .nth(2)
        .map(|value| value.to_string())
        .unwrap_or_else(|| {
            let mut body_lines = Vec::new();
            let mut delimiters = 0;
            for line in text.lines() {
                if line.trim() == "---" {
                    delimiters += 1;
                    continue;
                }
                if delimiters >= 2 {
                    body_lines.push(line);
                }
            }
            body_lines.join("\n")
        });

    let metadata_yaml: serde_yaml::Value = serde_yaml::from_str(&frontmatter_text)
        .map_err(|err| format!("{}: invalid YAML frontmatter: {err}", skill_file.display()))?;
    let metadata_json = serde_json::to_value(metadata_yaml).map_err(|err| {
        format!(
            "{}: failed converting YAML frontmatter: {err}",
            skill_file.display()
        )
    })?;
    let metadata_obj = metadata_json.as_object().cloned().ok_or_else(|| {
        format!(
            "{}: frontmatter must parse to a mapping",
            skill_file.display()
        )
    })?;

    Ok((metadata_obj.into_iter().collect(), body))
}

fn load_source_manifest(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(json!({
            "version": 1,
            "resolution": "later-source-wins",
            "sources": [
                {"name": "system", "priority": 100, "position": 0},
                {"name": "vendor", "priority": 80, "position": 1},
                {"name": "user", "priority": 60, "position": 2},
                {"name": "project", "priority": 40, "position": 3},
            ],
        }));
    }
    read_json(path)
}

fn load_health_data(path: &Path) -> Result<HashMap<String, Value>, String> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let payload = read_json(path)?;
    let skills = payload.get("skills").cloned().unwrap_or(Value::Null);
    let mut result = HashMap::new();
    match skills {
        Value::Object(map) => {
            for (key, value) in map {
                result.insert(key, value);
            }
        }
        Value::Array(items) => {
            for item in items {
                if let Some(name) = item.get("name").and_then(Value::as_str) {
                    result.insert(name.to_string(), item);
                }
            }
        }
        _ => {}
    }
    Ok(result)
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("failed reading {}: {err}", path.display()))?;
    serde_json::from_str(&text).map_err(|err| format!("failed parsing {}: {err}", path.display()))
}

fn collect_skill_entries(
    skills_root: &Path,
    docs: &[SkillDoc],
    source_manifest: &Value,
) -> Result<Vec<SkillEntry>, String> {
    let precedence = build_precedence_map(source_manifest);
    let mut entries = Vec::new();
    for doc in docs {
        let source = infer_skill_source(skills_root, doc, &precedence)?;
        let source_info = precedence.get(&source).cloned().unwrap_or((None, -1));
        entries.push(SkillEntry {
            slug: doc.slug.clone(),
            path: repo_relative(skills_root, &doc.skill_dir),
            source,
            source_priority: source_info.0,
            source_position: source_info.1,
            routing_layer: string_field(&doc.metadata, "routing_layer"),
            routing_owner: string_field(&doc.metadata, "routing_owner"),
            routing_gate: string_field(&doc.metadata, "routing_gate"),
            session_start: string_field(&doc.metadata, "session_start"),
        });
    }
    Ok(entries)
}

fn build_precedence_map(source_manifest: &Value) -> HashMap<String, (Option<i64>, i64)> {
    let mut result = HashMap::new();
    if let Some(sources) = source_manifest.get("sources").and_then(Value::as_array) {
        for (position, entry) in sources.iter().enumerate() {
            let name = normalize_source_name(
                entry
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("project"),
            );
            let priority = entry.get("priority").and_then(Value::as_i64);
            let source_position = entry
                .get("position")
                .and_then(Value::as_i64)
                .unwrap_or(position as i64);
            result.insert(name, (priority, source_position));
        }
    }
    result
}

fn normalize_source_name(raw: &str) -> String {
    match raw.trim().to_lowercase().as_str() {
        "" => "project".to_string(),
        "local" | "community" | "community-adapted" | "local - trainer" => "project".to_string(),
        other => other.to_string(),
    }
}

fn infer_skill_source(
    skills_root: &Path,
    doc: &SkillDoc,
    precedence: &HashMap<String, (Option<i64>, i64)>,
) -> Result<String, String> {
    let declared = normalize_source_name(
        doc.metadata
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("project"),
    );
    if declared != "project" {
        return Ok(declared);
    }

    let relative = doc.skill_dir.strip_prefix(skills_root).map_err(|err| {
        format!(
            "failed computing relative path for {}: {err}",
            doc.skill_dir.display()
        )
    })?;
    let head = relative
        .components()
        .next()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .unwrap_or_default();
    let inferred = match head.as_str() {
        ".system" => "system",
        "vendor" => "vendor",
        "user" => "user",
        _ => "project",
    };
    if precedence.contains_key(inferred) {
        return Ok(inferred.to_string());
    }
    Ok("project".to_string())
}

fn build_registry_and_manifest(
    docs: &[SkillDoc],
    skill_entries: &[SkillEntry],
    health_data: &HashMap<String, Value>,
) -> Result<(String, Value), String> {
    let source_entries = skill_entries
        .iter()
        .map(|entry| (entry.slug.clone(), entry))
        .collect::<HashMap<_, _>>();
    let keys = json!([
        "slug",
        "layer",
        "owner",
        "gate",
        "priority",
        "description",
        "session_start",
        "triggers",
        "health",
        "source",
        "source_priority"
    ]);
    let mut rows = Vec::new();
    let mut skills = Vec::new();

    for doc in docs {
        for field in [
            "routing_layer",
            "routing_owner",
            "routing_gate",
            "session_start",
        ] {
            if string_field(&doc.metadata, field).is_empty() {
                return Err(format!(
                    "{} missing required routing fields: {}",
                    repo_relative_path(&doc.skill_dir.join("SKILL.md")),
                    field
                ));
            }
        }

        let slug = doc.slug.clone();
        let source_entry = source_entries
            .get(&slug)
            .ok_or_else(|| format!("missing source entry for {}", slug))?;
        let status =
            optional_string_field(&doc.metadata, "status").unwrap_or_else(|| "Active".to_string());
        let priority = optional_string_field(&doc.metadata, "routing_priority")
            .or_else(|| optional_string_field(&doc.metadata, "priority"))
            .unwrap_or_else(|| "P2".to_string());
        let layer = string_field(&doc.metadata, "routing_layer");
        let owner = string_field(&doc.metadata, "routing_owner");
        let gate = string_field(&doc.metadata, "routing_gate");
        let session_start = string_field(&doc.metadata, "session_start");
        let description = optional_string_field(&doc.metadata, "description").unwrap_or_default();
        let trigger_str = extract_triggers(&doc.metadata, &description, &doc.body);
        let summary = pick_runtime_summary(&doc.metadata, 80);
        let long_summary = pick_runtime_summary(&doc.metadata, 200);
        let health_info = health_data.get(&slug);
        let health_score = health_info
            .and_then(|value| value.get("dynamic_score"))
            .and_then(value_to_f64)
            .unwrap_or(100.0);
        let indicator = if health_score >= 85.0 {
            "✓"
        } else if health_score >= 60.0 {
            "⚠"
        } else {
            "❌"
        };

        rows.push(format!(
            "| `{}` | {} | {} | {} | {} | {} | {} | {} {:.1} | {} |",
            slug,
            status,
            priority,
            layer,
            owner,
            gate,
            source_entry.source,
            indicator,
            health_score,
            summary
        ));
        skills.push(json!([
            slug,
            layer,
            owner,
            gate,
            priority,
            long_summary,
            session_start,
            trigger_str,
            round_one_decimal(health_score),
            source_entry.source,
            source_entry.source_priority,
        ]));
    }

    let registry = format!(
        "# Skill Routing Registry\n\n| Skill | Status | P | Layer | Owner | Gate | Source | Health | Description |\n|---|---|---|---|---|---|---|---|---|\n{}\n",
        rows.join("\n")
    );
    Ok((registry, json!({"keys": keys, "skills": skills})))
}

fn build_index(manifest: &Value) -> String {
    let selected = select_runtime_skills(manifest);
    let mut lines = vec![
        "# Skill Routing Index".to_string(),
        "".to_string(),
        "> Entry point for rapid lookup.".to_string(),
        "> Prefer `skills/SKILL_ROUTING_RUNTIME.json` for the lean machine-readable route map.".to_string(),
        "> Prefer `skills/SKILL_MANIFEST.json` for the full manifest (includes owner, priority, source, etc.).".to_string(),
        "> RUNTIME (v2) is a compact 8-key subset: slug, layer, owner, gate, session_start, summary, triggers, health.".to_string(),
        "> MANIFEST is the full 11-key record: slug, layer, owner, gate, priority, description, session_start, triggers, health, source, source_priority.".to_string(),
        "".to_string(),
        "## 6-rule gate checklist".to_string(),
    ];
    for (idx, item) in index_checklist().iter().enumerate() {
        lines.push(format!("{}. {}", idx + 1, item));
    }
    lines.extend([
        "".to_string(),
        "## Gates & Meta".to_string(),
        "| Name | Layer | Owner | Gate | Description |".to_string(),
        "|---|---|---|---|---|".to_string(),
    ]);
    for skill in selected {
        lines.push(format!(
            "| `{}` | {} | {} | {} | {} |",
            string_at(&skill, 0),
            string_at(&skill, 1),
            string_at(&skill, 2),
            string_at(&skill, 3),
            truncate_chars(&string_at(&skill, 5), 80)
        ));
    }
    lines.extend([
        "".to_string(),
        "See `skills/SKILL_ROUTING_LAYERS.md` for the full owner map and reroute rules."
            .to_string(),
        "".to_string(),
    ]);
    lines.join("\n")
}

fn build_runtime_index(manifest: &Value) -> Value {
    let selected = select_runtime_skills(manifest);
    let skills = selected
        .into_iter()
        .map(|skill| {
            json!([
                string_at(&skill, 0),
                string_at(&skill, 1),
                string_at(&skill, 2),
                string_at(&skill, 3),
                string_at(&skill, 6),
                summarize_text(&string_at(&skill, 5), 96),
                string_at(&skill, 7),
                value_at(&skill, 8),
            ])
        })
        .collect::<Vec<_>>();
    json!({
        "version": 2,
        "checklist": index_checklist(),
        "keys": ["slug", "layer", "owner", "gate", "session_start", "summary", "triggers", "health"],
        "skills": skills,
    })
}

fn build_shadow_map(skill_entries: &[SkillEntry], source_manifest: &Value) -> Value {
    let mut grouped: HashMap<String, Vec<&SkillEntry>> = HashMap::new();
    for entry in skill_entries {
        grouped.entry(entry.slug.clone()).or_default().push(entry);
    }

    let mut skills = serde_json::Map::new();
    let mut slugs = grouped.keys().cloned().collect::<Vec<_>>();
    slugs.sort();
    for slug in slugs {
        if let Some(group) = grouped.get(&slug) {
            let mut ordered = group.clone();
            ordered.sort_by(|left, right| {
                left.source_position
                    .cmp(&right.source_position)
                    .then_with(|| left.path.cmp(&right.path))
            });
            let winner = ordered.last().unwrap();
            let shadowed = ordered[..ordered.len().saturating_sub(1)]
                .iter()
                .map(|entry| skill_entry_to_value(entry))
                .collect::<Vec<_>>();
            skills.insert(
                slug,
                json!({
                    "winner": skill_entry_to_value(winner),
                    "shadowed": shadowed,
                    "shadowed_by": if shadowed.is_empty() { Vec::<String>::new() } else { vec![winner.path.clone()] },
                    "has_shadow": !shadowed.is_empty(),
                }),
            );
        }
    }

    json!({
        "version": 1,
        "resolution": source_manifest.get("resolution").cloned().unwrap_or_else(|| Value::String("later-source-wins".to_string())),
        "sources": source_manifest.get("sources").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
        "skills": Value::Object(skills),
    })
}

fn build_approval_policy(docs: &[SkillDoc]) -> Value {
    let mut skills = serde_json::Map::new();
    for doc in docs {
        skills.insert(
            doc.slug.clone(),
            json!({
                "allowed_tools": normalize_list(doc.metadata.get("allowed_tools")),
                "approval_required_tools": normalize_list(doc.metadata.get("approval_required_tools")),
                "filesystem_scope": doc.metadata.get("filesystem_scope").cloned().unwrap_or_else(|| Value::Array(Vec::new())),
                "network_access": doc.metadata.get("network_access").cloned().unwrap_or_else(|| Value::String("unspecified".to_string())),
                "destructive_risk": doc.metadata.get("destructive_risk").cloned().unwrap_or_else(|| Value::String("unspecified".to_string())),
                "bridge_behavior": doc.metadata.get("bridge_behavior").cloned().unwrap_or_else(|| Value::String("default".to_string())),
                "artifact_outputs": normalize_list(doc.metadata.get("artifact_outputs")),
            }),
        );
    }
    json!({"version": 1, "skills": Value::Object(skills)})
}

fn normalize_list(value: Option<&Value>) -> Vec<String> {
    match value {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::String(text)) => vec![text.clone()],
        Some(Value::Array(items)) => items.iter().map(value_to_string).collect(),
        Some(other) => vec![value_to_string(other)],
    }
}

fn skill_entry_to_value(entry: &SkillEntry) -> Value {
    json!({
        "slug": entry.slug,
        "path": entry.path,
        "source": entry.source,
        "source_priority": entry.source_priority,
        "source_position": entry.source_position,
        "routing_layer": entry.routing_layer,
        "routing_owner": entry.routing_owner,
        "routing_gate": entry.routing_gate,
        "session_start": entry.session_start,
    })
}

fn select_runtime_skills(manifest: &Value) -> Vec<Vec<Value>> {
    let mut selected = Vec::new();
    let mut seen = HashSet::new();
    if let Some(skills) = manifest.get("skills").and_then(Value::as_array) {
        for skill in skills {
            let Some(row) = skill.as_array() else {
                continue;
            };
            if row.len() < 6 {
                continue;
            }
            let slug = string_at(row, 0);
            if seen.insert(slug) {
                selected.push(row.clone());
            }
        }
    }

    selected.sort_by(|left, right| {
        runtime_rank(left)
            .cmp(&runtime_rank(right))
            .then_with(|| string_at(left, 0).cmp(&string_at(right, 0)))
    });
    selected
}

fn runtime_rank(skill: &[Value]) -> (i32, i32, i32) {
    let session_rank = match string_at(skill, 6).as_str() {
        "required" => 0,
        "preferred" => 1,
        _ => 2,
    };
    let gate_rank = if string_at(skill, 3) != "none" { 0 } else { 1 };
    let layer_rank = match string_at(skill, 1).as_str() {
        "L0" => 0,
        "L1" => 1,
        "L2" => 2,
        "L3" => 3,
        "L4" => 4,
        _ => 99,
    };
    (session_rank, gate_rank, layer_rank)
}

fn extract_triggers(metadata: &HashMap<String, Value>, description: &str, body: &str) -> String {
    extract_trigger_phrases(metadata, description, body).join(" ")
}

fn extract_trigger_phrases(
    metadata: &HashMap<String, Value>,
    description: &str,
    body: &str,
) -> Vec<String> {
    let noise = [
        "about", "after", "before", "check", "first", "from", "have", "help", "into", "make",
        "need", "only", "that", "them", "then", "this", "use", "user", "when", "with", "work",
        "task", "asks", "request", "using", "used", "best", "good", "real", "will",
    ]
    .into_iter()
    .collect::<HashSet<_>>();

    let mut samples = vec![description.to_string()];
    if let Some(trigger_phrases) = metadata.get("trigger_phrases") {
        match trigger_phrases {
            Value::String(text) => samples.push(text.clone()),
            Value::Array(items) => {
                for item in items {
                    let text = value_to_string(item);
                    if !text.trim().is_empty() {
                        samples.push(text);
                    }
                }
            }
            _ => {}
        }
    }
    samples.extend(body.lines().take(20).map(|line| line.to_string()));
    let source = samples.join("\n");

    let mut phrases = Vec::new();
    let mut seen = HashSet::new();
    let mut push = |phrase: String| {
        let cleaned = collapse_whitespace(phrase.trim_matches(|c: char| {
            matches!(
                c,
                ' ' | '-'
                    | '–'
                    | '—'
                    | '•'
                    | ','
                    | ':'
                    | ';'
                    | '('
                    | ')'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '\''
                    | '"'
                    | '`'
                    | '“'
                    | '”'
                    | '‘'
                    | '’'
                    | '/'
            )
        }));
        if cleaned.chars().count() < 2 {
            return;
        }
        let key = cleaned.to_lowercase();
        if seen.insert(key) {
            phrases.push(cleaned);
        }
    };

    if let Some(trigger_phrases) = metadata.get("trigger_phrases") {
        match trigger_phrases {
            Value::String(text) => push(text.clone()),
            Value::Array(items) => {
                for item in items {
                    push(value_to_string(item));
                }
            }
            _ => {}
        }
    }

    for capture in quote_regex().captures_iter(&source) {
        if let Some(found) = capture.get(1) {
            push(found.as_str().to_string());
        }
    }

    for chunk in chunk_split_regex().split(&source) {
        let chunk = chunk.trim();
        if chunk.is_empty() {
            continue;
        }
        if cjk_chunk_regex().is_match(chunk) {
            let len = chunk.chars().count();
            if (2..=24).contains(&len) {
                push(chunk.to_string());
                continue;
            }
            for found in cjk_extract_regex().find_iter(chunk) {
                push(found.as_str().to_string());
            }
        }
    }

    let lowered = source.to_lowercase();
    for found in english_token_regex().find_iter(&lowered) {
        let token = found.as_str();
        if !noise.contains(token) {
            push(token.to_string());
        }
    }

    phrases.truncate(16);
    phrases
}

fn summarize_text(text: &str, limit: usize) -> String {
    truncate_chars(&collapse_whitespace(text), limit)
}

fn pick_runtime_summary(metadata: &HashMap<String, Value>, limit: usize) -> String {
    let short_description =
        optional_string_field(metadata, "short_description").unwrap_or_default();
    if !short_description.is_empty() {
        return summarize_text(&short_description, limit);
    }
    summarize_text(
        &optional_string_field(metadata, "description").unwrap_or_default(),
        limit,
    )
}

fn collapse_whitespace(text: &str) -> String {
    whitespace_regex().replace_all(text, " ").trim().to_string()
}

fn truncate_chars(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

fn optional_string_field(metadata: &HashMap<String, Value>, key: &str) -> Option<String> {
    metadata.get(key).and_then(|value| {
        let text = value_to_string(value);
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn string_field(metadata: &HashMap<String, Value>, key: &str) -> String {
    optional_string_field(metadata, key).unwrap_or_default()
}

fn round_one_decimal(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.parse::<f64>().ok(),
        _ => None,
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(raw) => raw.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn string_at(row: &[Value], index: usize) -> String {
    row.get(index).map(value_to_string).unwrap_or_default()
}

fn value_at(row: &[Value], index: usize) -> Value {
    row.get(index).cloned().unwrap_or(Value::Null)
}

fn repo_relative(skills_root: &Path, path: &Path) -> String {
    let root = skills_root.parent().unwrap_or(skills_root);
    path.strip_prefix(root)
        .map(|value| value.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

fn repo_relative_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn index_checklist() -> Vec<&'static str> {
    vec![
        "Extract object / action / constraints / deliverable first.",
        "Check source gates before owners when the task starts from external evidence or official docs.",
        "Check artifact gates when the primary object is a PDF, DOCX, XLSX, or similar file artifact.",
        "Check evidence gates when screenshots, rendered pages, browser interaction, or root-cause debugging are central.",
        "Check delegation gate before owner selection when the task is complex and parallel sidecars would help.",
        "Only then choose the narrowest owner and add at most one overlay.",
    ]
}

fn whitespace_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\s+").expect("whitespace regex"))
}

fn quote_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r#"[\"“](.+?)[\"”]"#).expect("quote regex"))
}

fn chunk_split_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"[\n/|,，。；;：:（）()【】\[\]·]+").expect("chunk split regex")
    })
}

fn cjk_chunk_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"[\u{4e00}-\u{9fff}]").expect("cjk chunk regex"))
}

fn cjk_extract_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"[\u{4e00}-\u{9fff}]{2,12}").expect("cjk extract regex"))
}

fn english_token_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"\b[a-zA-Z][a-zA-Z0-9.+#/-]{2,}\b").expect("english token regex")
    })
}
