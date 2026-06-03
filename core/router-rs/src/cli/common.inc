pub(crate) fn print_json_value<T: Serialize>(payload: &T) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string(payload).map_err(|err| format!("serialize output failed: {err}"))?
    );
    Ok(())
}

pub(crate) fn parse_json_input<T>(raw: &str, context: &str) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_str(raw).map_err(|err| format!("parse {context} input failed: {err}"))
}

fn repo_root_from_cargo_manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn repo_root_for_runtime_path(runtime_path: &Path) -> PathBuf {
    let parent = runtime_path.parent().unwrap_or_else(|| Path::new("."));
    if parent.file_name().and_then(|name| name.to_str()) == Some("skills") {
        parent
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(repo_root_from_cargo_manifest_dir)
    } else {
        parent.to_path_buf()
    }
}

pub(crate) fn manifest_fallback_path(
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
) -> Result<Option<PathBuf>, String> {
    if let Some(path) = manifest_path {
        if path.exists() {
            return Ok(Some(path.to_path_buf()));
        }
        return Err(format!("manifest path does not exist: {}", path.display()));
    }
    let runtime_declared_fallback = match runtime_path {
        Some(path) => resolve_runtime_declared_manifest_fallback(path)?,
        None => None,
    };
    let fallback = runtime_declared_fallback
        .filter(|path| path.exists())
        .or_else(|| {
            runtime_path
                .and_then(Path::parent)
                .map(|parent| parent.join("SKILL_MANIFEST.json"))
                .filter(|path| path.exists())
        })
        .or_else(|| {
            Some(
                repo_root_from_cargo_manifest_dir()
                    .join("skills")
                    .join("SKILL_MANIFEST.json"),
            )
            .filter(|path| path.exists())
        });
    Ok(fallback)
}

pub(crate) fn resolve_runtime_declared_manifest_fallback(
    runtime_path: &Path,
) -> Result<Option<PathBuf>, String> {
    let runtime_payload = read_json(runtime_path)?;
    let declared = runtime_payload
        .get("scope")
        .and_then(Value::as_object)
        .and_then(|scope| scope.get("fallback_manifest"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(declared) = declared else {
        return Ok(None);
    };
    let resolved = if Path::new(declared).is_absolute() {
        PathBuf::from(declared)
    } else if declared == "skills" || declared.starts_with("skills/") {
        repo_root_for_runtime_path(runtime_path).join(declared)
    } else {
        runtime_path
            .parent()
            .map(|parent| parent.join(declared))
            .unwrap_or_else(|| repo_root_from_cargo_manifest_dir().join(declared))
    };
    Ok(Some(resolved))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn route_task_with_manifest_fallback(
    runtime_records: &[SkillRecord],
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
    host_id: Option<&str>,
    query: &str,
    session_id: &str,
    allow_overlay: bool,
    first_turn: bool,
) -> Result<RouteDecision, String> {
    let scoped_runtime = crate::route::filter_records_for_host(runtime_records, host_id)?;
    if let Some(decision) = literal_framework_alias_decision(&scoped_runtime, query, session_id) {
        return Ok(decision);
    }
    let hot_decision = route_task(
        &scoped_runtime,
        query,
        session_id,
        allow_overlay,
        first_turn,
    )?;
    let should_retry = should_retry_with_manifest(&hot_decision);
    let Some(fallback_path) = manifest_fallback_path(runtime_path, manifest_path)? else {
        return Ok(hot_decision);
    };
    let full_records = match load_records_from_manifest(&fallback_path)
        .and_then(|records| crate::route::filter_records_for_host(records, host_id))
    {
        Ok(records) => records,
        Err(err) if !should_retry && manifest_path.is_none() => {
            let mut degraded = hot_decision;
            degraded
                .reasons
                .push(format!("Manifest fallback unavailable: {err}"));
            return Ok(degraded);
        }
        Err(err) => return Err(err),
    };
    if let Some(decision) = literal_framework_alias_decision(&full_records, query, session_id) {
        return Ok(decision);
    }
    let full_decision = route_task(&full_records, query, session_id, allow_overlay, first_turn)?;
    if should_accept_manifest_fallback(
        &hot_decision,
        &full_decision,
        &scoped_runtime,
        should_retry,
        manifest_path.is_some(),
    ) {
        return Ok(full_decision);
    }
    Ok(hot_decision)
}
pub(crate) fn append_io_lock() -> &'static Mutex<()> {
    static APPEND_IO_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    APPEND_IO_LOCK.get_or_init(|| Mutex::new(()))
}

/// Resolve symlinks on the longest existing ancestor of `path`, then re-attach
/// any non-existing tail components verbatim. Mirrors runtime_storage.rs
/// `canonicalize_existing_ancestors` for containment checks against a canonical root.
fn canonicalize_existing_ancestors(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!(
            "write path must be absolute before symlink resolution: {}",
            path.display()
        ));
    }
    let mut current = path.to_path_buf();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    loop {
        match fs::symlink_metadata(&current) {
            Ok(_) => break,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                let Some(file_name) = current.file_name().map(|name| name.to_os_string()) else {
                    return Err(format!(
                        "write path has no existing ancestor: {}",
                        path.display()
                    ));
                };
                tail.push(file_name);
                current = current.parent().map(Path::to_path_buf).ok_or_else(|| {
                    format!("write path ancestor resolution failed: {}", path.display())
                })?;
            }
            Err(err) => {
                return Err(format!(
                    "stat write path ancestor {} failed: {err}",
                    current.display()
                ));
            }
        }
    }
    let canonical = fs::canonicalize(&current).map_err(|err| {
        format!(
            "canonicalize write path ancestor {} failed: {err}",
            current.display()
        )
    })?;
    let mut resolved = canonical;
    for component in tail.into_iter().rev() {
        resolved = resolved.join(component);
    }
    Ok(resolved)
}

/// Unified security guard for all write entry points.
///
/// Performs the following checks (in order):
///   1. Reject `..` path traversal components.
///   2. Reject symlinks at the target path itself.
///   3. If `allowed_root` is provided, resolve symlinks on existing ancestor
///      directories of both the root and the target path, then verify the
///      target remains under the root (prevents ancestor-directory symlink escape).
///   4. Create parent directories if needed (after all validation passes).
pub(crate) fn validate_write_path(path: &Path, allowed_root: Option<&Path>) -> Result<(), String> {
    // 1. Reject path traversal via `..` components.
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(format!(
            "write path {} must not contain '..' traversal segments",
            path.display()
        ));
    }
    // 2. Reject symlink at the target path itself.
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_symlink() => {
            return Err(format!(
                "write path {} must not be a symlink",
                path.display()
            ));
        }
        Ok(_) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(format!(
                "stat write path {} failed: {err}",
                path.display()
            ));
        }
    }
    // 3. Optional root containment: resolve symlinks on existing ancestors of
    //    both the allowed root and the target path, then verify containment.
    //    This prevents a symlink in an ancestor directory from redirecting the
    //    write outside the allowed boundary.
    if let Some(root) = allowed_root {
        let canonical_root = canonicalize_existing_ancestors(root)?;
        let canonical_path = canonicalize_existing_ancestors(path)?;
        if !canonical_path.starts_with(&canonical_root) {
            return Err(format!(
                "write path {} must stay under allowed root {} after symlink resolution",
                canonical_path.display(),
                canonical_root.display()
            ));
        }
    }
    // 4. Create parent directories (after all validation has passed).
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "create parent directory for {} failed: {err}",
                path.display()
            )
        })?;
    }
    Ok(())
}

pub(crate) fn append_text_with_process_lock(path: &Path, payload: &str, context: &str) -> Result<(), String> {
    validate_write_path(path, None)?;
    let _guard = append_io_lock()
        .lock()
        .map_err(|_| format!("{context} append lock poisoned"))?;
    let mut file = open_append_nofollow(path)
        .map_err(|err| format!("open {context} append failed for {}: {err}", path.display()))?;
    use fs2::FileExt;
    file.lock_exclusive()
        .map_err(|err| format!("lock {context} append failed for {}: {err}", path.display()))?;
    file.write_all(payload.as_bytes()).map_err(|err| {
        format!(
            "write {context} append failed for {}: {err}",
            path.display()
        )
    })?;
    file.sync_data().map_err(|err| {
        format!(
            "sync {context} append failed for {}: {err}",
            path.display()
        )
    })
}

/// Open a file for append with O_NOFOLLOW on Unix to prevent symlink-following
/// at the file-descriptor level (defense-in-depth beyond the metadata check in
/// `validate_write_path`).
#[cfg(unix)]
fn open_append_nofollow(path: &Path) -> Result<fs::File, std::io::Error> {
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
}

#[cfg(not(unix))]
fn open_append_nofollow(path: &Path) -> Result<fs::File, std::io::Error> {
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
}
pub(crate) fn configure_compute_parallelism(override_value: Option<usize>) -> Result<(), String> {
    let Some(thread_count) = override_value
        .or_else(|| env_usize("ROUTER_RS_COMPUTE_THREADS"))
        .filter(|value| *value > 0)
        .map(|value| value.clamp(1, MAX_COMPUTE_THREADS))
    else {
        return Ok(());
    };
    ThreadPoolBuilder::new()
        .num_threads(thread_count)
        .build_global()
        .map_err(|err| format!("configure compute thread pool failed: {err}"))
}

pub(crate) fn env_usize(name: &str) -> Option<usize> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
}
