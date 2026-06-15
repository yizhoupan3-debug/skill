#![allow(dead_code)]
//! Cursor alwaysApply rules must keep markdown links to policy canonicals
//! (drift guard for harness subtraction).
//!
//! Fenced-code stripping toggles only on lines that match a **CommonMark subset**:
//! at most **three** ASCII spaces, then `` ``` `` (opening or closing). More-indented
//! `` ``` `` lines are treated as body text, not fences. `~~~` fences are unsupported.
//!
//! If an opening `` ``` `` is never closed before EOF, stripping is **abandoned** and the
//! original source is returned so markdown links after a broken fence are not dropped
//! (fail-closed against false negatives).

use std::fs;
use std::path::{Path, PathBuf};

mod common;

use common::project_root;

/// Strip UTF-8 BOM and leading whitespace so `---` frontmatter is still discoverable.
fn normalize_mdc_source(text: &str) -> &str {
    text.strip_prefix('\u{FEFF}').unwrap_or(text).trim_start()
}

/// Conservative YAML-like scalar comment: first ASCII ` #` starts a line comment.
fn strip_inline_yaml_hash_comment(s: &str) -> &str {
    let s = s.trim();
    if let Some(i) = s.find(" #") {
        return s[..i].trim_end();
    }
    s
}

/// `alwaysApply: true` / `True` / quoted `true` (ASCII), optional `# …` tail; does not accept yes/on/1.
fn always_apply_value_is_true(raw_val: &str) -> bool {
    let mut v = raw_val.trim();
    if v.len() >= 2
        && ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')))
    {
        v = &v[1..v.len() - 1];
    }
    let v = strip_inline_yaml_hash_comment(v);
    v.eq_ignore_ascii_case("true")
}

fn line_declares_always_apply_true(line: &str) -> bool {
    let t = line.trim();
    let Some((key, val)) = t.split_once(':') else {
        return false;
    };
    key.trim() == "alwaysApply" && always_apply_value_is_true(val)
}

/// First YAML frontmatter block: opening `---` line then lines until a closing line whose
/// `trim()` is exactly `---` (allows trailing spaces on that line, e.g. `--- \n`).
fn first_frontmatter_block(text: &str) -> Option<&str> {
    let rest = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))?;
    let mut end_byte = 0usize;
    for raw_line in rest.split_inclusive('\n') {
        let line_body = raw_line.strip_suffix('\n').unwrap_or(raw_line);
        let line_core = line_body.trim_end_matches('\r');
        if line_core.trim() == "---" {
            return rest
                .get(..end_byte)
                .map(|s| s.trim_end_matches(['\r', '\n']));
        }
        end_byte += raw_line.len();
    }
    None
}

fn frontmatter_declares_always_apply_true(fm: &str) -> bool {
    fm.lines().any(line_declares_always_apply_true)
}

/// Opening `---` without a valid closing `---` line, but `alwaysApply: true` appears anywhere
/// in the tail → treat as malformed frontmatter (full tail scan; no line/byte window cap).
fn panic_if_unclosed_opening_frontmatter_declares_always_apply(path: &Path, norm: &str) {
    if first_frontmatter_block(norm).is_some() {
        return;
    }
    if !(norm.starts_with("---\n") || norm.starts_with("---\r\n")) {
        return;
    }
    let rest = norm
        .strip_prefix("---\n")
        .or_else(|| norm.strip_prefix("---\r\n"))
        .expect("starts with --- newline");
    if rest.lines().any(line_declares_always_apply_true) {
        panic!(
            "{}: malformed YAML frontmatter: opening --- without a closing --- line whose trim() is exactly '---', but found `alwaysApply: true` after that opening delimiter (full tail scan); fix the file or restore a valid closing ---",
            path.display()
        );
    }
}

fn line_is_backtick_fence_boundary(line: &str) -> bool {
    let b = line.as_bytes();
    let mut i = 0usize;
    while i < b.len() && i < 3 && b[i] == b' ' {
        i += 1;
    }
    line.get(i..).is_some_and(|tail| tail.starts_with("```"))
}

/// Remove ``` fenced blocks (≤3 leading spaces before ```); drops fence lines and body lines
/// inside. If EOF is reached while still inside a fence, returns `text` unchanged.
fn strip_triple_backtick_fenced_blocks(text: &str) -> String {
    let original = text;
    let mut out = String::with_capacity(text.len());
    let mut in_fence = false;
    for line in text.lines() {
        if line_is_backtick_fence_boundary(line) {
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

fn url_looks_like_relative_pointer(url: &str) -> bool {
    let u = url.trim();
    u.contains("../") || u.contains("./") || u.starts_with("docs/") || u.contains("/docs/")
}

fn markdown_link_url_contains(body: &str, needle: &str) -> bool {
    let cleaned = strip_triple_backtick_fenced_blocks(body);
    for (idx, _) in cleaned.match_indices("](") {
        let rest = &cleaned[idx + 2..];
        let Some(end) = rest.find(')') else {
            continue;
        };
        let url = &rest[..end];
        if url.contains(needle) && url_looks_like_relative_pointer(url) {
            return true;
        }
    }
    false
}

fn assert_canonical_markdown_links(path: &Path, text: &str) {
    let needles = [("AGENTS.md", "AGENTS.md"), ("spec.md", "spec.md")];
    for (label, needle) in needles {
        assert!(
            markdown_link_url_contains(text, needle),
            "{}: missing markdown link URL containing {} in `](url)` where url uses a relative pointer shape (`../`, `./`, `docs/`, or a `/docs/` path segment). Put at least one such link in unfenced body text; do not rely on ```-fenced blocks as the only canonical pointer.",
            path.display(),
            label
        );
    }
}

fn always_apply_mdc_paths(rules_dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(rules_dir) else {
        return out;
    };
    for ent in entries.flatten() {
        let path = ent.path();
        if path.extension().and_then(|s| s.to_str()) != Some("mdc") {
            continue;
        }
        let text =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let norm = normalize_mdc_source(&text);
        match first_frontmatter_block(norm) {
            Some(fm) => {
                if frontmatter_declares_always_apply_true(fm) {
                    out.push(path);
                }
            }
            None => {
                panic_if_unclosed_opening_frontmatter_declares_always_apply(&path, norm);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn line_declares_always_apply_accepts_inline_hash_comment() {
    assert!(line_declares_always_apply_true(
        "alwaysApply: true # cursor"
    ));
}

#[test]
fn line_declares_always_apply_accepts_uppercase_true_with_comment() {
    assert!(line_declares_always_apply_true("alwaysApply: True # x"));
}

#[test]
fn markdown_link_ignores_fenced_code_only_urls() {
    let only_fence = "```\n](../../AGENTS.md)\n```\n";
    assert!(!markdown_link_url_contains(only_fence, "AGENTS.md"));
    let body = "```\n](../../AGENTS.md)\n```\n\nx [a](../../AGENTS.md)\n";
    assert!(markdown_link_url_contains(body, "AGENTS.md"));
}

#[test]
fn markdown_link_requires_relative_pointer_shape() {
    let body = "x [a](AGENTS.md)";
    assert!(!markdown_link_url_contains(body, "AGENTS.md"));
}

#[test]
fn strip_fence_eof_inside_fence_returns_original_so_body_links_remain_visible() {
    let body = "# t\n```\ncode\n\n[a](../../AGENTS.md)\n";
    assert!(markdown_link_url_contains(body, "AGENTS.md"));
}

#[test]
fn unclosed_frontmatter_always_apply_after_many_lines_still_panics() {
    let padding = "\n".repeat(100);
    let norm = format!("---\n{padding}alwaysApply: true\n");
    let path = Path::new("__policy_fixture__.mdc");
    let caught = std::panic::catch_unwind(|| {
        panic_if_unclosed_opening_frontmatter_declares_always_apply(path, &norm);
    });
    assert!(
        caught.is_err(),
        "expected panic for alwaysApply beyond old scan window"
    );
}

#[test]
fn first_frontmatter_accepts_closing_delim_with_trailing_spaces() {
    let text = "---\nalwaysApply: false\n---  \n# body\n";
    let fm = first_frontmatter_block(text).expect("fm");
    assert!(fm.contains("alwaysApply: false"));
}

#[test]
fn cursor_always_apply_rules_link_agents_and_harness_architecture() {
    let root = project_root();
    let rules_dir = root.join(".cursor/rules");
    assert!(
        rules_dir.is_dir(),
        "expected {} to exist",
        rules_dir.display()
    );

    let paths = always_apply_mdc_paths(&rules_dir);
    assert!(
        !paths.is_empty(),
        "expected at least one alwaysApply: true .mdc under {}",
        rules_dir.display()
    );

    for path in &paths {
        let text =
            fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        assert_canonical_markdown_links(path, &text);
    }
}
