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
    #[arg(long)]
    apply: bool,
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
    skill_dir: PathBuf,
    path: String,
    source: String,
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

const INDEX_GATE_SHORTCUTS: [(&str, &str); 9] = [
    ("OpenAI API / 模型 / 官方当前文档", "openai-docs"),
    ("PR 评论 / review comment", "gh-address-comments"),
    ("CI 失败 / GitHub Actions 报红", "gh-fix-ci"),
    ("Sentry 告警 / 线上异常", "sentry"),
    ("根因未知的 bug / 失败 / 报错", "systematic-debugging"),
    ("需要并行 sidecar / 多代理拆分", "subagent-delegation"),
    ("PDF / DOCX / 表格产物", "pdf"),
    ("浏览器实操取证 / 页面交互", "playwright"),
    ("截图 / 页面 / 图表可视核查", "visual-review"),
];

const INDEX_COMMON_LANES: [(&str, &str); 12] = [
    ("已有方案，直接落代码", "plan-to-code"),
    ("重构但不想改行为", "refactoring"),
    ("测试设计 / flaky / 补测试", "test-engineering"),
    ("后端运行时问题", "backend-runtime-debugging"),
    ("前端运行时问题", "frontend-debugging"),
    ("README / ADR / 项目文档", "documentation-engineering"),
    ("构建 / 打包 / 工具链", "build-tooling"),
    ("Git 流程 / 合并 / 推送", "gitx"),
    ("多轮调研 / 对比 / 检索", "information-retrieval"),
    ("科研项目 / 课题下一步", "research-workbench"),
    ("文献梳理 / 搜论文 / novelty check", "literature-synthesis"),
    ("skill 库 / 路由框架自身", "skill-framework-developer"),
];

const INDEX_OVERLAY_SHORTCUTS: [(&str, &str); 3] = [
    ("需要审查问题清单", "code-review"),
    ("需要统一编码规范", "coding-standards"),
    ("需要多轮优化直到收敛", "execution-audit"),
];

fn main() -> Result<(), String> {
    let args = Cli::parse();
    let source_manifest = load_source_manifest(&args.source_manifest)?;
    let health_data = load_health_data(&args.health_manifest)?;
    let docs = load_skill_documents(&args.skills_root)?;
    let skill_entries = collect_skill_entries(&args.skills_root, &docs, &source_manifest)?;
    let bundle = compile_bundle(&docs, &skill_entries, &source_manifest, &health_data)?;

    if args.apply {
        write_bundle(&args.skills_root, &bundle)?;
    }

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

fn write_bundle(skills_root: &Path, bundle: &SkillBundle) -> Result<(), String> {
    write_text_if_changed(
        &skills_root.join("SKILL_ROUTING_REGISTRY.md"),
        &bundle.registry,
    )?;
    write_text_if_changed(&skills_root.join("SKILL_ROUTING_INDEX.md"), &bundle.index)?;
    write_json_if_changed(&skills_root.join("SKILL_MANIFEST.json"), &bundle.manifest)?;
    write_json_if_changed(
        &skills_root.join("SKILL_ROUTING_RUNTIME.json"),
        &bundle.runtime_index,
    )?;
    write_json_if_changed(
        &skills_root.join("SKILL_SHADOW_MAP.json"),
        &bundle.shadow_map,
    )?;
    write_json_if_changed(
        &skills_root.join("SKILL_APPROVAL_POLICY.json"),
        &bundle.approval_policy,
    )?;
    Ok(())
}

fn write_text_if_changed(path: &Path, content: &str) -> Result<(), String> {
    let content = if content.ends_with('\n') {
        content.to_string()
    } else {
        format!("{content}\n")
    };
    if fs::read_to_string(path).ok().as_deref() == Some(content.as_str()) {
        return Ok(());
    }
    fs::write(path, content).map_err(|err| format!("failed writing {}: {err}", path.display()))
}

fn write_json_if_changed(path: &Path, payload: &Value) -> Result<(), String> {
    let content = format!(
        "{}\n",
        serde_json::to_string(payload)
            .map_err(|err| format!("failed formatting {}: {err}", path.display()))?
    );
    write_text_if_changed(path, &content)
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
        if name.starts_with('.') && name != ".system" {
            continue;
        }
        discover_skill_dirs(&path, &mut entries)?;
    }

    Ok(entries)
}

fn discover_skill_dirs(root: &Path, entries: &mut Vec<(String, PathBuf)>) -> Result<(), String> {
    let skill_file = root.join("SKILL.md");
    if skill_file.is_file() {
        let slug = root
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .ok_or_else(|| format!("missing directory name for {}", root.display()))?;
        entries.push((slug, root.to_path_buf()));
        return Ok(());
    }

    let mut children = fs::read_dir(root)
        .map_err(|err| format!("failed reading {}: {err}", root.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed reading {}: {err}", root.display()))?;
    children.sort_by_key(|entry| entry.file_name());

    for child in children {
        let child_path = child.path();
        if !child_path.is_dir() {
            continue;
        }
        let child_name = child.file_name().to_string_lossy().to_string();
        if child_name == "dist" || child_name.starts_with('.') {
            continue;
        }
        discover_skill_dirs(&child_path, entries)?;
    }

    Ok(())
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
            "version": 2,
            "winning_rule": "highest-position-wins",
            "sources": [
                {"name": "system", "position": 0},
                {"name": "vendor", "position": 1},
                {"name": "user", "position": 2},
                {"name": "project", "position": 3},
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
        let source_position = precedence.get(&source).copied().ok_or_else(|| {
            format!("source manifest missing precedence position for source `{source}`")
        })?;
        entries.push(SkillEntry {
            slug: doc.slug.clone(),
            skill_dir: doc.skill_dir.clone(),
            path: repo_relative(skills_root, &doc.skill_dir),
            source,
            source_position,
            routing_layer: string_field(&doc.metadata, "routing_layer"),
            routing_owner: string_field(&doc.metadata, "routing_owner"),
            routing_gate: string_field(&doc.metadata, "routing_gate"),
            session_start: string_field(&doc.metadata, "session_start"),
        });
    }
    Ok(entries)
}

fn build_precedence_map(source_manifest: &Value) -> HashMap<String, i64> {
    let mut result = HashMap::new();
    if let Some(sources) = source_manifest.get("sources").and_then(Value::as_array) {
        for (position, entry) in sources.iter().enumerate() {
            let name = normalize_source_name(
                entry
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("project"),
            );
            let source_position = entry
                .get("position")
                .and_then(Value::as_i64)
                .unwrap_or(position as i64);
            result.insert(name, source_position);
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
    precedence: &HashMap<String, i64>,
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
    let selected_docs = select_manifest_docs(docs, skill_entries);
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
        "trigger_hints",
        "health",
        "source",
        "source_position"
    ]);
    let mut rows = Vec::new();
    let mut skills = Vec::new();

    for doc in selected_docs {
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
        let trigger_hints = extract_trigger_hints(&doc.metadata, &description, &doc.body);
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
            trigger_hints,
            round_one_decimal(health_score),
            source_entry.source,
            source_entry.source_position,
        ]));
    }

    let registry = format!(
        "# Skill Routing Registry\n\n| Skill | Status | P | Layer | Owner | Gate | Source | Health | Description |\n|---|---|---|---|---|---|---|---|---|\n{}\n",
        rows.join("\n")
    );
    Ok((registry, json!({"keys": keys, "skills": skills})))
}

fn select_manifest_docs<'a>(
    docs: &'a [SkillDoc],
    skill_entries: &[SkillEntry],
) -> Vec<&'a SkillDoc> {
    let mut ordered_entries = skill_entries.iter().collect::<Vec<_>>();
    ordered_entries.sort_by(|left, right| {
        left.source_position
            .cmp(&right.source_position)
            .then_with(|| left.path.cmp(&right.path))
    });

    let mut winner_paths = HashMap::new();
    for entry in ordered_entries {
        winner_paths.insert(entry.slug.as_str(), &entry.skill_dir);
    }

    let mut selected = Vec::new();
    let mut seen = HashSet::new();
    for doc in docs {
        if seen.contains(doc.slug.as_str()) {
            continue;
        }
        let Some(winner_path) = winner_paths.get(doc.slug.as_str()) else {
            continue;
        };
        if *winner_path != &doc.skill_dir {
            continue;
        }
        seen.insert(doc.slug.as_str());
        selected.push(doc);
    }
    selected
}

fn build_index(manifest: &Value) -> String {
    let selected = select_runtime_skills(manifest);
    let lookup: HashMap<String, Vec<Value>> = selected
        .iter()
        .map(|skill| (string_at(skill, 0), skill.clone()))
        .collect();
    let mut lines = vec![
        "# Skill Routing Index".to_string(),
        "".to_string(),
        "> Default read order: `skills/SKILL_ROUTING_RUNTIME.json` -> `skills/SKILL_ROUTING_INDEX.md`.".to_string(),
        "> Only open `skills/SKILL_MANIFEST.json` or `skills/SKILL_ROUTING_LAYERS.md` when the first two still leave owner/reroute ambiguity.".to_string(),
        "".to_string(),
        "## Quick gate checklist".to_string(),
    ];
    for (idx, item) in index_checklist().iter().enumerate() {
        lines.push(format!("{}. {}", idx + 1, item));
    }
    lines.extend([
        "".to_string(),
        "## Gate shortcuts".to_string(),
        "| If the task starts with... | Route first | Why |".to_string(),
        "|---|---|---|".to_string(),
    ]);
    for (label, slug) in INDEX_GATE_SHORTCUTS {
        let Some(skill) = lookup.get(slug) else {
            continue;
        };
        lines.push(format!(
            "| {} | `{}` | {} |",
            label,
            slug,
            summarize_text(&string_at(skill, 5), 56)
        ));
    }
    lines.extend([
        "".to_string(),
        "## Common lanes".to_string(),
        "| Common need | Route to | Why |".to_string(),
        "|---|---|---|".to_string(),
    ]);
    for (label, slug) in INDEX_COMMON_LANES {
        let Some(skill) = lookup.get(slug) else {
            continue;
        };
        lines.push(format!(
            "| {} | `{}` | {} |",
            label,
            slug,
            summarize_text(&string_at(skill, 5), 56)
        ));
    }
    lines.extend([
        "".to_string(),
        "## Optional overlays".to_string(),
        "| Add when... | Overlay | Why |".to_string(),
        "|---|---|---|".to_string(),
    ]);
    for (label, slug) in INDEX_OVERLAY_SHORTCUTS {
        let Some(skill) = lookup.get(slug) else {
            continue;
        };
        lines.push(format!(
            "| {} | `{}` | {} |",
            label,
            slug,
            summarize_text(&string_at(skill, 5), 56)
        ));
    }
    lines.extend([
        "".to_string(),
        "Need the full list? Use `skills/SKILL_ROUTING_RUNTIME.json` or `skills/SKILL_MANIFEST.json`."
            .to_string(),
        "Still ambiguous? See `skills/SKILL_ROUTING_LAYERS.md` for owner/reroute exceptions."
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
                value_at(&skill, 7),
                value_at(&skill, 8),
            ])
        })
        .collect::<Vec<_>>();
    json!({
        "version": 2,
        "checklist": index_checklist(),
        "keys": ["slug", "layer", "owner", "gate", "session_start", "summary", "trigger_hints", "health"],
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
        "winning_rule": source_manifest.get("winning_rule").cloned().unwrap_or_else(|| Value::String("highest-position-wins".to_string())),
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

fn extract_trigger_hints(
    metadata: &HashMap<String, Value>,
    description: &str,
    _body: &str,
) -> Vec<String> {
    let mut explicit_trigger_hints = Vec::new();
    if let Some(trigger_hints) = metadata.get("trigger_hints") {
        match trigger_hints {
            Value::String(text) => explicit_trigger_hints.push(text.clone()),
            Value::Array(items) => {
                for item in items {
                    let text = value_to_string(item);
                    if !text.trim().is_empty() {
                        explicit_trigger_hints.push(text);
                    }
                }
            }
            _ => {}
        }
    }

    let mut phrases = Vec::new();
    let mut seen = HashSet::new();

    for item in explicit_trigger_hints {
        push_trigger_phrase(&item, &mut seen, &mut phrases);
    }

    // Explicit frontmatter trigger hints are canonical. Do not auto-enrich them
    // from the skill body, or runtime artifacts will accumulate broad fragments
    // that distort routing.
    if !phrases.is_empty() {
        return phrases.into_iter().take(16).collect();
    }

    // For skills without explicit trigger_hints, keep fallback extraction scoped
    // to the frontmatter description. Mining arbitrary body lines produces broad
    // fragments that destabilize routing.
    let source = description.to_string();

    for capture in quote_regex().captures_iter(&source) {
        if let Some(found) = capture.get(1) {
            push_trigger_phrase(found.as_str(), &mut seen, &mut phrases);
        }
    }

    phrases.truncate(16);
    phrases
}

fn push_trigger_phrase(phrase: &str, seen: &mut HashSet<String>, phrases: &mut Vec<String>) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_skills_root(test_name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "skill-compiler-rs-{test_name}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create temp root");
        root
    }

    fn write_skill(skill_dir: &Path, name: &str) {
        fs::create_dir_all(skill_dir).expect("create skill dir");
        fs::write(
            skill_dir.join("SKILL.md"),
            format!(
                "---\nname: {name}\ndescription: test\nrouting_layer: L1\nrouting_owner: owner\nrouting_gate: none\nsession_start: n/a\n---\n## When to use\n- test\n"
            ),
        )
        .expect("write skill");
    }

    #[test]
    fn iter_skill_dirs_discovers_nested_bundles_and_skips_containers() {
        let skills_root = temp_skills_root("nested-discovery");
        write_skill(&skills_root.join("top-skill"), "top-skill");
        write_skill(
            &skills_root.join("primary-runtime").join("spreadsheets"),
            "spreadsheets",
        );
        fs::create_dir_all(skills_root.join("junk-container").join("nested"))
            .expect("create junk container");

        let discovered = iter_skill_dirs(&skills_root).expect("discover skills");
        let discovered_paths = discovered
            .iter()
            .map(|(_, path)| {
                path.strip_prefix(&skills_root)
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .collect::<Vec<_>>();

        assert_eq!(
            discovered_paths,
            vec![
                "primary-runtime/spreadsheets".to_string(),
                "top-skill".to_string(),
            ]
        );
    }

    #[test]
    fn iter_skill_dirs_discovers_nested_system_skills() {
        let skills_root = temp_skills_root("nested-system");
        write_skill(
            &skills_root.join(".system").join("openai-docs"),
            "openai-docs",
        );

        let discovered = iter_skill_dirs(&skills_root).expect("discover skills");
        let discovered_paths = discovered
            .iter()
            .map(|(_, path)| {
                path.strip_prefix(&skills_root)
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .collect::<Vec<_>>();

        assert_eq!(discovered_paths, vec![".system/openai-docs".to_string()]);
    }

    #[test]
    fn compiler_outputs_new_schema_keys() {
        let skills_root = temp_skills_root("schema-keys");
        fs::create_dir_all(skills_root.join("sample-skill")).expect("create sample skill dir");
        fs::write(
            skills_root.join("sample-skill").join("SKILL.md"),
            "---\nname: sample-skill\ndescription: test\nrouting_layer: L1\nrouting_owner: owner\nrouting_gate: none\nsession_start: n/a\ntrigger_hints:\n  - sample trigger\n---\n## When to use\n- test\n",
        )
        .expect("write skill");

        let docs = load_skill_documents(&skills_root).expect("load skill docs");
        let source_manifest = json!({
            "version": 2,
            "winning_rule": "highest-position-wins",
            "sources": [{"name": "project", "position": 3}],
        });
        let entries =
            collect_skill_entries(&skills_root, &docs, &source_manifest).expect("collect entries");
        let (_, manifest) =
            build_registry_and_manifest(&docs, &entries, &HashMap::new()).expect("manifest");
        let runtime = build_runtime_index(&manifest);
        let shadow_map = build_shadow_map(&entries, &source_manifest);

        assert_eq!(
            manifest["keys"],
            json!([
                "slug",
                "layer",
                "owner",
                "gate",
                "priority",
                "description",
                "session_start",
                "trigger_hints",
                "health",
                "source",
                "source_position"
            ])
        );
        assert_eq!(runtime["keys"][6], json!("trigger_hints"));
        assert_eq!(shadow_map["winning_rule"], json!("highest-position-wins"));
        assert!(manifest["skills"][0][7].is_array());
        assert!(runtime["skills"][0][6].is_array());
    }

    #[test]
    fn compiler_manifest_keeps_only_highest_precedence_duplicate_slug() {
        let skills_root = temp_skills_root("duplicate-slug");
        write_skill(&skills_root.join(".system").join("skill-a"), "skill-a");
        write_skill(&skills_root.join("skill-a"), "skill-a");

        let docs = load_skill_documents(&skills_root).expect("load skill docs");
        let source_manifest = json!({
            "version": 2,
            "winning_rule": "highest-position-wins",
            "sources": [
                {"name": "system", "position": 0},
                {"name": "project", "position": 3},
            ],
        });
        let entries =
            collect_skill_entries(&skills_root, &docs, &source_manifest).expect("collect entries");
        let (_, manifest) =
            build_registry_and_manifest(&docs, &entries, &HashMap::new()).expect("manifest");

        assert_eq!(manifest["skills"].as_array().map(Vec::len), Some(1));
        assert_eq!(manifest["skills"][0][0], json!("skill-a"));
        assert_eq!(manifest["skills"][0][9], json!("project"));
        assert_eq!(manifest["skills"][0][10], json!(3));
    }
}
