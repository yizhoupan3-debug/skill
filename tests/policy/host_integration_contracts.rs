use crate::common::{project_root, read_json, read_text, router_rs_json};

#[test]
fn autoresearch_skill_is_active_routing_entrypoint() {
    assert!(
        project_root()
            .join("core/research-harness/src/bin/autoresearch.rs")
            .exists()
    );
    assert!(project_root().join("skills/autoresearch/SKILL.md").exists());
}

#[test]
fn installed_project_hooks_are_router_rs_managed() {
    assert!(project_root().join(".codex/hooks.json").exists());
    assert!(!project_root().join(".codex/hooks").exists());
    let config = read_text(&project_root().join(".codex/config.toml"));
    // hooks are configured via hooks.json, not config.toml in v6
    assert!(!config.contains("codex_hooks"));
    let hooks = read_json(&project_root().join(".codex/hooks.json"));
    let hook_events = hooks["hooks"].as_object().unwrap();
    for event in [
        "SessionStart",
        "PreToolUse",
        "UserPromptSubmit",
        "PostToolUse",
        "Stop",
    ] {
        assert!(
            hook_events.contains_key(event),
            "missing Codex hook event: {event}"
        );
    }
    let hook_text = hooks.to_string();
    assert!(hook_text.contains("router-rs"));
    assert!(hook_text.contains("codex-router-rs-hook.sh"));
    assert!(!hook_text.contains("scripts/codex_hook_entrypoint.sh"));
    assert!(!hook_text.contains("sessionEnd"));
    let manifest = read_json(&project_root().join(".codex/host_entrypoints_sync_manifest.json"));
    assert!(manifest.to_string().contains(".codex/hooks.json"));
}

#[test]
fn cursor_hooks_json_matches_workspace_template_seven_event_set() {
    let contract = router_rs_json(&["schema-drift", "contract"]);
    let required: Vec<String> = contract["cursor_hooks_required"]
        .as_array()
        .expect("cursor_hooks_required")
        .iter()
        .map(|v| v.as_str().expect("event name").to_string())
        .collect();
    let forbidden: Vec<String> = contract["cursor_hooks_forbidden"]
        .as_array()
        .expect("cursor_hooks_forbidden")
        .iter()
        .map(|v| v.as_str().expect("event name").to_string())
        .collect();

    let hooks = read_json(&project_root().join(".cursor/hooks.json"));
    let template =
        read_json(&project_root().join("configs/framework/cursor-hooks.workspace-template.json"));
    for doc in [(&hooks, "hooks.json"), (&template, "workspace-template")] {
        let events = doc.0["hooks"].as_object().expect("hooks object");
        for ev in &required {
            let key = ev.clone();
            assert!(events.contains_key(&key), "{} missing event {}", doc.1, ev);
            let cmd = events[&key][0]["command"].as_str().unwrap_or("");
            assert!(
                cmd.contains("cursor-router-rs-hook.sh"),
                "{} {} must use cursor-router-rs-hook.sh",
                doc.1,
                ev
            );
        }
        for ev in &forbidden {
            let key = ev.clone();
            assert!(
                !events.contains_key(&key),
                "{} must not register removed event {}",
                doc.1,
                ev
            );
        }
    }
    let h = hooks["hooks"].as_object().unwrap();
    let t = template["hooks"].as_object().unwrap();
    assert_eq!(h.keys().collect::<Vec<_>>(), t.keys().collect::<Vec<_>>());
    for ev in &required {
        let key = ev.clone();
        assert_eq!(
            h[&key][0]["timeout"], t[&key][0]["timeout"],
            "timeout mismatch on {ev}"
        );
        assert_eq!(
            h[&key][0]["command"], t[&key][0]["command"],
            "command mismatch on {ev}"
        );
    }
    assert_eq!(h.get("postToolUse").unwrap()[0]["timeout"], 20);
}

#[test]
fn install_skills_uses_rust_only_entrypoints() {
    assert!(!project_root().join("scripts/install_skills.sh").exists());
    let mod_source =
        read_text(&project_root().join("core/runtime-core/src/host_integration/mod.rs"));
    let roots_source =
        read_text(&project_root().join("core/runtime-core/src/host_integration/roots.rs"));
    let projection_source = read_text(
        &project_root().join("core/runtime-core/src/host_integration/projection/mod.rs"),
    );
    assert!(
        mod_source.contains("InstallSkills") || roots_source.contains("InstallSkills"),
        "missing marker: InstallSkills"
    );
    assert!(
        mod_source.contains("InstallNativeIntegration")
            || roots_source.contains("InstallNativeIntegration"),
        "missing marker: InstallNativeIntegration"
    );
    assert!(
        projection_source.contains("validate_default_bootstrap")
            || roots_source.contains("validate_default_bootstrap"),
        "missing marker: validate_default_bootstrap"
    );
}

#[test]
fn sync_skills_uses_router_rs_directly() {
    assert!(!project_root().join("scripts/sync_skills.py").exists());
    let sync_source =
        read_text(&project_root().join("core/host-projection/src/host_entrypoint_sync.rs"));
    assert!(sync_source.contains("sync_host_entrypoints"));
    assert!(sync_source.contains("HostEntrypointPayloadProvider"));
    for forbidden in ["crate::codex_hooks", "build_codex_"] {
        assert!(
            !sync_source.contains(forbidden),
            "host_entrypoint_sync must stay provider-based and host-neutral: {forbidden}"
        );
    }
    let codex_source =
        read_text(&project_root().join("core/host-projection/src/hosts/codex_hooks/mod.rs"));
    assert!(codex_source.contains("codex_host_entrypoint_provider"));
    // HostEntrypointPayloadProvider lives in install.rs after codex_hooks split
    let codex_install =
        read_text(&project_root().join("core/host-projection/src/hosts/codex_hooks/install.rs"));
    assert!(codex_install.contains("HostEntrypointPayloadProvider"));
}

#[test]
fn prompt_policy_is_rust_owned() {
    let root = project_root();
    let mod_rs = read_text(&root.join("core/runtime-core/src/framework_runtime/mod.rs"));
    let compression =
        read_text(&root.join("core/runtime-core/src/framework_runtime/prompt_compression.rs"));
    assert!(mod_rs.contains("build_framework_prompt_compression_envelope"));
    assert!(compression.contains("prompt_policy_owner"));
}
