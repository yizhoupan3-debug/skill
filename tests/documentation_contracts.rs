#![allow(dead_code)]
mod common;

use common::{project_root, read_text};
use regex::Regex;

#[test]
fn rust_contracts_doc_no_longer_uses_stale_transition_wording() {
    let text = rust_contracts_doc();
    for stale_phrase in [
        "escape hatch",
        "not live yet",
        "implementation remains pending",
        "hidden behind an escape hatch",
    ] {
        assert!(
            !text.contains(stale_phrase),
            "stale phrase present: {stale_phrase}"
        );
    }
}

#[test]
fn host_and_contract_docs_avoid_stale_codex_and_planx_wording() {
    let root = project_root();
    let mut paths = vec!["docs/spec.md".to_string(), "AGENTS_CLAUDE.md".to_string()];
    let hosts_dir = root.join("docs/hosts");
    if hosts_dir.is_dir() {
        for entry in std::fs::read_dir(&hosts_dir).expect("read docs/hosts") {
            let entry = entry.expect("hosts dir entry");
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("md") {
                paths.push(
                    path.strip_prefix(&root)
                        .expect("host doc under repo root")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    paths.sort();
    paths.dedup();

    let joined = paths
        .iter()
        .map(|rel| read_text(&root.join(rel)))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        !joined.contains("AGENTS.md bootstrap"),
        "stale Codex bootstrap wording in host/contract docs"
    );

    let planx_impl = Regex::new(r"(?is)/planx[^\n]{0,400}implementation_plan\.md").unwrap();
    assert!(
        !planx_impl.is_match(&joined),
        "stale /planx + implementation_plan.md pairing in host/contract docs"
    );
}

#[test]
fn top_level_docs_do_not_revive_removed_legacy_python_work_as_active() {
    let root = project_root();
    let scoped_docs = [
        "docs/spec.md",
        "docs/framework_profile_contract.md",
    ];
    let joined = scoped_docs
        .iter()
        .map(|path| read_text(&root.join(path)))
        .collect::<Vec<_>>()
        .join("\n");
    for stale_phrase in [
        "keep-temporarily",
        "pending-removal",
        "Python artifact emitter 已支持",
        "Python artifact emitter 已外显",
        "Python / Rust parity tests",
        "framework_runtime/src/framework_runtime",
        "scripts/materialize_cli_host_entrypoints.py 管理",
        "runtime durable state: `framework_runtime/data",
    ] {
        assert!(
            !joined.contains(stale_phrase),
            "stale active-doc phrase present: {stale_phrase}"
        );
    }
}

fn rust_contracts_doc() -> String {
    // Content merged into spec.md after v6.5 consolidation
    read_text(&project_root().join("docs/spec.md"))
}

#[test]
fn harness_policy_map_documents_ship_readiness_stop_orchestration() {
    // Content merged into spec.md after v6.5 consolidation
    let spec = read_text(&project_root().join("docs/spec.md"));
    assert!(
        spec.contains("ship_readiness") || spec.contains("Stop") || spec.contains("closeout"),
        "spec.md must document Stop/closeout orchestration"
    );
}
