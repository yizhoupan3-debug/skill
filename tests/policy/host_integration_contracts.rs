use crate::common::{project_root, read_json, read_text};

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
    // .codex/ 目录在未运行 install-skills 时不存在；框架脚本始终存在
    assert!(project_root().join("configs/framework/hook.sh").exists());
    assert!(!project_root().join(".codex/hooks").exists());
}

#[test]
fn cursor_hooks_json_matches_workspace_template_seven_event_set() {
    // 只验证 workspace template（始终存在），跳过 .cursor/hooks.json（仅安装后存在）
    let template =
        read_json(&project_root().join("configs/framework/cursor-hooks.workspace-template.json"));
    let events = template["hooks"].as_object().expect("hooks object");
    let expected_events = [
        "beforeSubmitPrompt",
        "stop",
        "sessionStart",
        "sessionEnd",
        "postToolUse",
        "subagentStart",
        "subagentStop",
    ];
    for ev in &expected_events {
        assert!(
            events.contains_key(*ev),
            "workspace-template missing event {ev}"
        );
        let cmd = events[*ev][0]["command"].as_str().unwrap_or("");
        assert!(
            cmd.contains("cursor-router-rs-hook.sh"),
            "workspace-template {ev} must use cursor-router-rs-hook.sh"
        );
    }
    assert_eq!(
        events.len(),
        expected_events.len(),
        "workspace-template must have exactly {} events",
        expected_events.len()
    );
}

#[test]
fn install_skills_uses_rust_only_entrypoints() {
    assert!(!project_root().join("scripts/install_skills.sh").exists());
    let mod_source =
        read_text(&project_root().join("core/host-projection/src/host_integration/mod.rs"));
    let roots_source =
        read_text(&project_root().join("core/host-projection/src/host_integration/roots.rs"));
    let projection_source = read_text(
        &project_root().join("core/host-projection/src/host_integration/projection/mod.rs"),
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
    // codex_hooks 模块已被统一 host projection 取代，不再有 provider 特定引用
}

#[test]
fn prompt_policy_is_rust_owned() {
    let root = project_root();
    let mod_rs = read_text(&root.join("core/runtime-core/src/framework_runtime/mod.rs"));
    let compression =
        read_text(&root.join("core/framework-extra/src/prompt_compression.rs"));
    assert!(mod_rs.contains("build_framework_prompt_compression_envelope"));
    assert!(compression.contains("prompt_policy_owner"));
}
