//! 保证主要叙事面（docs/、skills/、根契约 Markdown）在 git 跟踪下可读且为有效 UTF-8，
//! 与 `/update` 的「全文档纳管」口径一致（不评价正文正确性；正确性由其它契约测试驱动修改）。

mod common;

use common::project_root;
use std::process::Command;

#[test]
fn git_tracked_markdown_doc_and_skill_surfaces_are_valid_utf8() {
    let root = project_root();
    let output = Command::new("git")
        .args(["ls-files", "-z", "--", "*.md"])
        .current_dir(&root)
        .output()
        .expect("git ls-files: requires git and a checkout");
    assert!(
        output.status.success(),
        "git ls-files failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mut scanned = 0usize;
    let mut missing = Vec::new();
    for chunk in output.stdout.split(|b| *b == 0) {
        if chunk.is_empty() {
            continue;
        }
        let rel = std::str::from_utf8(chunk).expect("git path must be utf-8");
        if !(rel.starts_with("docs/")
            || rel.starts_with("skills/")
            || matches!(rel, "AGENTS.md" | "README.md" | "RTK.md"))
        {
            continue;
        }
        let path = root.join(rel);
        if !path.is_file() {
            missing.push(rel.to_string());
            continue;
        }
        scanned += 1;
        std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("{}: {err}", path.display()));
    }

    assert!(
        missing.is_empty(),
        "git index lists markdown under docs/skills/roots but file missing on disk \
         (run `git status` / `git add` / `git rm` to reconcile before /update): {missing:?}"
    );

    assert!(
        scanned >= 80,
        "expected a large tracked markdown surface under docs/ + skills/ + roots; got {scanned}"
    );
}
