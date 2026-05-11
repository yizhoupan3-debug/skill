use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) struct HostEntrypointPayloadProvider {
    pub(crate) files: BTreeMap<String, Vec<u8>>,
    pub(crate) json_relative_paths: Vec<String>,
    pub(crate) manifest_relative_path: String,
    pub(crate) agent_policy_entrypoint: String,
    pub(crate) after_apply: Option<fn(&Path) -> Result<Value, String>>,
}

struct HostEntrypointSyncSection {
    text_files: Vec<String>,
    json_files: Vec<String>,
}

fn host_entrypoint_partial_sync_section(
    provider: &HostEntrypointPayloadProvider,
    _desired_files: &BTreeMap<String, Vec<u8>>,
) -> HostEntrypointSyncSection {
    let mut json_files = provider.json_relative_paths.clone();
    json_files.push(provider.manifest_relative_path.clone());
    HostEntrypointSyncSection {
        text_files: Vec::new(),
        json_files,
    }
}

#[derive(Default)]
struct SingleSyncReport {
    written: Vec<String>,
    would_write: Vec<String>,
    unchanged: Vec<String>,
    created_dirs: Vec<String>,
}

pub(crate) fn sync_host_entrypoints(
    repo_root: &Path,
    apply: bool,
    provider: HostEntrypointPayloadProvider,
) -> Result<Value, String> {
    let root = normalize_repo_root(repo_root)?;
    let desired_files = collect_host_sync_file_bytes(&root, provider)?;
    let partial_section =
        host_entrypoint_partial_sync_section(&desired_files.provider, &desired_files.files);
    let (matched_worktrees, skipped_worktrees) = discover_matching_worktrees(&root);
    let mut report = json!({
        "written": [],
        "would_write": [],
        "unchanged": [],
        "created_dirs": [],
        "synced_worktrees": [],
        "skipped_worktrees": skipped_worktrees,
    });
    let full_text_files =
        desired_host_entrypoint_text_files(&desired_files.provider, &desired_files.files);
    let mut full_json_files = desired_files.provider.json_relative_paths.clone();
    full_json_files.push(desired_files.provider.manifest_relative_path.clone());
    let full_section = HostEntrypointSyncSection {
        text_files: full_text_files
            .into_iter()
            .map(|path| path.to_string())
            .collect(),
        json_files: full_json_files
            .into_iter()
            .map(|path| path.to_string())
            .collect(),
    };
    let mut targets = vec![root.clone()];
    targets.extend(matched_worktrees);

    for target_root in targets {
        let section = if target_root == root {
            &full_section
        } else {
            &partial_section
        };
        let single = match sync_host_entrypoints_single_root(
            &desired_files,
            &target_root,
            &root,
            apply,
            section,
        ) {
            Ok(single) => single,
            Err(err) if target_root != root => {
                extend_report_array(
                    &mut report,
                    "skipped_worktrees",
                    vec![format!("{} ({err})", target_root.to_string_lossy())],
                )?;
                continue;
            }
            Err(err) => return Err(err),
        };
        extend_report_array(&mut report, "written", single.written)?;
        extend_report_array(&mut report, "would_write", single.would_write)?;
        extend_report_array(&mut report, "unchanged", single.unchanged)?;
        extend_report_array(&mut report, "created_dirs", single.created_dirs)?;
        if target_root != root {
            extend_report_array(
                &mut report,
                "synced_worktrees",
                vec![target_root.to_string_lossy().into_owned()],
            )?;
        }
    }

    sort_report_array(&mut report, "written")?;
    sort_report_array(&mut report, "would_write")?;
    sort_report_array(&mut report, "unchanged")?;
    sort_report_array(&mut report, "created_dirs")?;
    sort_report_array(&mut report, "synced_worktrees")?;
    sort_report_array(&mut report, "skipped_worktrees")?;
    if apply {
        if let Some(after_apply) = desired_files.provider.after_apply {
            after_apply(&root)?;
        }
    }
    Ok(report)
}

struct DesiredHostEntrypointFiles {
    provider: HostEntrypointPayloadProvider,
    files: BTreeMap<String, Vec<u8>>,
}

fn collect_host_sync_file_bytes(
    repo_root: &Path,
    provider: HostEntrypointPayloadProvider,
) -> Result<DesiredHostEntrypointFiles, String> {
    let mut files = provider.files.clone();
    files.insert(
        provider.manifest_relative_path.clone(),
        serialize_pretty_json_bytes(&build_host_entrypoint_sync_manifest(
            &provider, &files, repo_root,
        )?)?,
    );
    Ok(DesiredHostEntrypointFiles { provider, files })
}

fn build_host_entrypoint_sync_manifest(
    provider: &HostEntrypointPayloadProvider,
    desired_files: &BTreeMap<String, Vec<u8>>,
    repo_root: &Path,
) -> Result<Value, String> {
    let full_text_files = desired_host_entrypoint_text_files(provider, desired_files);
    let mut json_files = provider.json_relative_paths.clone();
    json_files.push(provider.manifest_relative_path.clone());
    let mut shared_system =
        crate::framework_host_targets::sync_manifest_shared_system_block(repo_root)?;
    if let Some(obj) = shared_system.as_object_mut() {
        obj.insert(
            "agent_policy_entrypoint".to_string(),
            Value::String(provider.agent_policy_entrypoint.clone()),
        );
    }
    Ok(json!({
        "schema_version": "host-entrypoints-sync-manifest-v1",
        "shared_system": shared_system,
        "full_sync": {
            "text_files": full_text_files,
            "json_files": json_files.clone(),
        },
        "partial_sync": {
            "text_files": [],
            "json_files": json_files,
        },
    }))
}

fn desired_host_entrypoint_text_files(
    provider: &HostEntrypointPayloadProvider,
    desired_files: &BTreeMap<String, Vec<u8>>,
) -> Vec<String> {
    desired_files
        .keys()
        .filter(|path| path.as_str() != provider.manifest_relative_path)
        .filter(|path| !provider.json_relative_paths.contains(path))
        .cloned()
        .collect()
}

fn serialize_pretty_json_bytes(payload: &Value) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(payload).map_err(|err| err.to_string())?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn sync_host_entrypoints_single_root(
    desired_files: &DesiredHostEntrypointFiles,
    target_root: &Path,
    report_root: &Path,
    apply: bool,
    section: &HostEntrypointSyncSection,
) -> Result<SingleSyncReport, String> {
    let mut report = SingleSyncReport::default();
    for relative in section.text_files.iter().chain(section.json_files.iter()) {
        let desired = desired_files
            .files
            .get(relative)
            .ok_or_else(|| format!("missing generated host-entrypoint payload for {}", relative))?;
        sync_host_entrypoint_file(
            desired,
            relative,
            target_root,
            report_root,
            apply,
            &mut report,
        )?;
    }

    Ok(report)
}

fn sync_host_entrypoint_file(
    desired: &[u8],
    relative: &str,
    target_root: &Path,
    report_root: &Path,
    apply: bool,
    report: &mut SingleSyncReport,
) -> Result<(), String> {
    let destination = target_root.join(relative);
    let existing = fs::read(&destination).ok();
    let changed = existing.as_deref() != Some(desired);
    if changed && apply {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        fs::write(&destination, desired).map_err(|err| err.to_string())?;
    }
    let bucket = if changed && apply {
        &mut report.written
    } else if changed {
        &mut report.would_write
    } else {
        &mut report.unchanged
    };
    bucket.push(describe_host_entrypoint_path(
        report_root,
        target_root,
        &destination,
    ));
    Ok(())
}

fn extend_report_array(report: &mut Value, key: &str, items: Vec<String>) -> Result<(), String> {
    let array = report
        .get_mut(key)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| format!("host-entrypoint sync report missing {key} array"))?;
    array.extend(items.into_iter().map(Value::String));
    Ok(())
}

fn sort_report_array(report: &mut Value, key: &str) -> Result<(), String> {
    let array = report
        .get_mut(key)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| format!("host-entrypoint sync report missing {key} array"))?;
    let mut values = array
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    values.sort();
    *array = values.into_iter().map(Value::String).collect();
    Ok(())
}

fn normalize_repo_root(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()
            .map_err(|err| err.to_string())?
            .join(path))
    }
}

fn discover_matching_worktrees(root: &Path) -> (Vec<PathBuf>, Vec<String>) {
    let worktree_listing = read_git_stdout(root, &["worktree", "list", "--porcelain"]);
    if worktree_listing.is_none() {
        return (Vec::new(), Vec::new());
    }

    let mut current: BTreeMap<String, String> = BTreeMap::new();
    let mut worktrees = Vec::new();
    for raw_line in worktree_listing.unwrap_or_default().lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            if !current.is_empty() {
                worktrees.push(current);
                current = BTreeMap::new();
            }
            continue;
        }
        let mut parts = line.splitn(2, ' ');
        let key = parts.next().unwrap_or_default().to_string();
        let value = parts.next().unwrap_or_default().to_string();
        current.insert(key, value);
    }
    if !current.is_empty() {
        worktrees.push(current);
    }

    let mut matches = Vec::new();
    let mut skipped = Vec::new();
    for entry in worktrees {
        let Some(worktree_path) = entry.get("worktree") else {
            continue;
        };
        let candidate = normalize_repo_root(Path::new(worktree_path))
            .unwrap_or_else(|_| PathBuf::from(worktree_path));
        if candidate == root {
            continue;
        }
        if !candidate.exists() {
            skipped.push(format!("{} (missing)", candidate.to_string_lossy()));
            continue;
        }
        matches.push(candidate);
    }
    (matches, skipped)
}

fn read_git_stdout(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn describe_host_entrypoint_path(report_root: &Path, target_root: &Path, path: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(report_root) {
        return relative.to_string_lossy().into_owned();
    }
    if let Ok(relative) = path.strip_prefix(target_root) {
        return format!(
            "{}::{}",
            target_root.to_string_lossy(),
            relative.to_string_lossy()
        );
    }
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn host_entrypoint_sync_engine_accepts_fake_provider_in_dry_run() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("router-rs-host-entrypoint-sync-{stamp}"));
        fs::create_dir_all(&root).unwrap();
        let registry_dir = root.join("configs/framework");
        fs::create_dir_all(&registry_dir).unwrap();
        let registry_json = registry_dir.join("RUNTIME_REGISTRY.json");
        fs::write(
            &registry_json,
            r#"{"schema_version":"framework-runtime-registry-v1","host_targets":{"supported":["codex-cli","codex-app","cursor","claude-code"],"metadata":{"codex-cli":{"install_tool":"codex","host_entrypoints":"AGENTS.md"},"codex-app":{"install_tool":"codex","host_entrypoints":"AGENTS.md"},"cursor":{"install_tool":"cursor","host_entrypoints":["AGENTS.md",".cursor/rules/*.mdc"]},"claude-code":{"install_tool":"claude","host_entrypoints":["AGENTS.md",".claude/rules/framework.md"]}}}}"#,
        )
        .unwrap();
        assert!(
            registry_json.is_file(),
            "fixture RUNTIME_REGISTRY.json missing at {}",
            registry_json.display()
        );

        let mut files = BTreeMap::new();
        files.insert("AGENTS.md".to_string(), b"fake policy\n".to_vec());
        files.insert(".fake/hooks.json".to_string(), b"{\"hooks\":{}}\n".to_vec());
        let provider = HostEntrypointPayloadProvider {
            files,
            json_relative_paths: vec![".fake/hooks.json".to_string()],
            manifest_relative_path: ".fake/host_entrypoints_sync_manifest.json".to_string(),
            agent_policy_entrypoint: "AGENTS.md".to_string(),
            after_apply: None,
        };

        let report = sync_host_entrypoints(&root, false, provider).unwrap();
        let would_write = report
            .get("would_write")
            .and_then(Value::as_array)
            .unwrap()
            .len();
        let written = report
            .get("written")
            .and_then(Value::as_array)
            .unwrap()
            .len();
        assert!(would_write > 0);
        assert_eq!(written, 0);

        fs::remove_dir_all(&root).unwrap();
    }

    fn git(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .status()
            .expect("git command should spawn");
        assert!(status.success(), "git {:?} failed", args);
    }

    #[test]
    fn partial_sync_does_not_overwrite_worktree_text_entrypoints() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!("router-rs-host-entrypoint-worktree-{stamp}"));
        let root = base.join("main");
        let sibling = base.join("sibling");
        fs::create_dir_all(&root).unwrap();
        git(&root, &["init"]);
        git(&root, &["config", "user.email", "test@example.com"]);
        git(&root, &["config", "user.name", "Test User"]);
        fs::write(root.join("README.md"), "seed\n").unwrap();
        git(&root, &["add", "README.md"]);
        git(&root, &["commit", "-m", "seed"]);
        git(
            &root,
            &[
                "worktree",
                "add",
                sibling.to_str().unwrap(),
                "-b",
                "sibling",
            ],
        );

        let registry_dir = root.join("configs/framework");
        fs::create_dir_all(&registry_dir).unwrap();
        fs::write(
            registry_dir.join("RUNTIME_REGISTRY.json"),
            r#"{"schema_version":"framework-runtime-registry-v1","host_targets":{"supported":["codex-cli","codex-app","cursor","claude-code"],"metadata":{"codex-cli":{"install_tool":"codex","host_entrypoints":"AGENTS.md"},"codex-app":{"install_tool":"codex","host_entrypoints":"AGENTS.md"},"cursor":{"install_tool":"cursor","host_entrypoints":["AGENTS.md",".cursor/rules/*.mdc"]},"claude-code":{"install_tool":"claude","host_entrypoints":["AGENTS.md",".claude/rules/framework.md"]}}}}"#,
        )
        .unwrap();
        fs::write(sibling.join("AGENTS.md"), "local sibling policy\n").unwrap();

        let mut files = BTreeMap::new();
        files.insert("AGENTS.md".to_string(), b"generated root policy\n".to_vec());
        files.insert(".fake/hooks.json".to_string(), b"{\"hooks\":{}}\n".to_vec());
        let provider = HostEntrypointPayloadProvider {
            files,
            json_relative_paths: vec![".fake/hooks.json".to_string()],
            manifest_relative_path: ".fake/host_entrypoints_sync_manifest.json".to_string(),
            agent_policy_entrypoint: "AGENTS.md".to_string(),
            after_apply: None,
        };

        let report = sync_host_entrypoints(&root, true, provider).unwrap();
        let sibling_canonical = sibling
            .canonicalize()
            .unwrap_or_else(|_| sibling.to_path_buf())
            .to_string_lossy()
            .into_owned();
        assert!(report["synced_worktrees"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str() == Some(sibling_canonical.as_str())));
        assert_eq!(
            fs::read_to_string(sibling.join("AGENTS.md")).unwrap(),
            "local sibling policy\n"
        );
        assert!(sibling.join(".fake/hooks.json").is_file());

        fs::remove_dir_all(&base).unwrap();
    }
}
