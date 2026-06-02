use super::*;

pub(crate) fn compatibility_alias_inventory() -> Value {
    json!({
        "schema_version": "framework-compatibility-alias-inventory-v1",
        "aliases": [
            {
                "alias": "codex host-integration ...",
                "primary_command": "framework host-integration ...",
                "owner": "host-integration",
                "reason": "backward-compatible parser path for existing Codex automation that has not moved to the host-neutral namespace",
                "independent_behavior": false,
                "kept_policy": "thin parser alias only; dispatches to the same host-integration implementation as the primary command",
                "removal_condition": "remove after all checked-in docs, tests, bootstrap snippets, and generated host entrypoints use framework host-integration",
            },
            {
                "alias": "framework host-integration install-skills",
                "primary_command": "framework host-integration install",
                "owner": "host-integration",
                "reason": "backward-compatible install command spelling for existing project-local projection setup calls",
                "independent_behavior": false,
                "kept_policy": "thin subcommand alias only; maps to install with compatibility_alias=true metadata",
                "removal_condition": "remove after project-local projection installs and docs no longer call install-skills",
            },
            {
                "alias": "--repo-root",
                "primary_command": "--framework-root",
                "owner": "root-resolution",
                "reason": "backward-compatible flag for selecting the shared framework root in old automation",
                "independent_behavior": false,
                "kept_policy": "framework-root alias only; never resolves or fills project_root",
                "removal_condition": "kept indefinitely unless all old automation migrates to --framework-root and no docs/tests/generated entrypoints reference --repo-root",
            }
        ]
    })
}

/// Lightweight summary for `framework doctor` (full manifest regen is expensive).
pub(crate) fn generated_artifacts_status_for_repo(repo_root: &Path) -> Result<Value, String> {
    generated_artifacts_status(
        Some(repo_root),
        Some(&repo_root.join("artifacts")),
        true,
    )
}

pub(crate) fn generated_artifacts_status(
    framework_root: Option<&Path>,
    artifact_root: Option<&Path>,
    skip_generator_run: bool,
) -> Result<Value, String> {
    let framework_root = resolve_framework_root(framework_root)?;
    let artifact_root = resolve_artifact_root(artifact_root, &framework_root)?;
    let manifest_path = framework_root.join("configs/framework/GENERATED_ARTIFACTS.json");
    let manifest = read_json_if_exists(&manifest_path)?.ok_or_else(|| {
        format!(
            "missing generated artifact manifest: {}",
            manifest_path.display()
        )
    })?;
    let manifest: GeneratedArtifactsManifest = serde_json::from_value(manifest)
        .map_err(|err| format!("invalid generated artifact manifest: {err}"))?;
    if manifest.schema_version != GENERATED_ARTIFACTS_MANIFEST_SCHEMA_VERSION {
        return Err(format!(
            "unsupported generated artifact manifest schema_version {:?} at {}; expected {}",
            manifest.schema_version,
            manifest_path.display(),
            GENERATED_ARTIFACTS_MANIFEST_SCHEMA_VERSION
        ));
    }
    let temp_root_guard = if skip_generator_run {
        None
    } else {
        Some(prepare_generated_artifact_temp_root(
            &framework_root,
            &artifact_root,
        )?)
    };
    let temp_root = temp_root_guard.as_ref().map(|guard| guard.path());
    let mut results = Vec::new();
    let mut ok = true;
    let mut declared_paths = BTreeSet::new();
    let mut executed_generators = BTreeSet::new();

    for artifact in &manifest.generated_artifacts {
        validate_generated_artifact_entry(artifact)?;
        declared_paths.insert(artifact.path.clone());
        if !skip_generator_run {
            let temp_root = temp_root.expect("temp root prepared when generators run");
            if executed_generators.insert(artifact.generator.clone()) {
                run_generated_artifact_generator(&artifact.generator, &framework_root, temp_root)?;
            }
        }
        let checked_in_path = framework_root.join(&artifact.path);
        let regenerated_path = temp_root.map(|root| root.join(&artifact.path));
        let exists = checked_in_path.is_file();
        let regenerated_exists = regenerated_path
            .as_ref()
            .is_some_and(|path| path.is_file());
        let checked_in = if exists {
            Some(fs::read(&checked_in_path).map_err(|err| err.to_string())?)
        } else {
            None
        };
        let regenerated = if regenerated_exists {
            let path = regenerated_path.as_ref().expect("regenerated path");
            Some(fs::read(path).map_err(|err| err.to_string())?)
        } else {
            None
        };
        let forbidden = checked_in
            .as_ref()
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .map(|content| {
                if artifact.compare == "normalized-text" {
                    let normalized = normalize_generated_artifact_text(content, &[&framework_root]);
                    generated_artifact_forbidden_markers(&artifact.path, &normalized)
                } else {
                    generated_artifact_forbidden_markers(&artifact.path, content)
                }
            })
            .unwrap_or_default();
        let drifted = if skip_generator_run {
            false
        } else {
            generated_artifact_drifted(
                &artifact.compare,
                checked_in.as_deref(),
                regenerated.as_deref(),
                &framework_root,
                temp_root.expect("temp root prepared when drift is checked"),
            )?
        };
        let clean = if skip_generator_run {
            exists && forbidden.is_empty()
        } else {
            exists && regenerated_exists && !drifted && forbidden.is_empty()
        };
        ok &= clean;
        results.push(json!({
            "path": artifact.path,
            "exists": exists,
            "regenerated_exists": regenerated_exists,
            "clean": clean,
            "drifted": drifted,
            "forbidden_markers": forbidden,
            "compare": artifact.compare,
            "generator": artifact.generator,
        }));
    }

    let undeclared = undeclared_generated_framework_artifacts(&framework_root, &declared_paths)?;
    ok &= undeclared.is_empty();
    let drifted_artifacts: Vec<Value> = results
        .iter()
        .filter(|artifact| artifact.get("drifted").and_then(Value::as_bool) == Some(true))
        .map(|artifact| {
            json!({
                "path": artifact["path"].clone(),
                "generator": artifact["generator"].clone(),
                "compare": artifact["compare"].clone(),
            })
        })
        .collect();

    Ok(json!({
        "schema_version": "framework-generated-artifacts-status-v1",
        "ok": ok,
        "manifest_status": {
            "mode": if skip_generator_run {
                "manifest-backed-generated-artifact-metadata-only"
            } else {
                "manifest-backed-generated-artifact-drift-gate"
            },
            "artifact_root": artifact_root.to_string_lossy(),
            "temp_root": temp_root
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_default(),
            "skip_generator_run": skip_generator_run,
            "undeclared_generated_artifacts": undeclared,
            "declared_generated_artifact_paths": declared_paths.iter().cloned().collect::<Vec<_>>(),
            "drifted_artifacts": drifted_artifacts,
        },
        "drift_gate": {
            "enabled": true,
            "compare": ["byte-for-byte", "normalized-text"],
            "manifest": manifest_path.to_string_lossy(),
        },
        "framework_root": framework_root.to_string_lossy(),
        "artifact_root": artifact_root.to_string_lossy(),
        "manifest": manifest_path.to_string_lossy(),
        "generated_artifacts": results,
    }))
}

pub(crate) fn validate_generated_artifact_entry(
    artifact: &GeneratedArtifactManifestEntry,
) -> Result<(), String> {
    if Path::new(&artifact.path).is_absolute()
        || artifact.path.contains("..")
        || (artifact.path.starts_with('.') && !allowed_dot_generated_artifact(&artifact.path))
    {
        return Err(format!(
            "generated artifact path must be repo-relative and non-traversing: {}",
            artifact.path
        ));
    }
    if !matches!(
        artifact.compare.as_str(),
        "byte-for-byte" | "normalized-text"
    ) {
        return Err(format!(
            "unsupported generated artifact compare mode for {}: {}",
            artifact.path, artifact.compare
        ));
    }
    Ok(())
}

pub(crate) fn generated_artifact_drifted(
    compare: &str,
    checked_in: Option<&[u8]>,
    regenerated: Option<&[u8]>,
    framework_root: &Path,
    regenerated_root: &Path,
) -> Result<bool, String> {
    match compare {
        "byte-for-byte" => Ok(checked_in != regenerated),
        "normalized-text" => {
            let Some(checked_in) = checked_in else {
                return Ok(regenerated.is_some());
            };
            let Some(regenerated) = regenerated else {
                return Ok(true);
            };
            let checked_in = std::str::from_utf8(checked_in)
                .map_err(|err| format!("normalized-text artifact is not UTF-8: {err}"))?;
            let regenerated = std::str::from_utf8(regenerated)
                .map_err(|err| format!("normalized-text artifact is not UTF-8: {err}"))?;
            Ok(
                normalize_generated_artifact_text(checked_in, &[framework_root, regenerated_root])
                    != normalize_generated_artifact_text(
                        regenerated,
                        &[framework_root, regenerated_root],
                    ),
            )
        }
        other => Err(format!(
            "unsupported generated artifact compare mode: {other}"
        )),
    }
}

pub(crate) fn normalize_generated_artifact_text(content: &str, roots: &[&Path]) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut normalized = content.to_string();
    let mut roots = roots
        .iter()
        .map(|root| root.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    roots.sort_unstable_by_key(|root| std::cmp::Reverse(root.len()));
    for root in roots {
        normalized = normalized.replace(&root, "${FRAMEWORK_ROOT}");
    }
    normalized.replace(&home, "${HOME}")
}

pub(crate) fn allowed_dot_generated_artifact(path: &str) -> bool {
    matches!(
        path,
        ".codex/host_entrypoints_sync_manifest.json"
            | ".codex/README.md"
            | ".codex/prompts/framework.md"
            | ".claude/rules/framework.md"
            | ".claude/settings.json"
            | ".claude/.framework-projection.json"
            | ".claude/CLAUDE.md"
            | ".claude/mcp.json"
            | ".claude/.framework-projection-desktop.json"
            | ".gemini/antigravity/rules/framework.md"
            | ".gemini/settings.json"
            | ".gemini/mcp.json"
            | ".opencode/opencode.json"
            | ".opencode/.framework-projection.json"
    )
}

struct GeneratedArtifactTempRoot {
    path: PathBuf,
}

impl GeneratedArtifactTempRoot {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for GeneratedArtifactTempRoot {
    pub(crate) fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
        if let Some(parent) = self.path.parent() {
            let _ = fs::remove_dir(parent);
        }
    }
}

pub(crate) fn prepare_generated_artifact_temp_root(
    framework_root: &Path,
    artifact_root: &Path,
) -> Result<GeneratedArtifactTempRoot, String> {
    let temp_root = artifact_root
        .join("generated-artifacts-drift-check")
        .join(format!(
            "run-{}",
            Local::now().timestamp_nanos_opt().unwrap_or_default()
        ));
    copy_framework_tree_for_generation(framework_root, &temp_root)?;
    Ok(GeneratedArtifactTempRoot { path: temp_root })
}

pub(crate) fn copy_framework_tree_for_generation(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination)
        .map_err(|err| format!("failed to create {}: {err}", destination.display()))?;
    for entry in fs::read_dir(source)
        .map_err(|err| format!("failed to read directory {}: {err}", source.display()))?
    {
        let entry = entry.map_err(|err| {
            format!(
                "failed to read directory entry under {}: {err}",
                source.display()
            )
        })?;
        let path = entry.path();
        let name = entry.file_name();
        let name_text = name.to_string_lossy();
        let target = destination.join(&name);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|err| format!("failed to inspect {}: {err}", path.display()))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            if should_skip_generated_artifact_copy_dir(&name_text) {
                continue;
            }
            copy_framework_tree_for_generation(&path, &target)?;
        } else if metadata.is_file() {
            if should_skip_generated_artifact_copy_file(&name_text) {
                continue;
            }
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)
                    .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
            }
            fs::copy(&path, &target).map_err(|err| {
                format!(
                    "failed to copy {} to {}: {err}",
                    path.display(),
                    target.display()
                )
            })?;
        }
    }
    Ok(())
}

pub(crate) fn should_skip_generated_artifact_copy_dir(name: &str) -> bool {
    GENERATED_ARTIFACT_COPY_SKIP_DIR_NAMES.contains(&name)
}

pub(crate) fn should_skip_generated_artifact_copy_file(name: &str) -> bool {
    name == ".DS_Store" || name.ends_with(".marker")
}

pub(crate) fn run_generated_artifact_generator(
    generator: &str,
    framework_root: &Path,
    temp_root: &Path,
) -> Result<(), String> {
    let timeout = generated_artifact_generator_timeout();
    let log_stamp = Local::now().timestamp_nanos_opt().unwrap_or_default();
    let stdout_path = temp_root.join(format!(".generated-artifact-{log_stamp}.stdout.log"));
    let stderr_path = temp_root.join(format!(".generated-artifact-{log_stamp}.stderr.log"));
    let stdout_file = fs::File::create(&stdout_path)
        .map_err(|err| format!("failed to create {}: {err}", stdout_path.display()))?;
    let stderr_file = fs::File::create(&stderr_path)
        .map_err(|err| format!("failed to create {}: {err}", stderr_path.display()))?;
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(rewrite_generated_artifact_generator(
            generator,
            framework_root,
            temp_root,
        ))
        .current_dir(temp_root)
        .env("SKILL_FRAMEWORK_ROOT", temp_root)
        .env("SKILL_ARTIFACT_ROOT", temp_root.join("artifacts"))
        .env("ROUTER_RS_NO_REBUILD", "1")
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .map_err(|err| err.to_string())?;
    let start = Instant::now();
    loop {
        if child.try_wait().map_err(|err| err.to_string())?.is_some() {
            break;
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait().map_err(|err| err.to_string())?;
            let stdout = fs::read_to_string(&stdout_path).unwrap_or_default();
            let stderr = fs::read_to_string(&stderr_path).unwrap_or_default();
            return Err(format!(
                "generated artifact generator timed out after {}s: {generator}\nstdout:\n{}\nstderr:\n{}",
                timeout.as_secs(),
                stdout,
                stderr
            ));
        }
        thread::sleep(Duration::from_millis(100));
    }
    let status = child.wait().map_err(|err| err.to_string())?;
    let stdout = fs::read_to_string(&stdout_path).unwrap_or_default();
    let stderr = fs::read_to_string(&stderr_path).unwrap_or_default();
    let _ = fs::remove_file(&stdout_path);
    let _ = fs::remove_file(&stderr_path);
    if status.success() {
        return Ok(());
    }
    Err(format!(
        "generated artifact generator failed: {generator}\nstdout:\n{}\nstderr:\n{}",
        stdout, stderr
    ))
}

pub(crate) fn generated_artifact_generator_timeout() -> Duration {
    match std::env::var("ROUTER_RS_GENERATOR_TIMEOUT_SECONDS")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
    {
        Some(0) | None => GENERATED_ARTIFACT_GENERATOR_TIMEOUT,
        Some(seconds) => Duration::from_secs(seconds),
    }
}

pub(crate) fn rewrite_generated_artifact_generator(
    generator: &str,
    framework_root: &Path,
    temp_root: &Path,
) -> String {
    generator
        .replace(
            &framework_root.to_string_lossy().to_string(),
            &temp_root.to_string_lossy(),
        )
        .replace(
            "./scripts/",
            &format!("{}/scripts/", temp_root.to_string_lossy()),
        )
        .replace(
            " scripts/",
            &format!(" {}/scripts/", temp_root.to_string_lossy()),
        )
}

pub(crate) fn generated_artifact_forbidden_markers(path: &str, content: &str) -> Vec<&'static str> {
    let mut markers = Vec::new();
    for (name, needle) in [
        ("expanded-codex-home", "/Users/joe/.codex"),
        (
            "expanded-consuming-project-root",
            r"${HOME}/Documents/skill",
        ),
        ("copied-skill-body", "# Plan To Code"),
    ] {
        if content.contains(needle) {
            markers.push(name);
        }
    }
    if !path.starts_with("skills/SKILL_") && content.contains("\"skills\":[[") {
        markers.push("copied-runtime-payload");
    }
    markers
}

pub(crate) fn undeclared_generated_framework_artifacts(
    framework_root: &Path,
    declared_paths: &BTreeSet<String>,
) -> Result<Vec<String>, String> {
    let allowed_reports = surface_policy_generated_reports(framework_root)?;
    let mut undeclared = Vec::new();
    let candidates = generated_artifact_reverse_reference_candidates(framework_root)?;
    for path in candidates {
        let rel = path
            .strip_prefix(framework_root)
            .map_err(|err| err.to_string())?
            .to_string_lossy()
            .into_owned();
        if !declared_paths.contains(&rel) && !allowed_reports.contains(&rel) {
            undeclared.push(rel);
        }
    }
    undeclared.sort();
    undeclared.dedup();
    Ok(undeclared)
}

pub(crate) fn surface_policy_generated_reports(framework_root: &Path) -> Result<BTreeSet<String>, String> {
    let path = framework_root.join("configs/framework/FRAMEWORK_SURFACE_POLICY.json");
    let Some(policy) = read_json_if_exists(&path)? else {
        return Ok(BTreeSet::new());
    };
    let mut reports = BTreeSet::new();
    for key in ["derived_reports", "deprecated_or_foldable_reports"] {
        let Some(items) = policy.get(key).and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            let Some(rel) = item.as_str() else {
                continue;
            };
            reports.insert(rel.to_string());
        }
    }
    Ok(reports)
}

pub(crate) fn generated_artifact_reverse_reference_candidates(
    framework_root: &Path,
) -> Result<Vec<PathBuf>, String> {
    let mut candidates = Vec::new();
    for rel in [
        "configs/framework",
        "docs",
        "tests",
        ".github/workflows",
        ".codex",
        ".claude",
        ".gemini",
        ".cursor/rules",
    ] {
        collect_generated_artifact_marker_files(
            framework_root,
            &framework_root.join(rel),
            &mut candidates,
        )?;
    }
    let rel = "AGENTS.md";
    collect_generated_artifact_marker_files(
        framework_root,
        &framework_root.join(rel),
        &mut candidates,
    )?;
    collect_root_skill_generated_surfaces(framework_root, &mut candidates)?;
    Ok(candidates)
}

pub(crate) fn collect_root_skill_generated_surfaces(
    framework_root: &Path,
    candidates: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let skills_root = framework_root.join("skills");
    if !skills_root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(&skills_root).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.starts_with("SKILL_") {
            collect_generated_artifact_marker_files(framework_root, &path, candidates)?;
        }
    }
    Ok(())
}

pub(crate) fn collect_generated_artifact_marker_files(
    framework_root: &Path,
    path: &Path,
    candidates: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            return Ok(());
        };
        if matches!(name, ".git" | "target" | "artifacts") {
            return Ok(());
        }
        for entry in fs::read_dir(path).map_err(|err| err.to_string())? {
            let entry = entry.map_err(|err| err.to_string())?;
            collect_generated_artifact_marker_files(framework_root, &entry.path(), candidates)?;
        }
        return Ok(());
    }
    if !path.is_file() || !is_generated_artifact_scan_file(path) {
        return Ok(());
    }
    let Some(content) = read_text_if_exists(path)? else {
        return Ok(());
    };
    if !content.contains("generated-by-") && !is_managed_projection_content(&content) {
        return Ok(());
    }
    let rel = path
        .strip_prefix(framework_root)
        .map_err(|err| err.to_string())?;
    if rel.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|item| matches!(item, "target" | "artifacts"))
    }) {
        return Ok(());
    }
    candidates.push(path.to_path_buf());
    Ok(())
}

pub(crate) fn is_generated_artifact_scan_file(path: &Path) -> bool {
    if matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some("AGENTS.md")
    ) {
        return true;
    }
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("json" | "md" | "toml" | "yaml" | "yml" | "txt")
    )
}

pub(crate) fn skills_source_rel(repo_root: &Path) -> Result<String, String> {
    let registry = load_runtime_registry(repo_root)?;
    let source_rel = registry
        .workspace_bootstrap_defaults
        .skills
        .source_rel
        .unwrap_or_else(|| "skills".to_string());
    validate_source_rel(&source_rel)?;
    Ok(source_rel)
}

pub(crate) fn validate_source_rel(source_rel: &str) -> Result<(), String> {
    let candidate = Path::new(source_rel);
    if candidate.as_os_str().is_empty() {
        return Err("skills source_rel must not be empty".to_string());
    }
    if candidate.is_absolute() {
        return Err(format!(
            "skills source_rel must be repository-relative, got absolute path: {source_rel}"
        ));
    }
    if candidate
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(format!(
            "skills source_rel must not contain '..' segments: {source_rel}"
        ));
    }
    Ok(())
}

pub(crate) fn resolve_router_rs_executable(repo_root: &Path) -> Result<PathBuf, String> {
    if let Ok(raw) = std::env::var("ROUTER_RS_BIN") {
        let path = PathBuf::from(raw);
        if path.is_file() {
            return Ok(path);
        }
    }
    let installed = crate::router_self::default_router_rs_install_path();
    if installed.is_file() {
        return Ok(installed);
    }
    if let Ok(exe) = which::which("router-rs") {
        let path_text = exe.to_string_lossy();
        if !is_ephemeral_executable_path(&path_text) {
            return Ok(exe);
        }
    }
    if let Ok(td) = std::env::var("CARGO_TARGET_DIR") {
        let base = PathBuf::from(td);
        for candidate in [base.join("release/router-rs"), base.join("debug/router-rs")] {
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    let cur = std::env::current_exe().map_err(|err| err.to_string())?;
    if cur
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "router-rs" || name.starts_with("router-rs"))
    {
        return Ok(cur);
    }
    let repo_root = normalize_path(repo_root)?;
    for candidate in [
        repo_root.join("target/release/router-rs"),
        repo_root.join("target/debug/router-rs"),
        repo_root.join("core/router-rs/target/release/router-rs"),
        repo_root.join("core/router-rs/target/debug/router-rs"),
    ] {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "could not resolve router-rs executable for subprocess (try `cargo build --release --manifest-path core/router-rs/Cargo.toml`, `router-rs self install`, or set ROUTER_RS_BIN); repo_root={}",
        repo_root.display()
    ))
}

pub(crate) fn run_router_rs_json(repo_root: &Path, args: &[String]) -> Result<Value, String> {
    let exe = resolve_router_rs_executable(repo_root)?;
    let output = Command::new(&exe)
        .args(args)
        .arg("--repo-root")
        .arg(repo_root)
        .output()
        .map_err(|err| err.to_string())?;
    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).map_err(|err| err.to_string())?;
        return serde_json::from_str(stdout.trim()).map_err(|err| err.to_string());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if stderr.is_empty() {
        format!(
            "router-rs subprocess failed (status {:?}); executable {}",
            output.status,
            exe.display()
        )
    } else {
        stderr
    })
}

