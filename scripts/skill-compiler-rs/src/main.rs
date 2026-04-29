use clap::Parser;
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Map, Value};
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
    loadouts: Value,
    tiers: Value,
    framework_surface_policy: Value,
}

const INDEX_GATE_SHORTCUTS: [(&str, &str); 7] = [
    ("OpenAI API / 模型 / 官方当前文档", "openai-docs"),
    ("PR 评论 / review comment", "gh-address-comments"),
    ("CI 失败 / GitHub Actions 报红", "gh-fix-ci"),
    ("Sentry 告警 / 线上异常", "sentry"),
    ("根因未知的 bug / 失败 / 报错", "systematic-debugging"),
    ("PDF / DOCX / 表格产物", "pdf"),
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
    ("Git 流程 / 分支合并 / 推送", "gitx"),
    ("多轮调研 / 对比 / 检索", "information-retrieval"),
    ("科研项目 / 课题下一步", "research-workbench"),
    ("文献梳理 / 搜论文 / novelty check", "literature-synthesis"),
    ("skill 库 / 路由框架自身", "skill-framework-developer"),
];

const INDEX_OVERLAY_SHORTCUTS: [(&str, &str); 2] = [
    ("需要审查问题清单", "code-review"),
    ("需要统一编码规范", "coding-standards"),
];

const HOT_FIRST_TURN_OWNER_SLUGS: [&str; 0] = [];
const RUNTIME_EXECUTION_CODE_SLUGS: &[&str] = &[
    "api-design",
    "api-integration-debugging",
    "api-load-tester",
    "architect-review",
    "auth-implementation",
    "backend-runtime-debugging",
    "build-tooling",
    "code-acceleration",
    "code-review",
    "codex-hook-builder",
    "coding-standards",
    "datastore-cache-queue",
    "dependency-migration",
    "docker",
    "env-config-management",
    "error-handling-patterns",
    "github-actions-authoring",
    "idea-to-plan",
    "linux-server-ops",
    "monorepo-tooling",
    "observability",
    "plan-to-code",
    "refactoring",
    "release-engineering",
    "security-audit",
    "security-threat-model",
    "shell-cli",
    "tdd-workflow",
    "test-engineering",
];
const RUNTIME_LANGUAGE_FRAMEWORK_SLUGS: &[&str] = &[
    "accessibility-auditor",
    "chrome-extension-dev",
    "css-pro",
    "frontend-debugging",
    "frontend-design",
    "go-pro",
    "i18n-l10n",
    "javascript-pro",
    "native-app-debugging",
    "nextjs",
    "node-backend",
    "npm-package-authoring",
    "python-pro",
    "react",
    "rust-pro",
    "seo-web",
    "sql-pro",
    "svelte",
    "typescript-pro",
    "vue",
    "web-platform-basics",
];
const RUNTIME_PLATFORM_INTEGRATION_SLUGS: &[&str] = &[
    "agent-memory",
    "chatgpt-apps",
    "cloudflare-deploy",
    "data-wrangling",
    "mcp-builder",
    "performance-expert",
    "prompt-engineer",
    "web-scraping",
];
const FRAMEWORK_COMMAND_RUNTIME_ROWS: [(&str, &str, &str, &[&str], &str); 3] = [
    (
        "autopilot",
        "Run the local framework autopilot supervisor entrypoint.",
        "owner",
        &["$autopilot", "/autopilot"],
        "artifacts/codex-skill-surface/skills/autopilot/SKILL.md",
    ),
    (
        "gitx",
        "Run the safe Git review-fix-tidy-commit-branch-merge-push workflow end to end.",
        "owner",
        &["$gitx", "/gitx", "gitx"],
        "skills/gitx/SKILL.md",
    ),
    (
        "team",
        "Run the local framework team orchestration entrypoint.",
        "owner",
        &["$team", "/team"],
        "artifacts/codex-skill-surface/skills/team/SKILL.md",
    ),
];

const DEFAULT_SURFACE_OWNERS: &[&str] = &[];
const RESEARCH_LOADOUT_OWNERS: &[&str] = &[
    "research-workbench",
    "literature-synthesis",
    "citation-management",
    "paper-workbench",
    "paper-reviewer",
    "paper-reviser",
    "paper-writing",
    "ai-research",
    "autoresearch",
    "experiment-reproducibility",
    "research-engineer",
    "statistical-analysis",
    "scientific-figure-plotting",
];
const IMPLEMENTATION_LOADOUT_OWNERS: &[&str] = &[
    "plan-to-code",
    "systematic-debugging",
    "test-engineering",
    "tdd-workflow",
    "refactoring",
    "build-tooling",
    "dependency-migration",
    "typescript-pro",
    "javascript-pro",
    "python-pro",
    "rust-pro",
    "go-pro",
    "react",
    "nextjs",
    "node-backend",
];
const AUDIT_LOADOUT_OWNERS: &[&str] = &[
    "code-review",
    "architect-review",
    "security-audit",
    "security-threat-model",
    "accessibility-auditor",
    "visual-review",
    "gh-address-comments",
    "gh-fix-ci",
    "sentry",
];
const FRAMEWORK_LOADOUT_OWNERS: &[&str] = &[
    "skill-framework-developer",
    "skill-creator",
    "skill-installer",
    "plugin-creator",
    "agent-memory",
    "agent-swarm-orchestration",
];
const OPS_LOADOUT_OWNERS: &[&str] = &[
    "gitx",
    "release-engineering",
    "github-actions-authoring",
    "cloudflare-deploy",
    "docker",
    "linux-server-ops",
    "observability",
    "env-config-management",
    "shell-cli",
];
const DEFAULT_OVERLAYS: &[&str] = &[];
const IMPLEMENTATION_OVERLAYS: &[&str] = &[
    "coding-standards",
    "error-handling-patterns",
    "tdd-workflow",
];
const AUDIT_OVERLAYS: &[&str] = &["code-review", "security-audit", "i18n-l10n"];
const FRAMEWORK_OVERLAYS: &[&str] = &["code-review"];

fn main() -> Result<(), String> {
    let args = Cli::parse();
    let source_manifest = load_source_manifest(&args.source_manifest)?;
    let health_data = load_health_data(&args.health_manifest)?;
    let docs = load_skill_documents(&args.skills_root)?;
    let skill_entries = collect_skill_entries(&args.skills_root, &docs, &source_manifest)?;
    let bundle = compile_bundle(&docs, &skill_entries, &source_manifest, &health_data)?;
    validate_runtime_contract(&args.skills_root, &bundle)?;

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

    if args.apply {
        println!("{}", build_apply_summary(&bundle));
        return Ok(());
    }

    println!("{}", bundle.registry);
    Ok(())
}

fn validate_runtime_contract(skills_root: &Path, bundle: &SkillBundle) -> Result<(), String> {
    let repo_root = skills_root.parent().unwrap_or(skills_root);
    let manifest_skills = bundle
        .manifest
        .get("skills")
        .and_then(Value::as_array)
        .ok_or_else(|| "manifest missing skills rows".to_string())?;
    let manifest_keys = bundle
        .manifest
        .get("keys")
        .and_then(Value::as_array)
        .ok_or_else(|| "manifest missing keys".to_string())?;
    let manifest_key_index = key_index(manifest_keys);
    let manifest_slug_idx = *manifest_key_index
        .get("slug")
        .ok_or_else(|| "manifest missing slug key".to_string())?;
    let manifest_path_idx = *manifest_key_index
        .get("skill_path")
        .ok_or_else(|| "manifest missing skill_path key".to_string())?;
    let mut manifest_slugs = HashSet::new();
    for row in manifest_skills.iter().filter_map(Value::as_array) {
        let slug = string_at(row, manifest_slug_idx);
        let skill_path = string_at(row, manifest_path_idx);
        if slug.is_empty() || skill_path.is_empty() {
            return Err("manifest row has empty slug or skill_path".to_string());
        }
        let resolved = repo_root.join(&skill_path);
        if !resolved.is_file() {
            return Err(format!(
                "manifest skill `{slug}` points at missing SKILL.md: {skill_path}"
            ));
        }
        manifest_slugs.insert(slug);
    }

    let runtime_skills = bundle
        .runtime_index
        .get("skills")
        .and_then(Value::as_array)
        .ok_or_else(|| "runtime missing skills rows".to_string())?;
    let runtime_keys = bundle
        .runtime_index
        .get("keys")
        .and_then(Value::as_array)
        .ok_or_else(|| "runtime missing keys".to_string())?;
    let runtime_key_index = key_index(runtime_keys);
    let runtime_slug_idx = *runtime_key_index
        .get("slug")
        .ok_or_else(|| "runtime missing slug key".to_string())?;
    let runtime_path_idx = *runtime_key_index
        .get("skill_path")
        .ok_or_else(|| "runtime missing skill_path key".to_string())?;
    for row in runtime_skills.iter().filter_map(Value::as_array) {
        let slug = string_at(row, runtime_slug_idx);
        let skill_path = string_at(row, runtime_path_idx);
        let is_framework_command = FRAMEWORK_COMMAND_RUNTIME_ROWS
            .iter()
            .any(|(framework_slug, _, _, _, _)| *framework_slug == slug);
        if !manifest_slugs.contains(&slug) && !is_framework_command {
            return Err(format!("runtime skill `{slug}` is not in manifest"));
        }
        if !is_framework_command && !repo_root.join(&skill_path).is_file() {
            return Err(format!(
                "runtime skill `{slug}` points at missing SKILL.md: {skill_path}"
            ));
        }
    }
    Ok(())
}

fn key_index(keys: &[Value]) -> HashMap<String, usize> {
    keys.iter()
        .enumerate()
        .filter_map(|(idx, key)| key.as_str().map(|raw| (raw.to_string(), idx)))
        .collect()
}

fn build_apply_summary(bundle: &SkillBundle) -> String {
    let manifest_skill_count = bundle
        .manifest
        .get("skills")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let runtime_skill_count = bundle
        .runtime_index
        .get("skills")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let runtime_keys = bundle
        .runtime_index
        .get("keys")
        .and_then(Value::as_array)
        .map(|keys| {
            keys.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();

    format!(
        "Applied skill routing artifacts.\n- manifest skills: {manifest_skill_count}\n- hot runtime skills: {runtime_skill_count}\n- runtime keys: {runtime_keys}"
    )
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
    write_json_if_changed(&skills_root.join("SKILL_LOADOUTS.json"), &bundle.loadouts)?;
    write_json_if_changed(&skills_root.join("SKILL_TIERS.json"), &bundle.tiers)?;
    let repo_root = skills_root.parent().unwrap_or(skills_root);
    write_json_if_changed(
        &repo_root
            .join("configs")
            .join("framework")
            .join("FRAMEWORK_SURFACE_POLICY.json"),
        &bundle.framework_surface_policy,
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
    let tiers = build_tier_catalog(&manifest);
    let framework_surface_policy = build_framework_surface_policy(&tiers);
    let loadouts = build_loadouts(&framework_surface_policy, &manifest);
    Ok(SkillBundle {
        registry,
        index,
        manifest,
        runtime_index,
        shadow_map,
        approval_policy,
        loadouts,
        tiers,
        framework_surface_policy,
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
        "source_position",
        "skill_path"
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
        let mut health_score = health_info
            .and_then(|value| value.get("dynamic_score"))
            .and_then(value_to_f64)
            .unwrap_or(100.0);
        if !doc.skill_dir.join("SKILL.md").is_file() {
            health_score = health_score.min(0.0_f64);
        }
        if optional_string_field(&doc.metadata, "name").as_deref() != Some(slug.as_str()) {
            health_score = health_score.min(70.0_f64);
        }
        if trigger_hints.is_empty() && matches!(session_start.as_str(), "required" | "preferred") {
            health_score = health_score.min(75.0_f64);
        }
        let indicator = if health_score >= 85.0 {
            "✓"
        } else if health_score >= 60.0 {
            "⚠"
        } else {
            "❌"
        };

        let skill_row = vec![
            json!(slug),
            json!(layer),
            json!(owner),
            json!(gate),
            json!(priority),
            json!(long_summary),
            json!(session_start),
            json!(trigger_hints),
            json!(round_one_decimal(health_score)),
            json!(source_entry.source),
            json!(source_entry.source_position),
            json!(format!("{}/SKILL.md", source_entry.path)),
        ];

        if is_runtime_owned_skill(&string_at(&skill_row, 0)) {
            continue;
        }

        rows.push(format!(
            "| `{}` | {} | {} | {} | {} | {} | {} | {} {:.1} | {} |",
            string_at(&skill_row, 0),
            status,
            string_at(&skill_row, 4),
            string_at(&skill_row, 1),
            string_at(&skill_row, 2),
            string_at(&skill_row, 3),
            source_entry.source,
            indicator,
            health_score,
            summary
        ));
        skills.push(Value::Array(skill_row));
    }

    let registry = format!(
        "# Skill Routing Registry\n\n| Skill | Status | P | Layer | Owner | Gate | Source | Health | Description |\n|---|---|---|---|---|---|---|---|---|\n{}\n",
        rows.join("\n")
    );
    Ok((registry, json!({"keys": keys, "skills": skills})))
}

fn is_runtime_owned_skill(slug: &str) -> bool {
    RUNTIME_EXECUTION_CODE_SLUGS.contains(&slug)
        || RUNTIME_LANGUAGE_FRAMEWORK_SLUGS.contains(&slug)
        || RUNTIME_PLATFORM_INTEGRATION_SLUGS.contains(&slug)
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
    let full_manifest_path = "skills/SKILL_MANIFEST.json";
    let mut skills = selected
        .iter()
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
                string_at(&skill, 4),
                string_at(&skill, 11),
            ])
        })
        .collect::<Vec<_>>();
    for row in framework_command_runtime_rows() {
        skills.push(row);
    }
    json!({
        "version": 2,
        "checklist": index_checklist(),
        "scope": {
            "kind": "hot",
            "policy": "session-start required gates plus explicit framework command aliases; route/search may load the fallback manifest after runtime-owned skills have been excluded.",
            "fallback_manifest": full_manifest_path,
            "full_skill_count": manifest.get("skills").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
            "hot_skill_count": skills.len(),
        },
        "keys": ["slug", "layer", "owner", "gate", "session_start", "summary", "trigger_hints", "health", "priority", "skill_path"],
        "skills": skills,
    })
}

fn framework_command_runtime_rows() -> Vec<Value> {
    FRAMEWORK_COMMAND_RUNTIME_ROWS
        .iter()
        .map(|(slug, summary, owner, trigger_hints, skill_path)| {
            json!([
                slug,
                "L0",
                owner,
                "none",
                "n/a",
                summary,
                trigger_hints,
                100.0,
                "P1",
                skill_path
            ])
        })
        .collect()
}

fn string_list_at(value: &Value, path: &[&str]) -> Vec<String> {
    let mut current = value;
    for key in path {
        current = current.get(*key).unwrap_or(&Value::Null);
    }
    current
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn build_loadouts(surface_policy: &Value, manifest: &Value) -> Value {
    let default_loadouts = string_list_at(surface_policy, &["default_surface", "default_loadouts"]);
    let explicit_opt_in_loadouts = string_list_at(
        surface_policy,
        &["default_surface", "explicit_opt_in_loadouts"],
    );
    let tier_activation_defaults = surface_policy
        .get("default_surface")
        .and_then(|surface| surface.get("tier_activation_defaults"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let slugs = manifest_slug_set(manifest);
    json!({
        "version": 2,
        "schema_version": "skill-loadouts-v2",
        "source": "generated-by-skill-compiler-rs",
        "source_of_truth": false,
        "derived_from": "configs/framework/FRAMEWORK_SURFACE_POLICY.json",
        "report_status": "foldable_generated_report",
        "activation_policy": {
            "default_loadouts": default_loadouts,
            "explicit_opt_in_loadouts": explicit_opt_in_loadouts,
            "experimental_tiers": tier_activation_defaults.get("experimental").cloned().unwrap_or_else(|| json!("explicit_opt_in")),
            "deprecated_tiers": tier_activation_defaults.get("deprecated").cloned().unwrap_or_else(|| json!("disabled")),
            "compatibility_surfaces": "explicit_opt_in"
        },
        "loadouts": {
            "default_surface_loadout": {
                "activation": "default",
                "surface_class": "default",
                "owners": filter_existing_slugs(&slugs, DEFAULT_SURFACE_OWNERS),
                "overlays": filter_existing_slugs(&slugs, DEFAULT_OVERLAYS),
                "exclude": [],
                "purpose": "Single default day-to-day surface; specialized owners route by query instead of default loadout membership."
            },
            "research_loadout": {
                "activation": "explicit",
                "surface_class": "specialist",
                "owners": filter_existing_slugs(&slugs, RESEARCH_LOADOUT_OWNERS),
                "overlays": filter_existing_slugs(&slugs, &["code-review"]),
                "exclude": [],
                "purpose": "Research-project front door plus bounded research, repo investigation, and evidence gathering."
            },
            "implementation_loadout": {
                "activation": "explicit",
                "surface_class": "specialist",
                "owners": filter_existing_slugs(&slugs, IMPLEMENTATION_LOADOUT_OWNERS),
                "overlays": filter_existing_slugs(&slugs, IMPLEMENTATION_OVERLAYS),
                "exclude": [],
                "purpose": "Concrete implementation and refactor execution with test support."
            },
            "audit_loadout": {
                "activation": "explicit",
                "surface_class": "specialist",
                "owners": filter_existing_slugs(&slugs, AUDIT_LOADOUT_OWNERS),
                "overlays": filter_existing_slugs(&slugs, AUDIT_OVERLAYS),
                "exclude": [],
                "purpose": "Strict sign-off, audit, verification, and issue surfacing."
            },
            "framework_loadout": {
                "activation": "explicit",
                "surface_class": "specialist",
                "owners": filter_existing_slugs(&slugs, FRAMEWORK_LOADOUT_OWNERS),
                "overlays": filter_existing_slugs(&slugs, FRAMEWORK_OVERLAYS),
                "exclude": [],
                "purpose": "Framework design, routing policy, orchestrator evolution, and execution-shape normalization work."
            },
            "ops_loadout": {
                "activation": "explicit",
                "surface_class": "specialist",
                "owners": filter_existing_slugs(&slugs, OPS_LOADOUT_OWNERS),
                "overlays": filter_existing_slugs(&slugs, &["code-review"]),
                "exclude": [],
                "purpose": "Operational changes, deployment support, and production diagnostics."
            }
        }
    })
}

fn manifest_slug_set(manifest: &Value) -> HashSet<String> {
    manifest
        .get("skills")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_array)
        .map(|row| string_at(row, 0))
        .filter(|slug| !slug.is_empty())
        .collect()
}

fn filter_existing_slugs(known_slugs: &HashSet<String>, slugs: &[&str]) -> Vec<String> {
    slugs
        .iter()
        .filter(|slug| known_slugs.contains(**slug))
        .map(|slug| (*slug).to_string())
        .collect()
}

fn build_framework_surface_policy(tiers: &Value) -> Value {
    let tier_counts = tiers
        .get("summary")
        .and_then(|summary| summary.get("tier_counts"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let activation_counts = tiers
        .get("summary")
        .and_then(|summary| summary.get("activation_counts"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    json!({
        "version": 1,
        "schema_version": "framework-surface-policy-v1",
        "source": "generated-by-skill-compiler-rs",
        "source_of_truth": true,
        "derived_reports": ["skills/SKILL_TIERS.json"],
        "deprecated_or_foldable_reports": ["skills/SKILL_LOADOUTS.json"],
        "kernel": {
            "canonical_axes": ["routing", "memory", "continuity", "host_projection"],
            "policy": "Keep only routing, memory, continuity, and explicit host projections on the mainline; everything else is an opt-in capability."
        },
        "migration_guardrails": {
            "preserve_rust_runtime_authority": true,
            "avoid_runtime_kernel_fork": true,
            "compatibility_surfaces_explicit_only": true
        },
        "default_surface": {
            "default_loadouts": ["default_surface_loadout"],
            "explicit_opt_in_loadouts": [
                "research_loadout",
                "implementation_loadout",
                "audit_loadout",
                "framework_loadout",
                "ops_loadout"
            ],
            "default_entry_loadout": "default_surface_loadout",
            "lean_default_owners": [],
            "default_overlays": [],
            "tier_activation_defaults": {
                "core": "default",
                "optional": "explicit_opt_in",
                "experimental": "explicit_opt_in",
                "deprecated": "disabled"
            }
        },
        "skill_system": {
            "tier_catalog_path": "skills/SKILL_TIERS.json",
            "loadout_catalog_path": "skills/SKILL_LOADOUTS.json",
            "tier_counts": tier_counts,
            "activation_counts": activation_counts
        },
        "physical_boundaries": {
            "source_roots": ["tools/browser-mcp/src/", "scripts/", "skills/", "docs/", "tests/", "tools/", "configs/"],
            "compiled_output_roots": ["target/", "rust_tools/target/", "scripts/**/target/", "tools/**/dist/", "tools/**/output/"],
            "generated_roots": ["skills/SKILL_*.json", "skills/SKILL_ROUTING_*.md", "AGENTS.md"],
            "session_artifact_roots": [
                "SESSION_SUMMARY.md",
                "NEXT_ACTIONS.json",
                "EVIDENCE_INDEX.json",
                "TRACE_METADATA.json",
                ".supervisor_state.json",
                "artifacts/current/",
                "artifacts/bootstrap/",
                "artifacts/ops/",
                "artifacts/evidence/",
                "artifacts/scratch/"
            ],
            "rules": [
                "Do not mix compiled outputs or scratch runs back into source roots.",
                "Generated routing and Codex host payload artifacts remain replaceable outputs, not authoring sources of truth.",
                "Session continuity stays under root mirrors plus artifacts/current and must not drift into random repo folders."
            ]
        },
        "outcome_metrics": [
            {
                "id": "first_attempt_success_rate",
                "label": "第一次成功率",
                "definition": "在默认面内、不借兼容回退也不补人工热修的情况下，一次执行直接完成任务的比例。"
            },
            {
                "id": "checkpoint_resume_success_rate",
                "label": "断点恢复成功率",
                "definition": "依靠 continuity artifacts 和 resume binding 恢复任务时，能否稳定接回同一 task story 的比例。"
            },
            {
                "id": "new_task_onboarding_cost",
                "label": "新任务接入成本",
                "definition": "把一个新任务接入默认工作流所需的显式配置、额外说明和定制 loadout 成本。"
            }
        ]
    })
}

fn build_shadow_map(skill_entries: &[SkillEntry], source_manifest: &Value) -> Value {
    let mut grouped: HashMap<String, Vec<&SkillEntry>> = HashMap::new();
    for entry in skill_entries {
        if is_runtime_owned_skill(&entry.slug) {
            continue;
        }
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
        if is_runtime_owned_skill(&doc.slug) {
            continue;
        }
        let allowed_tools = normalize_list(doc.metadata.get("allowed_tools"));
        let approval_required_tools = normalize_list(doc.metadata.get("approval_required_tools"));
        let filesystem_scope = doc
            .metadata
            .get("filesystem_scope")
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new()));
        let network_access = doc
            .metadata
            .get("network_access")
            .cloned()
            .unwrap_or_else(|| Value::String("unspecified".to_string()));
        let destructive_risk = doc
            .metadata
            .get("destructive_risk")
            .cloned()
            .unwrap_or_else(|| Value::String("unspecified".to_string()));
        let bridge_behavior = doc
            .metadata
            .get("bridge_behavior")
            .cloned()
            .unwrap_or_else(|| Value::String("default".to_string()));
        let artifact_outputs = normalize_list(doc.metadata.get("artifact_outputs"));

        let mut policy = Map::new();
        if !allowed_tools.is_empty() {
            policy.insert("allowed_tools".to_string(), json!(allowed_tools));
        }
        if !approval_required_tools.is_empty() {
            policy.insert(
                "approval_required_tools".to_string(),
                json!(approval_required_tools),
            );
        }
        if filesystem_scope != Value::Array(Vec::new()) {
            policy.insert("filesystem_scope".to_string(), filesystem_scope);
        }
        if network_access != Value::String("unspecified".to_string()) {
            policy.insert("network_access".to_string(), network_access);
        }
        if destructive_risk != Value::String("unspecified".to_string()) {
            policy.insert("destructive_risk".to_string(), destructive_risk);
        }
        if bridge_behavior != Value::String("default".to_string()) {
            policy.insert("bridge_behavior".to_string(), bridge_behavior);
        }
        if !artifact_outputs.is_empty() {
            policy.insert("artifact_outputs".to_string(), json!(artifact_outputs));
        }
        if !policy.is_empty() {
            skills.insert(doc.slug.clone(), Value::Object(policy));
        }
    }
    json!({
        "version": 2,
        "schema_version": "skill-approval-policy-v2",
        "source": "generated-by-skill-compiler-rs",
        "default_policy": {
            "allowed_tools": [],
            "approval_required_tools": [],
            "filesystem_scope": [],
            "network_access": "unspecified",
            "destructive_risk": "unspecified",
            "bridge_behavior": "default",
            "artifact_outputs": []
        },
        "skills": Value::Object(skills)
    })
}

fn build_tier_catalog(manifest: &Value) -> Value {
    let skills = manifest
        .get("skills")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut core = Vec::new();
    let mut optional = Vec::new();
    let mut experimental = Vec::new();
    let mut deprecated = Vec::new();
    let mut skill_details = Map::new();

    for skill in skills.iter().filter_map(Value::as_array) {
        let slug = string_at(skill, 0);
        if slug.is_empty() {
            continue;
        }
        let health = value_at(skill, 8).as_f64().unwrap_or(100.0);
        let tier = if is_core_surface_skill(skill) {
            core.push(slug.clone());
            "core"
        } else if health < 60.0 {
            deprecated.push(slug.clone());
            "deprecated"
        } else if health < 85.0 {
            experimental.push(slug.clone());
            "experimental"
        } else {
            optional.push(slug.clone());
            "optional"
        };
        skill_details.insert(slug.clone(), build_tier_skill_detail(skill, tier));
    }

    core.sort();
    optional.sort();
    experimental.sort();
    deprecated.sort();
    let tier_counts = json!({
        "core": core.len(),
        "optional": optional.len(),
        "experimental": experimental.len(),
        "deprecated": deprecated.len(),
    });
    let activation_counts = json!({
        "default": core.len(),
        "explicit_opt_in": optional.len() + experimental.len(),
        "disabled": deprecated.len(),
    });

    json!({
        "version": 1,
        "schema_version": "skill-tier-catalog-v1",
        "source": "generated-by-skill-compiler-rs",
        "source_of_truth": false,
        "derived_from": "configs/framework/FRAMEWORK_SURFACE_POLICY.json",
        "report_status": "generated_debug_report",
        "tier_order": ["core", "optional", "experimental", "deprecated"],
        "generation_policy": {
            "core": "session-start required source/artifact/evidence/delegation gate skills only; generic control owners are explicit or fallback-only, not hot-runtime entries",
            "optional": "non-core skills that have not been folded into runtime-owned execution, code, language, framework, platform, or integration capabilities",
            "experimental": "reserved for unstable or low-health routing signals",
            "deprecated": "reserved for very-low-health and unused skills with reroute pressure"
        },
        "surface_policy": {
            "default_loadouts": ["default_surface_loadout"],
            "explicit_opt_in_loadouts": [
                "audit_loadout",
                "framework_loadout",
                "implementation_loadout",
                "ops_loadout",
                "research_loadout"
            ],
            "tier_activation_defaults": {
                "core": "default",
                "optional": "explicit_opt_in",
                "experimental": "explicit_opt_in",
                "deprecated": "disabled"
            }
        },
        "summary": {
            "total_skills": core.len() + optional.len() + experimental.len() + deprecated.len(),
            "tier_counts": tier_counts,
            "activation_counts": activation_counts
        },
        "tiers": {
            "core": core,
            "optional": optional,
            "experimental": experimental,
            "deprecated": deprecated
        },
        "skills": Value::Object(skill_details)
    })
}

fn is_core_surface_skill(skill: &[Value]) -> bool {
    if string_at(skill, 0) == "systematic-debugging" {
        return false;
    }
    string_at(skill, 2) == "gate"
        && string_at(skill, 6) == "required"
        && matches!(
            string_at(skill, 3).as_str(),
            "source" | "artifact" | "evidence" | "delegation"
        )
}

fn build_tier_skill_detail(skill: &[Value], tier: &str) -> Value {
    let core = tier == "core";
    let deprecated = tier == "deprecated";
    let slug = string_at(skill, 0);
    let layer = string_at(skill, 1);
    let owner = string_at(skill, 2);
    let gate = string_at(skill, 3);
    let priority = string_at(skill, 4);
    let session_start = string_at(skill, 6);
    let source = string_at(skill, 9);
    let source_position = value_at(skill, 10);
    let health = value_at(skill, 8);
    json!({
        "tier": tier,
        "reasons": if core {
            vec![
                "owner:gate".to_string(),
                format!("gate:{gate}"),
                "session_start:required".to_string()
            ]
        } else if deprecated {
            vec![
                "health:very-low".to_string(),
                "disabled-until-reroute-reviewed".to_string()
            ]
        } else if tier == "experimental" {
            vec![
                "health:low".to_string(),
                "explicit-opt-in-until-stabilized".to_string()
            ]
        } else {
            vec!["specialist-opt-in".to_string()]
        },
        "surface": {
            "activation_mode": if core {
                "default"
            } else if deprecated {
                "disabled"
            } else {
                "explicit_opt_in"
            },
            "default_surface_enabled": core,
            "default_loadout_memberships": []
        },
        "signals": {
            "layer": layer,
            "owner": owner,
            "gate": gate,
            "priority": priority,
            "session_start": session_start,
            "source": source,
            "source_position": source_position,
            "health": health,
            "loadouts": []
        },
        "slug": slug
    })
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

fn select_hot_runtime_skills(manifest: &Value) -> Vec<Vec<Value>> {
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
            if is_hot_runtime_skill(row) && seen.insert(slug) {
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

fn select_runtime_skills(manifest: &Value) -> Vec<Vec<Value>> {
    let mut selected = select_hot_runtime_skills(manifest);
    if selected.is_empty() {
        selected = select_all_runtime_skills(manifest);
    }
    selected
}

fn select_all_runtime_skills(manifest: &Value) -> Vec<Vec<Value>> {
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

fn is_hot_runtime_skill(skill: &[Value]) -> bool {
    is_core_surface_skill(skill)
        || is_required_delegation_gate(skill)
        || is_first_turn_preferred_owner(skill)
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

fn is_required_delegation_gate(skill: &[Value]) -> bool {
    string_at(skill, 2) == "gate"
        && string_at(skill, 6) == "required"
        && string_at(skill, 3) == "delegation"
}

fn is_first_turn_preferred_owner(skill: &[Value]) -> bool {
    string_at(skill, 2) == "owner"
        && matches!(string_at(skill, 6).as_str(), "required" | "preferred")
        && HOT_FIRST_TURN_OWNER_SLUGS
            .iter()
            .any(|slug| *slug == string_at(skill, 0))
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
        "讨论: extract object / action / constraints / deliverable / success criteria first.",
        "规划: check source, artifact, and evidence gates before owner selection.",
        "规划: choose the narrowest domain owner and add at most one overlay.",
        "执行: take the smallest route delta and do not widen the abstraction.",
        "验证: close with tests, commands, screenshots, artifacts, or an explicit blocker.",
        "Completion pressure changes route context only; it must not change selected owner.",
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
            &skills_root.join(".system").join("skill-installer"),
            "skill-installer",
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

        assert_eq!(
            discovered_paths,
            vec![".system/skill-installer".to_string()]
        );
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
                "source_position",
                "skill_path"
            ])
        );
        assert_eq!(runtime["keys"][6], json!("trigger_hints"));
        assert_eq!(runtime["keys"][8], json!("priority"));
        assert_eq!(shadow_map["winning_rule"], json!("highest-position-wins"));
        assert!(manifest["skills"][0][7].is_array());
        assert!(runtime["skills"][0][6].is_array());
        assert_eq!(runtime["skills"][0][8], json!("P2"));
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

    #[test]
    fn compiler_builds_generated_surface_catalogs_and_hot_runtime_index() {
        let skills_root = temp_skills_root("hot-runtime");
        fs::create_dir_all(skills_root.join("source-gate")).expect("create gate dir");
        fs::write(
            skills_root.join("source-gate").join("SKILL.md"),
            "---\nname: source-gate\ndescription: source gate\nrouting_layer: L0\nrouting_owner: gate\nrouting_gate: source\nrouting_priority: P1\nsession_start: required\ntrigger_hints:\n  - source gate\n---\n## When to use\n- source gate\n",
        )
        .expect("write gate skill");
        fs::create_dir_all(skills_root.join("delegation-gate")).expect("create delegation dir");
        fs::write(
            skills_root.join("delegation-gate").join("SKILL.md"),
            "---\nname: delegation-gate\ndescription: delegation gate\nrouting_layer: L0\nrouting_owner: gate\nrouting_gate: delegation\nrouting_priority: P1\nsession_start: required\n---\n## When to use\n- delegation gate\n",
        )
        .expect("write delegation skill");
        fs::create_dir_all(skills_root.join("preferred-owner")).expect("create preferred dir");
        fs::write(
            skills_root.join("preferred-owner").join("SKILL.md"),
            "---\nname: preferred-owner\ndescription: preferred owner\nrouting_layer: L1\nrouting_owner: owner\nrouting_gate: none\nrouting_priority: P1\nsession_start: preferred\n---\n## When to use\n- preferred owner\n",
        )
        .expect("write preferred owner skill");
        fs::create_dir_all(skills_root.join("plan-to-code")).expect("create plan-to-code dir");
        fs::write(
            skills_root.join("plan-to-code").join("SKILL.md"),
            "---\nname: plan-to-code\ndescription: plan to code\nrouting_layer: L2\nrouting_owner: owner\nrouting_gate: none\nrouting_priority: P1\nsession_start: preferred\n---\n## When to use\n- plan to code\n",
        )
        .expect("write hot owner skill");
        write_skill(&skills_root.join("optional-owner"), "optional-owner");

        let docs = load_skill_documents(&skills_root).expect("load skill docs");
        let source_manifest = json!({
            "version": 2,
            "winning_rule": "highest-position-wins",
            "sources": [{"name": "project", "position": 3}],
        });
        let entries =
            collect_skill_entries(&skills_root, &docs, &source_manifest).expect("collect entries");
        let bundle =
            compile_bundle(&docs, &entries, &source_manifest, &HashMap::new()).expect("compile");

        assert_eq!(bundle.manifest["skills"].as_array().map(Vec::len), Some(4));
        assert!(!bundle.manifest["skills"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row.get(0) == Some(&json!("plan-to-code"))));
        assert_eq!(
            bundle.runtime_index["skills"].as_array().map(Vec::len),
            Some(5)
        );
        assert!(bundle.runtime_index["skills"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row.get(0) == Some(&json!("autopilot"))));
        assert!(bundle.runtime_index["skills"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row.get(0) == Some(&json!("gitx"))));
        assert!(bundle.runtime_index["skills"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row.get(0) == Some(&json!("team"))));
        assert!(!bundle.runtime_index["skills"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row.get(0) == Some(&json!("plan-to-code"))));
        assert!(!bundle.runtime_index["skills"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row.get(0) == Some(&json!("preferred-owner"))));
        assert_eq!(bundle.runtime_index["scope"]["kind"], json!("hot"));
        assert_eq!(bundle.tiers["summary"]["tier_counts"]["core"], json!(2));
        assert_eq!(
            bundle.tiers["summary"]["activation_counts"]["default"],
            json!(2)
        );
        assert_eq!(
            bundle.tiers["skills"]["delegation-gate"]["surface"]["activation_mode"],
            json!("default")
        );
        assert_eq!(
            bundle.tiers["skills"]["preferred-owner"]["surface"]["activation_mode"],
            json!("explicit_opt_in")
        );
        assert!(bundle.tiers["skills"].get("plan-to-code").is_none());
        assert_eq!(
            bundle.loadouts["source"],
            json!("generated-by-skill-compiler-rs")
        );
        assert_eq!(
            bundle.loadouts["derived_from"],
            json!("configs/framework/FRAMEWORK_SURFACE_POLICY.json")
        );
        assert_eq!(
            bundle.loadouts["loadouts"]["default_surface_loadout"]["owners"],
            json!([])
        );
        assert_eq!(bundle.loadouts["source_of_truth"], json!(false));
        assert_eq!(
            bundle.framework_surface_policy["source"],
            json!("generated-by-skill-compiler-rs")
        );
        assert_eq!(
            bundle.framework_surface_policy["source_of_truth"],
            json!(true)
        );
        assert_eq!(
            bundle.framework_surface_policy["derived_reports"],
            json!(["skills/SKILL_TIERS.json"])
        );
        assert_eq!(
            bundle.tiers["derived_from"],
            json!("configs/framework/FRAMEWORK_SURFACE_POLICY.json")
        );
        assert_eq!(
            bundle.tiers["report_status"],
            json!("generated_debug_report")
        );
    }
}
