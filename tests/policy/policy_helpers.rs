// Shared helper functions and constants extracted from policy_contracts.rs
// for reuse across policy test modules.
#![allow(dead_code)]

use crate::common::read_text;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// Retired skill slugs from the previous runtime-owned skill directory
/// (third-party, code generation, and runtime-vendor skills).
pub const RETIRED_RUNTIME_OWNED_SKILL_SLUGS: &[&str] = &[
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
    "ai-research",
    "chatgpt-apps",
    "cloudflare-deploy",
    "data-wrangling",
    "information-retrieval",
    "literature-synthesis",
    "mcp-builder",
    "performance-expert",
    "prompt-engineer",
    "research-engineer",
    "web-scraping",
];

/// IDs for framework commands that map to runtime-only slugs (not regular skills).
pub const FRAMEWORK_COMMAND_IDS: &[&str] = &[
    "deepinterview",
    "gitx",
    "update",
];

/// Host-agnostic hot-route skills exempt from closed-set host coverage checks.
/// These are Codex-installer-only skills that should not list extra hosts beyond their frontmatter.
pub const HOT_RUNTIME_CODEX_PRODUCT_ONLY_SLUGS: &[&str] =
    &["plugin-creator", "skill-installer", "openai-docs"];

/// Find the index of a named key in a JSON keys array.
///
/// # Panics
/// Panics if `name` is not found in `keys`.
pub fn key_index(keys: &[Value], name: &str) -> usize {
    keys.iter()
        .position(|key| key.as_str() == Some(name))
        .unwrap_or_else(|| panic!("missing key {name}"))
}

/// Find the index of the first matching key among a list of alternatives.
///
/// # Panics
/// Panics if none of `names` are found in `keys`.
pub fn key_index_first(keys: &[Value], names: &[&str]) -> usize {
    names
        .iter()
        .find_map(|name| keys.iter().position(|key| key.as_str() == Some(*name)))
        .unwrap_or_else(|| panic!("missing keys {:?}", names))
}

/// Hot runtime rows store per-skill hosts under `host_platforms` or legacy `source_position`.
pub fn runtime_host_platforms_index(keys: &[Value]) -> usize {
    key_index_first(keys, &["host_platforms", "source_position"])
}

/// Runtime rows store the description under `description` or legacy `summary`.
pub fn runtime_description_index(keys: &[Value]) -> usize {
    key_index_first(keys, &["description", "summary"])
}

/// Recursively walk a directory tree, calling `visitor` for each file found.
///
/// Skips `.git`, `target`, `node_modules`, `.venv`, `venv`, `__pycache__`,
/// and `generated-artifacts-drift-check` directories.
pub fn collect_files(root: &Path, visitor: &mut dyn FnMut(&Path)) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let directory_name = path.file_name().and_then(|name| name.to_str());
            if matches!(
                directory_name,
                Some(
                    ".git"
                        | "target"
                        | "node_modules"
                        | ".venv"
                        | "venv"
                        | "__pycache__"
                        | "generated-artifacts-drift-check"
                )
            ) {
                continue;
            }
            collect_files(&path, visitor);
        } else if path.is_file() {
            visitor(&path);
        }
    }
}

/// Collect all files under `root` with a specific file extension (e.g. `"rs"`, `"md"`, `"py"`).
pub fn collect_files_with_extension(root: &Path, extension: &str) -> Vec<PathBuf> {
    let mut results = Vec::new();
    collect_files(root, &mut |path| {
        if path.extension().and_then(|ext| ext.to_str()) == Some(extension) {
            results.push(path.to_path_buf());
        }
    });
    results
}

/// Concatenate all markdown file content under the given root directories.
pub fn markdown_text_under(roots: &[PathBuf]) -> String {
    let mut chunks = Vec::new();
    for root in roots {
        collect_files(root, &mut |path| {
            if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
                chunks.push(read_text(path));
            }
        });
    }
    chunks.join("\n")
}

/// Check if a path is an allowed Python control plane path (test fixture exemption).
///
/// Allows the Cursor hook-test helpers that must use Python for Codex CLI configuration.
pub fn allowed_python_control_plane_path(path: &Path) -> bool {
    let text = path.to_string_lossy();
    text == ".cursor/hook-tests/test_install_codex_cli_hooks.py"
        || text.starts_with(".cursor/hook-tests/tmp_")
}
