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
    } else {
        runtime_path
            .parent()
            .map(|parent| parent.join(declared))
            .unwrap_or_else(|| repo_root_from_cargo_manifest_dir().join(declared))
    };
    Ok(Some(resolved))
}

pub(crate) fn route_task_with_manifest_fallback(
    runtime_records: &[SkillRecord],
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
    query: &str,
    session_id: &str,
    allow_overlay: bool,
    first_turn: bool,
) -> Result<RouteDecision, String> {
    if let Some(decision) = literal_framework_alias_decision(runtime_records, query, session_id) {
        return Ok(decision);
    }
    let hot_decision = route_task(
        runtime_records,
        query,
        session_id,
        allow_overlay,
        first_turn,
    )?;
    let should_retry = should_retry_with_manifest(&hot_decision);
    let Some(fallback_path) = manifest_fallback_path(runtime_path, manifest_path)? else {
        return Ok(hot_decision);
    };
    let full_records = match load_records_from_manifest(&fallback_path) {
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
    if full_records.len() <= runtime_records.len() {
        return Ok(hot_decision);
    }
    if let Some(decision) = literal_framework_alias_decision(&full_records, query, session_id) {
        return Ok(decision);
    }
    let full_decision = route_task(&full_records, query, session_id, allow_overlay, first_turn)?;
    if should_accept_manifest_fallback(
        &hot_decision,
        &full_decision,
        runtime_records,
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

pub(crate) fn append_text_with_process_lock(path: &Path, payload: &str, context: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "create {context} parent failed for {}: {err}",
                parent.display()
            )
        })?;
    }
    let _guard = append_io_lock()
        .lock()
        .map_err(|_| format!("{context} append lock poisoned"))?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| format!("open {context} append failed for {}: {err}", path.display()))?;
    file.write_all(payload.as_bytes()).map_err(|err| {
        format!(
            "write {context} append failed for {}: {err}",
            path.display()
        )
    })
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
