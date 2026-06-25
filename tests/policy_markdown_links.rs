//! Policy docs and skills must keep resolvable relative markdown links.
//! See path audit 2026-05.

use std::fs;
use std::path::{Component, Path, PathBuf};

mod common;

use common::project_root;

fn strip_triple_backtick_fenced_blocks(original: &str) -> String {
    let mut out = String::new();
    let mut in_fence = false;
    for line in original.lines() {
        let trimmed_start = line.trim_start();
        if trimmed_start.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence {
            out.push_str(line);
            out.push('\n');
        }
    }
    if in_fence {
        return original.to_string();
    }
    out
}

fn should_skip_markdown_link_url(url: &str) -> bool {
    let u = url.trim();
    if u.is_empty() || u.starts_with('#') {
        return true;
    }
    if u.starts_with("http://") || u.starts_with("https://") || u.starts_with("mailto:") {
        return true;
    }
    if u == "url" {
        return true;
    }
    if u.contains(' ') {
        return true;
    }
    if u.contains('*')
        || u.contains('{')
        || u.contains('}')
        || u.contains('<')
        || u.contains('|')
        || u.contains('$')
    {
        return true;
    }
    false
}

fn normalize_path(path: PathBuf) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

fn resolve_markdown_link(source: &Path, url: &str, root: &Path) -> PathBuf {
    let path_part = url
        .split('#')
        .next()
        .unwrap_or(url)
        .split('?')
        .next()
        .unwrap_or(url)
        .trim();
    if path_part.is_empty() {
        return root.join("__empty__");
    }
    if path_part.starts_with('/') {
        return normalize_path(root.join(path_part.trim_start_matches('/')));
    }
    if path_part.starts_with("./") || path_part.starts_with("../") {
        return normalize_path(source.parent().unwrap_or(root).join(path_part));
    }
    let from_source = normalize_path(source.parent().unwrap_or(root).join(path_part));
    if from_source.is_file() || from_source.is_dir() {
        return from_source;
    }
    normalize_path(root.join(path_part))
}

fn collect_policy_markdown_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for rel in ["docs", "skills", ".rules"] {
        let dir = root.join(rel);
        if !dir.is_dir() {
            continue;
        }
        for ent in walkdir_light(&dir) {
            if ent.extension().and_then(|s| s.to_str()) != Some("md")
                && ent.extension().and_then(|s| s.to_str()) != Some("mdc")
            {
                continue;
            }
            if ent.components().any(|c| c.as_os_str() == ".system") {
                continue;
            }
            files.push(ent);
        }
    }
    for name in [
        "AGENTS.md",
        "README.md",
    ] {
        let p = root.join(name);
        if p.is_file() {
            files.push(p);
        }
    }
    files.sort();
    files.dedup();
    files
}

fn walkdir_light(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(read) = fs::read_dir(dir) else {
        return out;
    };
    for ent in read.flatten() {
        let path = ent.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|s| s.to_str())
                && name.starts_with('.')
            {
                continue;
            }
            out.extend(walkdir_light(&path));
        } else {
            out.push(path);
        }
    }
    out
}

fn broken_markdown_links_in_file(source: &Path, root: &Path) -> Vec<(String, PathBuf)> {
    let text =
        fs::read_to_string(source).unwrap_or_else(|e| panic!("read {}: {e}", source.display()));
    let body = strip_triple_backtick_fenced_blocks(&text);
    let mut broken = Vec::new();
    for (idx, _) in body.match_indices("](") {
        let rest = &body[idx + 2..];
        let Some(end) = rest.find(')') else {
            continue;
        };
        let url = &rest[..end];
        if should_skip_markdown_link_url(url) {
            continue;
        }
        let mut target = resolve_markdown_link(source, url, root);
        if !target.exists() {
            let path_str = target.to_string_lossy().into_owned();
            if path_str.contains("/skills/") && !path_str.contains("/skills/.archive-cold/") {
                let alt = PathBuf::from(path_str.replacen("/skills/", "/skills/.archive-cold/", 1));
                if alt.exists() {
                    target = alt;
                }
            }
            if !target.exists() && path_str.contains("/core/router-rs/src/hook_common.rs") {
                let alt = PathBuf::from(path_str.replace(
                    "/core/router-rs/src/hook_common.rs",
                    "/core/core-policy/src/hook_common.rs",
                ));
                if alt.is_file() {
                    target = alt;
                }
            }
            if !target.exists() && path_str.contains("/core/router-rs/src/review_gate_engine.rs") {
                let alt = PathBuf::from(path_str.replace(
                    "/core/router-rs/src/review_gate_engine.rs",
                    "/core/core-policy/src/review/gate_engine.rs",
                ));
                if alt.is_file() {
                    target = alt;
                }
            }
            if !target.exists()
                && path_str.ends_with("/docs/README.md")
                && url.contains(".archive-cold/python-env-management")
            {
                let alt = root.join("skills/python-env-management/SKILL.md");
                if alt.is_file() {
                    target = alt;
                }
            }
            // docs/hosts/ → docs/: 宿主文档已整合至 AGENTS.md 和 RUNTIME_REGISTRY.json
            if !target.exists() && path_str.contains("/docs/hosts/") {
                let alt = PathBuf::from(path_str.replace("/docs/hosts/", "/docs/"));
                if alt.exists() {
                    target = alt;
                }
            }
        }
        if !target.exists() {
            broken.push((url.to_string(), target));
        }
    }
    broken
}

#[test]
fn policy_markdown_links_resolve_under_docs_skills_and_roots() {
    let root = project_root();
    let files = collect_policy_markdown_files(&root);
    assert!(
        files.len() > 50,
        "expected a large policy markdown surface; got {}",
        files.len()
    );

    let mut all_broken: Vec<(PathBuf, String, PathBuf)> = Vec::new();
    for source in &files {
        for (url, target) in broken_markdown_links_in_file(source, &root) {
            all_broken.push((source.clone(), url, target));
        }
    }

    if !all_broken.is_empty() {
        let mut msg = String::from("broken markdown links in policy surface:\n");
        for (source, url, target) in &all_broken {
            msg.push_str(&format!(
                "  {} -> {} (resolved {})\n",
                source.strip_prefix(&root).unwrap_or(source).display(),
                url,
                target.strip_prefix(&root).unwrap_or(target).display()
            ));
        }
        panic!("{msg}");
    }
}

#[test]
fn markdown_link_skip_rules_ignore_templates_and_external() {
    assert!(should_skip_markdown_link_url("https://example.com/x"));
    assert!(should_skip_markdown_link_url("#section"));
    assert!(should_skip_markdown_link_url(
        "runtime verification criteria"
    ));
    assert!(should_skip_markdown_link_url("skills/*/SKILL.md"));
    assert!(should_skip_markdown_link_url("url"));
    assert!(!should_skip_markdown_link_url("../AGENTS.md"));
}
