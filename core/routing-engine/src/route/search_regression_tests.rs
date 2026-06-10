//! Search correctness regression fixtures (stable ordering vs baseline slugs).

use super::{
    filter_record_indices_for_host, load_records_from_manifest, search_skills, search_skills_subset,
};
use std::collections::HashSet;
use std::path::PathBuf;

fn manifest_records() -> Vec<super::SkillRecord> {
    let manifest_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_MANIFEST.json");
    load_records_from_manifest(&manifest_path).expect("load manifest records")
}

#[test]
fn search_subset_matches_full_scan_for_host_filter() {
    let records = manifest_records();
    let query = "DESIGN.md 设计规范 token";
    let host_id = "cursor";
    let indices = filter_record_indices_for_host(&records, Some(host_id)).expect("indices");
    let subset = search_skills_subset(&records, Some(&indices), query, 5);
    let filtered: Vec<_> = indices.iter().map(|&idx| records[idx].clone()).collect();
    let full = search_skills(&filtered, query, 5);
    assert_eq!(
        subset.iter().map(|row| row.slug.as_str()).collect::<Vec<_>>(),
        full.iter().map(|row| row.slug.as_str()).collect::<Vec<_>>(),
        "index-based search must match cloned host-filtered search"
    );
}

#[test]
fn search_baseline_slugs_design_md_and_plugin_creator() {
    let records = manifest_records();
    let design = search_skills(&records, "DESIGN.md 设计规范 token", 5);
    assert_eq!(design.first().map(|row| row.slug.as_str()), Some("design-md"));
    assert!(!design.iter().any(|row| row.slug == "css-pro"));

    let indices = filter_record_indices_for_host(&records, Some("cursor")).expect("cursor indices");
    let framework = search_skills_subset(
        &records,
        Some(&indices),
        "skill framework developer routing",
        20,
    );
    let slugs: HashSet<_> = framework.iter().map(|row| row.slug.as_str()).collect();
    assert!(
        slugs.contains("skill-framework-developer"),
        "manifest baseline search must still surface skill-framework-developer; got {slugs:?}"
    );
}
