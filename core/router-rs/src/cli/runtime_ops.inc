

fn write_json_payload(path: &Path, payload: &Value) -> Result<usize, String> {
    let serialized = format!(
        "{}\n",
        serde_json::to_string_pretty(payload)
            .map_err(|err| format!("serialize persisted payload failed: {err}"))?
    );
    write_text_payload(path, &serialized)
}

pub(crate) fn write_transport_binding_payload(payload: Value) -> Result<Value, String> {
    let path = required_non_empty_string(&payload, "path", "write transport binding")?;
    let session_id = required_non_empty_string(&payload, "session_id", "write transport binding")?;
    let job_id = optional_non_empty_string(&payload, "job_id");
    let transport = build_trace_transport_payload(&payload, session_id, job_id);
    let bytes_written = write_json_payload(Path::new(&path), &transport)?;
    Ok(serde_json::json!({
        "schema_version": TRANSPORT_BINDING_WRITE_SCHEMA_VERSION,
        "authority": TRANSPORT_BINDING_WRITE_AUTHORITY,
        "path": path,
        "bytes_written": bytes_written,
    }))
}

pub(crate) fn write_checkpoint_resume_manifest_payload(payload: Value) -> Result<Value, String> {
    let path = required_non_empty_string(&payload, "path", "write checkpoint resume manifest")?;
    let manifest = build_checkpoint_resume_manifest(payload)?
        .get("resume_manifest")
        .cloned()
        .ok_or_else(|| "checkpoint resume manifest payload missing resume_manifest".to_string())?;
    let bytes_written = write_json_payload(Path::new(&path), &manifest)?;
    Ok(serde_json::json!({
        "schema_version": CHECKPOINT_MANIFEST_WRITE_SCHEMA_VERSION,
        "authority": CHECKPOINT_MANIFEST_WRITE_AUTHORITY,
        "path": path,
        "bytes_written": bytes_written,
    }))
}

pub(crate) fn write_text_payload(path: &Path, payload: &str) -> Result<usize, String> {
    validate_write_path(path, None)?;

    let parent = path.parent().ok_or_else(|| {
        format!(
            "write path {} has no parent directory",
            path.display()
        )
    })?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| format!("persist path {} has no file name", path.display()))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let thread_id = format!("{:?}", std::thread::current().id())
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>();

    // Atomic write: create temp with create_new (fails if name already exists,
    // preventing TOCTOU races), fsync, then rename over the target.
    // Follows the pattern in runtime_storage.rs filesystem_write_text_inner.
    const MAX_TEMP_ATTEMPTS: u32 = 128;
    let (tmp_path, mut file) = {
        let mut chosen: Option<(PathBuf, fs::File)> = None;
        for attempt in 0u32..MAX_TEMP_ATTEMPTS {
            let sequence = WRITE_TEXT_PAYLOAD_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let candidate = path.with_file_name(format!(
                "{file_name}.{pid}.{thread_id}.{nonce}.{sequence}.tmp"
            ));
            match fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&candidate)
            {
                Ok(file) => {
                    chosen = Some((candidate, file));
                    break;
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => {
                    return Err(format!(
                        "create temp payload {} failed: {err}",
                        candidate.display()
                    ));
                }
            }
        }
        chosen.ok_or_else(|| {
            "exhausted temp file create attempts (unexpected collision load)".to_string()
        })?
    };
    let write_result = file
        .write_all(payload.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|err| {
            format!(
                "write/sync temp payload {} failed: {err}",
                tmp_path.display()
            )
        });
    if let Err(err) = write_result {
        drop(file);
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }
    drop(file);
    fs::rename(&tmp_path, path).map_err(|err| {
        let _ = fs::remove_file(&tmp_path);
        format!("replace payload {} failed: {err}", path.display())
    })?;
    Ok(payload.len())
}
