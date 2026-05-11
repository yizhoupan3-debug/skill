#[cfg(test)]
mod route_metadata_tests {
    use crate::route::aliases::{
        framework_alias_entrypoints_from_hints, has_explicit_framework_alias_call,
        has_literal_framework_alias_call,
    };
    use crate::route::records::{
        load_records, load_records_cached_for_stdio, load_records_cached_for_stdio_resolved,
        load_records_from_manifest, load_records_from_runtime,
    };
    use crate::route::routing::route_task;
    use crate::route::signals::has_paper_review_judgment_context;
    use crate::route::text::normalize_text;
    use crate::route::types::{RawSkillRecord, SkillRecord};
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn temp_route_path(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("router-rs-route-{label}-{nonce}.json"))
    }

    #[test]
    fn runtime_sidecar_applies_declarative_negative_triggers() {
        let path = temp_route_path("runtime-records");
        let metadata_path = path
            .parent()
            .expect("runtime parent")
            .join("SKILL_ROUTING_METADATA.json");
        fs::write(
            &path,
            serde_json::to_string(&json!({
                "version": 3,
                "keys": ["slug", "layer", "owner", "gate", "session_start", "summary", "trigger_hints", "priority", "skill_path"],
                "skills": [[
                    "sample-skill",
                    "L1",
                    "owner",
                    "none",
                    "n/a",
                    "Sample skill",
                    ["sample"],
                    "P1",
                    "skills/sample-skill/SKILL.md"
                ]]
            }))
            .expect("serialize runtime"),
        )
        .expect("write runtime");
        fs::write(
            &metadata_path,
            serde_json::to_string(&json!({
                "skills": {
                    "sample-skill": {
                        "negative_triggers": ["blocked route"]
                    }
                }
            }))
            .expect("serialize metadata"),
        )
        .expect("write metadata");

        let records = load_records_from_runtime(&path).expect("load runtime");
        let record = records
            .iter()
            .find(|record| record.slug == "sample-skill")
            .expect("sample record");
        assert!(record.do_not_use_tokens.contains("blocked"));
        assert!(record.do_not_use_tokens.contains("route"));

        fs::remove_file(path).expect("cleanup runtime");
        fs::remove_file(metadata_path).expect("cleanup metadata");
    }

    #[test]
    fn manifest_sidecar_applies_declarative_negative_triggers_to_runtime_records() {
        let root = std::env::temp_dir().join(format!(
            "router-rs-route-meta-sidecar-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create temp route root");
        let runtime_path = root.join("SKILL_ROUTING_RUNTIME.json");
        let manifest_path = root.join("SKILL_MANIFEST.json");
        let metadata_path = root.join("SKILL_ROUTING_METADATA.json");
        let runtime_payload = json!({
            "version": 3,
            "keys": ["slug", "layer", "owner", "gate", "session_start", "summary", "trigger_hints", "priority", "skill_path"],
            "skills": [[
                "sample-skill",
                "L1",
                "owner",
                "none",
                "n/a",
                "Sample skill",
                ["sample"],
                "P1",
                "skills/sample-skill/SKILL.md"
            ]]
        });
        let manifest_payload = json!({
            "keys": ["slug", "layer", "owner", "gate", "priority", "description", "session_start", "trigger_hints", "source", "source_position", "skill_path"],
            "skills": [[
                "sample-skill",
                "L1",
                "owner",
                "none",
                "P1",
                "Sample skill",
                "n/a",
                ["sample"],
                "project",
                3,
                "skills/sample-skill/SKILL.md"
            ]]
        });
        let metadata_payload = json!({
            "schema_version": "skill-routing-metadata-v1",
            "skills": {
                "sample-skill": {
                    "negative_triggers": ["sidecar blocked"]
                }
            }
        });
        fs::write(
            &runtime_path,
            serde_json::to_string(&runtime_payload).unwrap(),
        )
        .expect("write runtime");
        fs::write(
            &manifest_path,
            serde_json::to_string(&manifest_payload).unwrap(),
        )
        .expect("write manifest");
        fs::write(
            &metadata_path,
            serde_json::to_string(&metadata_payload).unwrap(),
        )
        .expect("write metadata");

        let records =
            load_records(Some(&runtime_path), Some(&manifest_path)).expect("load route records");
        let record = records
            .iter()
            .find(|record| record.slug == "sample-skill")
            .expect("sample record");
        assert!(record.do_not_use_tokens.contains("sidecar"));
        assert!(record.do_not_use_tokens.contains("blocked"));

        fs::remove_dir_all(root).expect("cleanup route root");
    }

    #[test]
    fn stdio_route_cache_refreshes_when_metadata_sidecar_changes() {
        let root = std::env::temp_dir().join(format!(
            "router-rs-route-meta-cache-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create temp route root");
        let runtime_path = root.join("SKILL_ROUTING_RUNTIME.json");
        let manifest_path = root.join("SKILL_MANIFEST.json");
        let metadata_path = root.join("SKILL_ROUTING_METADATA.json");
        fs::write(
            &runtime_path,
            serde_json::to_string(&json!({
                "version": 3,
                "keys": ["slug", "layer", "owner", "gate", "session_start", "summary", "trigger_hints", "priority", "skill_path"],
                "skills": [[
                    "sample-skill",
                    "L1",
                    "owner",
                    "none",
                    "n/a",
                    "Sample skill",
                    ["sample"],
                    "P1",
                    "skills/sample-skill/SKILL.md"
                ]]
            }))
            .unwrap(),
        )
        .expect("write runtime");
        fs::write(
            &manifest_path,
            serde_json::to_string(&json!({
                "keys": ["slug", "layer", "owner", "gate", "priority", "description", "session_start", "trigger_hints", "source", "source_position", "skill_path"],
                "skills": [[
                    "sample-skill",
                    "L1",
                    "owner",
                    "none",
                    "P1",
                    "Sample skill",
                    "n/a",
                    ["sample"],
                    "project",
                    3,
                    "skills/sample-skill/SKILL.md"
                ]]
            }))
            .unwrap(),
        )
        .expect("write manifest");
        fs::write(
            &metadata_path,
            serde_json::to_string(&json!({
                "schema_version": "skill-routing-metadata-v1",
                "skills": {"sample-skill": {"negative_triggers": ["first blocked"]}}
            }))
            .unwrap(),
        )
        .expect("write metadata");

        let first =
            load_records_cached_for_stdio_resolved(Some(&runtime_path), Some(&manifest_path))
                .expect("first cached load");
        assert!(first[0].do_not_use_tokens.contains("first"));
        assert!(!first[0].do_not_use_tokens.contains("second"));

        thread::sleep(Duration::from_millis(25));
        fs::write(
            &metadata_path,
            serde_json::to_string(&json!({
                "schema_version": "skill-routing-metadata-v1",
                "skills": {"sample-skill": {"negative_triggers": ["second blocked"]}}
            }))
            .unwrap(),
        )
        .expect("update metadata");
        let second =
            load_records_cached_for_stdio_resolved(Some(&runtime_path), Some(&manifest_path))
                .expect("second cached load");
        assert!(!second[0].do_not_use_tokens.contains("first"));
        assert!(second[0].do_not_use_tokens.contains("second"));

        fs::remove_dir_all(root).expect("cleanup route root");
    }

    #[test]
    fn framework_alias_entrypoints_include_mode_variants() {
        let entrypoints = framework_alias_entrypoints_from_hints(
            "autopilot",
            "L0",
            &[
                "/autopilot".to_string(),
                "/autopilot-quick".to_string(),
                "/autopilot-deep".to_string(),
                "/autopilot deep".to_string(),
                "autopilot".to_string(),
            ],
        );
        for expected in [
            "/autopilot",
            "/autopilot-quick",
            "/autopilot-deep",
            "/autopilot deep",
            "autopilot",
        ] {
            assert!(
                entrypoints.contains(&expected.to_string()),
                "missing entrypoint {expected}"
            );
        }
    }

    #[test]
    fn route_task_matches_autopilot_quick_and_deep_entrypoints() {
        let records = vec![SkillRecord::from_raw(RawSkillRecord {
            slug: "autopilot".to_string(),
            skill_path: Some("skills/autopilot/SKILL.md".to_string()),
            layer: "L0".to_string(),
            owner: "owner".to_string(),
            gate: "none".to_string(),
            priority: "P1".to_string(),
            session_start: "required".to_string(),
            summary: "Autopilot owner".to_string(),
            short_description: String::new(),
            when_to_use: String::new(),
            do_not_use: String::new(),
            tags: Vec::new(),
            trigger_hints: vec![
                "/autopilot".to_string(),
                "/autopilot-quick".to_string(),
                "/autopilot-deep".to_string(),
            ],
        })];
        for query in ["/autopilot", "/autopilot-quick", "/autopilot-deep"] {
            let decision =
                route_task(&records, query, "session", false, false).expect("route decision");
            assert_eq!(decision.selected_skill, "autopilot");
            assert_eq!(decision.layer, "L0");
        }
    }

    #[test]
    fn load_records_prefers_default_runtime_even_with_explicit_manifest() {
        let root = temp_route_path("runtime-first-manifest");
        let skills_root = root.join("skills");
        fs::create_dir_all(&skills_root).expect("create skills root");
        let manifest_path = skills_root.join("SKILL_MANIFEST.json");
        fs::write(
            &manifest_path,
            serde_json::to_string(&json!({
                "keys": ["slug", "layer", "owner", "gate", "priority", "description", "session_start", "trigger_hints", "source", "source_position", "skill_path"],
                "skills": [[
                    "manifest-owner",
                    "L1",
                    "manifest-owner",
                    "none",
                    "P1",
                    "Manifest owner",
                    "n/a",
                    ["manifest owner"],
                    "project",
                    1,
                    "skills/manifest-owner/SKILL.md"
                ]]
            }))
            .expect("serialize manifest"),
        )
        .expect("write manifest");

        let loaded = load_records(None, Some(&manifest_path)).expect("load records");
        assert!(
            loaded.iter().any(|record| record.slug == "autopilot"),
            "default runtime hot index should be preferred when available"
        );
        assert!(
            loaded.iter().all(|record| record.slug != "manifest-owner"),
            "explicit manifest should not bypass runtime-first loading"
        );

        fs::remove_dir_all(&root).expect("cleanup route root");
    }

    #[test]
    fn records_cache_evicts_oldest_admission_when_over_capacity() {
        let root = std::env::temp_dir().join(format!(
            "router-rs-records-cache-evict-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create temp cache-evict root");
        let mut pairs: Vec<(PathBuf, PathBuf)> = Vec::new();
        for idx in 0..5usize {
            let runtime_path = root.join(format!("SKILL_ROUTING_RUNTIME_{idx}.json"));
            let manifest_path = root.join(format!("SKILL_MANIFEST_{idx}.json"));
            fs::write(
                &runtime_path,
                serde_json::to_string(&json!({
                    "keys": ["slug", "layer", "owner", "gate", "summary", "trigger_hints", "priority", "session_start"],
                    "skills": [[format!("slug{idx}"), "L2", "primary", "none", format!("summary-{idx}"), ["trigger"], "P1", "always"]]
                }))
                .expect("serialize runtime fixture"),
            )
            .expect("write runtime fixture");
            fs::write(
                &manifest_path,
                serde_json::to_string(&json!({
                    "keys": ["slug", "description", "layer", "owner", "gate", "trigger_hints", "priority", "session_start"],
                    "skills": [[format!("slug{idx}"), format!("manifest-{idx}"), "L2", "primary", "none", ["trigger"], "P1", "always"]]
                }))
                .expect("serialize manifest fixture"),
            )
            .expect("write manifest fixture");
            pairs.push((runtime_path, manifest_path));
        }

        let first = load_records_cached_for_stdio(Some(&pairs[0].0), Some(&pairs[0].1))
            .expect("load pair 0");
        for (runtime_path, manifest_path) in pairs.iter().skip(1) {
            load_records_cached_for_stdio(Some(runtime_path), Some(manifest_path))
                .expect("load subsequent pair");
        }
        let replay = load_records_cached_for_stdio(Some(&pairs[0].0), Some(&pairs[0].1))
            .expect("reload pair 0 after fifo eviction");
        assert!(
            !Arc::ptr_eq(&first, &replay),
            "test builds cap the cache at RECORDS_CACHE_MAX_KEYS; oldest key must reload"
        );

        fs::remove_dir_all(&root).expect("cleanup cache-evict root");
    }

    #[test]
    fn paper_stack_plain_slug_counts_as_explicit_framework_alias_when_hint_has_sigil() {
        let record = SkillRecord::from_raw(RawSkillRecord {
            slug: "paper-reviewer".to_string(),
            skill_path: Some("skills/paper-reviewer/SKILL.md".to_string()),
            layer: "L2".to_string(),
            owner: "owner".to_string(),
            gate: "none".to_string(),
            priority: "P2".to_string(),
            session_start: "preferred".to_string(),
            summary: "paper reviewer lane".to_string(),
            short_description: String::new(),
            when_to_use: String::new(),
            do_not_use: String::new(),
            tags: Vec::new(),
            trigger_hints: vec!["$paper-reviewer".to_string(), "/paper-reviewer".to_string()],
        });
        assert!(!record.framework_alias_entrypoints.is_empty());
        let query = normalize_text("用 paper-reviewer 逻辑模式看一下 claim/evidence");
        assert!(
            has_literal_framework_alias_call(&query, &record),
            "expected plain slug token to satisfy literal alias call"
        );
        let tokens = vec![
            "用".into(),
            "paper-reviewer".into(),
            "逻辑模式看一下".into(),
            "claim/evidence".into(),
        ];
        assert!(
            has_explicit_framework_alias_call(&query, &tokens, &record),
            "expected plain slug parity for scoring gate"
        );
        let tight = normalize_text("用paper-reviewer审一下 claim");
        assert!(
            has_literal_framework_alias_call(&tight, &record),
            "expected slug token detection without spaces around CJK adjacency"
        );
    }

    #[test]
    fn manuscript_critique_only_wording_triggers_paper_review_judgment_heuristic() {
        let qt = normalize_text("只想要科学性批评不要改稿 manuscript");
        let tokens: Vec<String> = Vec::new();
        assert!(has_paper_review_judgment_context(&qt, &tokens));
    }

    #[test]
    fn manifest_paper_reviewer_row_accepts_plain_slug_literal() {
        let manifest_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../skills/SKILL_MANIFEST.json");
        let records = load_records_from_manifest(&manifest_path).expect("manifest load");
        let rec = records
            .iter()
            .find(|r| r.slug == "paper-reviewer")
            .expect("paper-reviewer row");
        assert!(
            !rec.framework_alias_entrypoints.is_empty(),
            "manifest row should carry framework alias entrypoints"
        );
        let q = normalize_text("用 paper-reviewer 逻辑模式审一下 claim evidence");
        assert!(
            has_literal_framework_alias_call(&q, rec),
            "{:?}",
            rec.framework_alias_entrypoints
        );
    }
}
