// CDP/Chrome 助手、Attach 候选、skill 路由与 MCP 收尾工具函数。
fn wait_for_cdp(port: u16) -> Result<(), Value> {
    let deadline = SystemTime::now() + Duration::from_secs(8);
    while SystemTime::now() < deadline {
        if cdp_version_json(port).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(browser_error(
        "BROWSER_LAUNCH_FAILED",
        "Chrome remote debugging endpoint did not become ready.",
        &["retry browser_open"],
        false,
    ))
}

fn cdp_version_json(port: u16) -> Result<Value, Value> {
    cdp_http_json(port, "/json/version")
}

fn cdp_http_json(port: u16, path: &str) -> Result<Value, Value> {
    reqwest::blocking::get(format!("http://127.0.0.1:{port}{path}"))
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json::<Value>())
        .map_err(|err| {
            browser_error(
                "CDP_HTTP_FAILED",
                &format!("Chrome CDP HTTP request failed: {err}"),
                &["verify Chrome remote debugging is reachable"],
                true,
            )
        })
}

fn find_chrome_binary() -> Result<PathBuf, Value> {
    if let Ok(path) = std::env::var("BROWSER_MCP_CHROME_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
    }
    let candidates = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
    ];
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
        .ok_or_else(|| {
            browser_error(
                "BROWSER_LAUNCH_FAILED",
                "No Chrome/Chromium binary was found.",
                &["install Google Chrome", "set BROWSER_MCP_CHROME_PATH"],
                false,
            )
        })
}

fn allocate_debug_port() -> u16 {
    49_000 + ((now_millis() % 10_000) as u16)
}

fn summary_expression() -> &'static str {
    r#"(function(){
const main = document.querySelector('main') || document.body;
const mainText = ((main && main.textContent) || '').replace(/\s+/g, ' ').trim();
const visibleText = ((document.body && document.body.innerText) || '').trim();
const seen = new Set();
const messages = [];
for (const raw of visibleText.split('\n')) {
  const line = raw.trim();
  if (line && !seen.has(line)) {
    seen.add(line);
    messages.push(line);
    if (messages.length >= 8) break;
  }
}
return {mainGoalArea: mainText.slice(0, 240), visibleMessages: messages.map(line => line.slice(0,160)), forms: document.querySelectorAll('form').length, dialogs: document.querySelectorAll('dialog,[role="dialog"],[aria-modal="true"]').length};
})()"#
}

fn element_collection_expression() -> &'static str {
    r#"(function(){
const selector = 'a,button,input,textarea,select,[role="button"],[role="link"],[contenteditable="true"],summary';
function roleFor(el){
  const role = el.getAttribute('role');
  if (role) return role;
  const tag = el.tagName.toLowerCase();
  if (tag === 'a') return 'link';
  if (tag === 'button' || el.type === 'button' || el.type === 'submit') return 'button';
  if (tag === 'input' || tag === 'textarea' || el.isContentEditable) return 'textbox';
  if (tag === 'select') return 'combobox';
  return tag;
}
function cssPath(el){
  if (el.dataset && el.dataset.testid) return `[data-testid="${CSS.escape(el.dataset.testid)}"]`;
  const parts = [];
  let node = el;
  while (node && node.nodeType === 1 && node !== document.body) {
    let part = node.tagName.toLowerCase();
    if (node.id) {
      part += `#${CSS.escape(node.id)}`;
      parts.unshift(part);
      break;
    }
    const parent = node.parentElement;
    if (!parent) break;
    const siblings = Array.from(parent.children).filter(child => child.tagName === node.tagName);
    if (siblings.length > 1) part += `:nth-of-type(${siblings.indexOf(node) + 1})`;
    parts.unshift(part);
    node = parent;
  }
  return parts.join(' > ');
}
return Array.from(document.querySelectorAll(selector)).map((el, index) => {
  const rect = el.getBoundingClientRect();
  const visible = !!(rect.width && rect.height) && getComputedStyle(el).visibility !== 'hidden' && getComputedStyle(el).display !== 'none';
  const label = el.getAttribute('aria-label') || el.getAttribute('placeholder') || el.innerText || el.value || el.textContent || '';
  return {role: roleFor(el), name: String(label).replace(/\s+/g,' ').trim().slice(0,120), text: String(el.innerText || el.textContent || '').replace(/\s+/g,' ').trim().slice(0,160), visible, enabled: !el.disabled, tag: el.tagName.toLowerCase(), testId: el.dataset ? el.dataset.testid || null : null, ordinal: index, selector: cssPath(el)};
}).filter(item => item.visible);
})()"#
}

fn create_fingerprint(
    descriptor: &ElementDescriptor,
    counts: &mut HashMap<String, usize>,
) -> String {
    if let Some(test_id) = descriptor.test_id.as_ref() {
        return format!("tid::{test_id}");
    }
    let base = format!(
        "{}::{}::{}",
        descriptor.role, descriptor.name, descriptor.tag
    );
    let count = counts.entry(base.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        base
    } else {
        format!("{base}#{}", *count)
    }
}

fn has_meaningful_change(previous: &PageSnapshot, next: &PageSnapshot) -> bool {
    if previous.url != next.url || previous.title != next.title {
        return true;
    }
    if previous.text_content != next.text_content {
        return true;
    }
    let previous_fingerprints = previous
        .interactive_elements
        .iter()
        .map(|element| element.fingerprint.as_str())
        .collect::<std::collections::HashSet<_>>();
    let next_fingerprints = next
        .interactive_elements
        .iter()
        .map(|element| element.fingerprint.as_str())
        .collect::<std::collections::HashSet<_>>();
    previous_fingerprints != next_fingerprints
}

fn compute_delta(previous: &PageSnapshot, next: &PageSnapshot) -> Value {
    let previous_refs = previous
        .interactive_elements
        .iter()
        .map(|element| element.ref_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let next_refs = next
        .interactive_elements
        .iter()
        .map(|element| element.ref_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let previous_text = previous
        .text_lines
        .iter()
        .map(String::as_str)
        .collect::<std::collections::HashSet<_>>();
    json!({
        "fromRevision": previous.revision,
        "toRevision": next.revision,
        "urlChanged": previous.url != next.url,
        "titleChanged": previous.title != next.title,
        "newElements": next.interactive_elements.iter().filter(|element| !previous_refs.contains(element.ref_id.as_str())).take(10).map(|element| json!({"ref": element.ref_id, "role": element.role, "name": element.name})).collect::<Vec<_>>(),
        "removedRefs": previous.interactive_elements.iter().filter(|element| !next_refs.contains(element.ref_id.as_str())).take(10).map(|element| Value::String(element.ref_id.clone())).collect::<Vec<_>>(),
        "newText": next.text_lines.iter().filter(|line| !previous_text.contains(line.as_str())).take(10).cloned().collect::<Vec<_>>(),
        "alerts": next.text_lines.iter().filter(|line| line.to_ascii_lowercase().contains("error") || line.to_ascii_lowercase().contains("failed") || line.to_ascii_lowercase().contains("invalid") || line.to_ascii_lowercase().contains("warning")).take(5).cloned().collect::<Vec<_>>(),
    })
}

fn interactive_element_value(element: &InteractiveElement) -> Value {
    json!({
        "ref": element.ref_id,
        "pageRevision": element.page_revision,
        "role": element.role,
        "name": element.name,
        "text": element.text,
        "visible": element.visible,
        "enabled": element.enabled,
        "locatorHint": {"tag": element.tag, "testId": element.test_id},
        "fingerprint": element.fingerprint,
    })
}

fn network_event_value(event: NetworkEvent) -> Value {
    json!({
        "id": event.id,
        "method": event.method,
        "url": event.url,
        "status": event.status,
        "contentType": event.content_type,
        "resourceType": event.resource_type,
        "timestamp": event.timestamp,
        "ok": event.ok,
        "errorText": event.error_text,
        "durationMs": event.duration_ms,
    })
}

fn env_non_empty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn resolve_headless_option(cli_value: Option<String>) -> bool {
    cli_value
        .or_else(|| env_non_empty("BROWSER_MCP_HEADLESS"))
        .map(|value| value != "false")
        .unwrap_or(true)
}

fn opt_string_value(value: Option<String>) -> Value {
    value.map(Value::String).unwrap_or(Value::Null)
}

fn base_attached_runtime_diagnostics(configured_source: &ConfiguredAttachSource) -> Value {
    json!({
        "status": "not_configured",
        "descriptorSource": configured_source.source,
        "descriptorPath": configured_source.path,
        "inputArtifactKind": null,
        "schemaVersion": null,
        "attachMode": null,
        "artifactBackendFamily": null,
        "recommendedEntrypoint": null,
        "sourceTransportMethod": null,
        "sourceHandoffMethod": null,
        "traceStreamPath": null,
        "bindingArtifactSource": null,
        "handoffSource": null,
        "resumeManifestSource": null,
        "traceStreamSource": null,
        "replaySupported": false,
        "eventCount": 0,
        "latestEventId": null,
        "latestEventKind": null,
        "latestEventTimestamp": null,
        "warning": null,
    })
}

fn attached_runtime_replay_context(diagnostics: &Value) -> Value {
    json!({
        "descriptorSource": diagnostics.get("descriptorSource").cloned().unwrap_or(Value::Null),
        "descriptorPath": diagnostics.get("descriptorPath").cloned().unwrap_or(Value::Null),
        "inputArtifactKind": diagnostics.get("inputArtifactKind").cloned().unwrap_or(Value::Null),
        "attachMode": diagnostics.get("attachMode").cloned().unwrap_or(Value::Null),
        "artifactBackendFamily": diagnostics.get("artifactBackendFamily").cloned().unwrap_or(Value::Null),
        "recommendedEntrypoint": diagnostics.get("recommendedEntrypoint").cloned().unwrap_or(Value::Null),
        "sourceTransportMethod": diagnostics.get("sourceTransportMethod").cloned().unwrap_or(Value::Null),
        "sourceHandoffMethod": diagnostics.get("sourceHandoffMethod").cloned().unwrap_or(Value::Null),
        "traceStreamPath": diagnostics.get("traceStreamPath").cloned().unwrap_or(Value::Null),
        "bindingArtifactSource": diagnostics.get("bindingArtifactSource").cloned().unwrap_or(Value::Null),
        "handoffSource": diagnostics.get("handoffSource").cloned().unwrap_or(Value::Null),
        "resumeManifestSource": diagnostics.get("resumeManifestSource").cloned().unwrap_or(Value::Null),
        "traceStreamSource": diagnostics.get("traceStreamSource").cloned().unwrap_or(Value::Null),
    })
}

fn descriptor_leaf<'a>(descriptor: &'a Value, path_parts: &[&str]) -> Option<&'a Value> {
    let mut current = descriptor;
    for part in path_parts {
        current = current.get(*part)?;
    }
    Some(current)
}

fn descriptor_string(descriptor: &Value, path_parts: &[&str]) -> Option<String> {
    descriptor_leaf(descriptor, path_parts)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn descriptor_bool(descriptor: &Value, path_parts: &[&str]) -> Option<bool> {
    descriptor_leaf(descriptor, path_parts).and_then(Value::as_bool)
}

fn descriptor_resolved_artifact(descriptor: &Value, field: &str) -> Option<String> {
    descriptor_string(descriptor, &["resolved_artifacts", field])
        .or_else(|| descriptor_string(descriptor, &[field]))
}

fn normalize_runtime_locator_for_existing_file(locator: &str) -> String {
    let path = PathBuf::from(locator);
    if path.exists() {
        return path.to_string_lossy().into_owned();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(&path))
        .ok()
        .filter(|candidate| candidate.exists())
        .map(|candidate| candidate.to_string_lossy().into_owned())
        .unwrap_or_else(|| locator.to_string())
}

fn normalized_descriptor_value(value: Option<&Value>, path_like: bool) -> Option<String> {
    let value = value?;
    if path_like {
        return value.as_str().filter(|item| !item.is_empty()).map(|item| {
            let path = PathBuf::from(item);
            if path.is_absolute() {
                path
            } else {
                std::env::current_dir()
                    .map(|cwd| cwd.join(path))
                    .unwrap_or_else(|_| PathBuf::from(item))
            }
            .to_string_lossy()
            .into_owned()
        });
    }
    Some(match value {
        Value::String(item) => item.clone(),
        Value::Bool(item) => item.to_string(),
        Value::Number(item) => item.to_string(),
        Value::Null => "null".to_string(),
        other => other.to_string(),
    })
}

fn assert_attach_descriptor_leaf_matches_canonical(
    original: &Value,
    canonical: &Value,
    path_parts: &[&str],
    path_like: bool,
) -> Result<(), String> {
    let Some(requested) = descriptor_leaf(original, path_parts) else {
        return Ok(());
    };
    if requested.is_null() {
        return Ok(());
    }
    let resolved = descriptor_leaf(canonical, path_parts).ok_or_else(|| {
        format!(
            "runtime attach descriptor must already carry canonical {}",
            path_parts.join(".")
        )
    })?;
    if normalized_descriptor_value(Some(requested), path_like)
        != normalized_descriptor_value(Some(resolved), path_like)
    {
        return Err(format!(
            "runtime attach descriptor must already match canonical {}",
            path_parts.join(".")
        ));
    }
    Ok(())
}

fn assert_attach_descriptor_matches_canonical(
    original: &Value,
    canonical: &Value,
) -> Result<(), String> {
    for field in [
        ["requested_artifacts", "binding_artifact_path"],
        ["requested_artifacts", "handoff_path"],
        ["requested_artifacts", "resume_manifest_path"],
        ["resolved_artifacts", "binding_artifact_path"],
        ["resolved_artifacts", "handoff_path"],
        ["resolved_artifacts", "resume_manifest_path"],
        ["resolved_artifacts", "trace_stream_path"],
    ] {
        assert_attach_descriptor_leaf_matches_canonical(original, canonical, &field, true)?;
    }
    for field in [
        &["attach_mode"][..],
        &["artifact_backend_family"][..],
        &["source_transport_method"][..],
        &["source_handoff_method"][..],
        &["attach_method"][..],
        &["subscribe_method"][..],
        &["cleanup_method"][..],
        &["resume_mode"][..],
        &["cleanup_semantics"][..],
        &["recommended_entrypoint"][..],
        &["attach_capabilities", "artifact_replay"][..],
        &["attach_capabilities", "live_remote_stream"][..],
        &["attach_capabilities", "cleanup_preserves_replay"][..],
        &["resolution", "binding_artifact_path"][..],
        &["resolution", "handoff_path"][..],
        &["resolution", "resume_manifest_path"][..],
        &["resolution", "trace_stream_path"][..],
    ] {
        assert_attach_descriptor_leaf_matches_canonical(original, canonical, field, false)?;
    }
    Ok(())
}

fn assert_attach_descriptor_contract(descriptor: &Value) -> Result<(), String> {
    for (field, expected) in [
        ("attach_mode", RUNTIME_ATTACH_MODE),
        (
            "source_transport_method",
            RUNTIME_ATTACH_SOURCE_TRANSPORT_METHOD,
        ),
        (
            "source_handoff_method",
            RUNTIME_ATTACH_SOURCE_HANDOFF_METHOD,
        ),
        ("attach_method", RUNTIME_ATTACH_METHOD),
        ("subscribe_method", RUNTIME_ATTACH_SUBSCRIBE_METHOD),
        ("cleanup_method", RUNTIME_ATTACH_CLEANUP_METHOD),
        ("resume_mode", RUNTIME_ATTACH_RESUME_MODE),
    ] {
        if let Some(value) = descriptor_string(descriptor, &[field]) {
            if value != expected {
                return Err(format!(
                    "runtime attach descriptor must use {field}={expected}"
                ));
            }
        }
    }
    if let Some(value) = descriptor_bool(descriptor, &["attach_capabilities", "artifact_replay"]) {
        if !value {
            return Err(
                "runtime attach descriptor must advertise attach_capabilities.artifact_replay=true"
                    .to_string(),
            );
        }
    }
    if let Some(value) = descriptor_bool(
        descriptor,
        &["attach_capabilities", "cleanup_preserves_replay"],
    ) {
        if !value {
            return Err(
                "runtime attach descriptor must advertise attach_capabilities.cleanup_preserves_replay=true"
                    .to_string(),
            );
        }
    }
    if let Some(value) = descriptor_bool(descriptor, &["attach_capabilities", "live_remote_stream"])
    {
        if value {
            return Err(
                "runtime attach descriptor must advertise attach_capabilities.live_remote_stream=false"
                    .to_string(),
            );
        }
    }
    Ok(())
}

fn attach_descriptor_needs_rust_hydration(descriptor: &Value) -> bool {
    [
        ["requested_artifacts", "binding_artifact_path"],
        ["requested_artifacts", "handoff_path"],
        ["requested_artifacts", "resume_manifest_path"],
        ["resolved_artifacts", "binding_artifact_path"],
        ["resolved_artifacts", "handoff_path"],
        ["resolved_artifacts", "resume_manifest_path"],
    ]
    .iter()
    .any(|path_parts| {
        descriptor_string(descriptor, path_parts)
            .map(|value| !value.is_empty())
            .unwrap_or(false)
    })
}

fn collect_attach_artifact_candidates(root: &Path, candidates: &mut Vec<AttachArtifactCandidate>) {
    if !root.exists() {
        return;
    }
    collect_filesystem_attach_candidates(root, candidates);
    collect_sqlite_attach_candidates(root, candidates);
}

fn default_attach_discovery_roots(repo_root: &Path) -> Vec<PathBuf> {
    let mut roots = vec![
        repo_root.join("artifacts").join("scratch"),
        repo_root.join("artifacts").join("current"),
    ];
    if std::env::var("BROWSER_MCP_DISCOVER_REPO_ROOT")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        roots.push(repo_root.to_path_buf());
    }
    roots
}

fn select_attach_artifact_candidate(roots: Vec<PathBuf>) -> Option<String> {
    let mut candidates = Vec::new();
    for root in roots {
        collect_attach_artifact_candidates(&root, &mut candidates);
    }
    candidates.sort_by(|left, right| {
        right
            .rank
            .cmp(&left.rank)
            .then_with(|| right.path.cmp(&left.path))
    });
    candidates
        .into_iter()
        .next()
        .map(|candidate| candidate.path)
}

fn collect_filesystem_attach_candidates(
    root: &Path,
    candidates: &mut Vec<AttachArtifactCandidate>,
) {
    collect_filesystem_attach_candidates_with_depth(root, candidates, 0);
}

fn collect_filesystem_attach_candidates_with_depth(
    root: &Path,
    candidates: &mut Vec<AttachArtifactCandidate>,
    depth: usize,
) {
    const MAX_DISCOVERY_DEPTH: usize = 8;
    if depth > MAX_DISCOVERY_DEPTH {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if should_skip_attach_discovery_dir(&path) {
                continue;
            }
            collect_filesystem_attach_candidates_with_depth(&path, candidates, depth + 1);
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let in_transport_dir = path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            == Some("runtime_event_transports");
        if file_name != "TRACE_RESUME_MANIFEST.json" && !in_transport_dir {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(payload) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let recency_ms = path
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0);
        if file_name == "TRACE_RESUME_MANIFEST.json" {
            if let Some(candidate) =
                manifest_attach_candidate(&payload, path.to_string_lossy().into_owned(), recency_ms)
            {
                candidates.push(candidate);
            }
        } else if let Some(candidate) =
            binding_attach_candidate(&payload, path.to_string_lossy().into_owned(), recency_ms)
        {
            candidates.push(candidate);
        }
    }
}

fn collect_sqlite_attach_candidates(root: &Path, candidates: &mut Vec<AttachArtifactCandidate>) {
    collect_sqlite_attach_candidates_with_depth(root, candidates, 0);
}

fn collect_sqlite_attach_candidates_with_depth(
    root: &Path,
    candidates: &mut Vec<AttachArtifactCandidate>,
    depth: usize,
) {
    const MAX_DISCOVERY_DEPTH: usize = 8;
    if depth > MAX_DISCOVERY_DEPTH {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if should_skip_attach_discovery_dir(&path) {
                continue;
            }
            collect_sqlite_attach_candidates_with_depth(&path, candidates, depth + 1);
            continue;
        }
        if !file_type.is_file()
            || path.file_name().and_then(|name| name.to_str())
                != Some("runtime_checkpoint_store.sqlite3")
        {
            continue;
        }
        let recency_ms = path
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0);
        append_sqlite_attach_candidates(&path, recency_ms, candidates);
    }
}

fn should_skip_attach_discovery_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    matches!(
        name,
        ".git"
            | ".cursor"
            | "node_modules"
            | "target"
            | ".venv"
            | "venv"
            | "__pycache__"
            | ".next"
            | ".idea"
            | ".vscode"
    )
}

fn append_sqlite_attach_candidates(
    db_path: &Path,
    recency_ms: i64,
    candidates: &mut Vec<AttachArtifactCandidate>,
) {
    let Ok(conn) = Connection::open(db_path) else {
        return;
    };
    let Ok(mut stmt) = conn.prepare(
        "SELECT rowid, payload_key, payload_text FROM runtime_storage_payloads \
         WHERE payload_key LIKE '%TRACE_RESUME_MANIFEST.json' \
            OR payload_key LIKE '%runtime_event_transports/%.json'",
    ) else {
        return;
    };
    let Ok(rows) = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    }) else {
        return;
    };
    for row in rows.filter_map(Result::ok) {
        let (row_id, payload_key, payload_text) = row;
        let Ok(payload) = serde_json::from_str::<Value>(&payload_text) else {
            continue;
        };
        let row_recency = recency_ms.saturating_add(row_id);
        let attach_path = sqlite_payload_locator(db_path, &payload_key);
        if payload_key.ends_with("TRACE_RESUME_MANIFEST.json") {
            if let Some(candidate) = manifest_attach_candidate(&payload, attach_path, row_recency) {
                candidates.push(candidate);
            }
        } else if let Some(candidate) = binding_attach_candidate(
            &sqlite_rooted_binding_payload(db_path, payload),
            attach_path,
            row_recency,
        ) {
            candidates.push(candidate);
        }
    }
}

fn sqlite_payload_locator(db_path: &Path, payload_key: &str) -> String {
    let path = PathBuf::from(payload_key);
    if path.is_absolute() {
        return path.to_string_lossy().into_owned();
    }
    db_path
        .parent()
        .map(|parent| parent.join(&path))
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn sqlite_rooted_binding_payload(db_path: &Path, mut payload: Value) -> Value {
    let Some(binding_path) = descriptor_string(&payload, &["binding_artifact_path"]) else {
        return payload;
    };
    if PathBuf::from(&binding_path).is_absolute() {
        return payload;
    }
    if let Some(map) = payload.as_object_mut() {
        map.insert(
            "binding_artifact_path".to_string(),
            Value::String(sqlite_payload_locator(db_path, &binding_path)),
        );
    }
    payload
}

fn manifest_attach_candidate(
    payload: &Value,
    attach_path: String,
    recency_ms: i64,
) -> Option<AttachArtifactCandidate> {
    if descriptor_string(payload, &["schema_version"]).as_deref()
        != Some(TRACE_RESUME_MANIFEST_SCHEMA_VERSION)
    {
        return None;
    }
    descriptor_string(payload, &["event_transport_path"])?;
    Some(AttachArtifactCandidate {
        path: attach_path,
        rank: AttachArtifactCandidateRank {
            updated_at_ms: descriptor_string(payload, &["updated_at"])
                .as_deref()
                .and_then(parse_rfc3339_millis)
                .unwrap_or(0),
            recency_ms,
            source_priority: 1,
        },
    })
}

fn binding_attach_candidate(
    payload: &Value,
    fallback_attach_path: String,
    recency_ms: i64,
) -> Option<AttachArtifactCandidate> {
    if descriptor_string(payload, &["schema_version"]).as_deref()
        != Some(RUNTIME_EVENT_TRANSPORT_SCHEMA_VERSION)
    {
        return None;
    }
    if descriptor_string(payload, &["binding_backend_family"]).as_deref() == Some("filesystem") {
        return None;
    }
    let path = descriptor_string(payload, &["binding_artifact_path"])
        .filter(|path| !path.is_empty())
        .unwrap_or(fallback_attach_path);
    Some(AttachArtifactCandidate {
        path,
        rank: AttachArtifactCandidateRank {
            updated_at_ms: 0,
            recency_ms,
            source_priority: 0,
        },
    })
}

fn parse_rfc3339_millis(value: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|datetime| datetime.timestamp_millis())
}

fn compact_summary(summary: &Value, text_budget: usize) -> Value {
    json!({
        "mainGoalArea": truncate_text(value_str(summary.get("mainGoalArea")), text_budget),
        "visibleMessages": summary.get("visibleMessages").and_then(Value::as_array).cloned().unwrap_or_default().into_iter().map(|value| Value::String(truncate_text(&value_string(Some(&value)), text_budget.min(200)))).collect::<Vec<_>>(),
        "forms": summary.get("forms").and_then(Value::as_u64).unwrap_or(0),
        "dialogs": summary.get("dialogs").and_then(Value::as_u64).unwrap_or(0),
    })
}

fn browser_error(
    code: &str,
    message: &str,
    suggested_next_actions: &[&str],
    recoverable: bool,
) -> Value {
    json!({
        "code": code,
        "message": message,
        "recoverable": recoverable,
        "suggested_next_actions": suggested_next_actions,
    })
}

fn skill_error(code: &str, message: &str) -> Value {
    browser_error(
        code,
        message,
        &[
            "ensure the MCP server was started with --repo-root pointing at the repository root",
            "ensure skills/SKILL_ROUTING_RUNTIME.json and skills/SKILL_MANIFEST.json are generated",
        ],
        true,
    )
}

fn runtime_error(code: &str, message: &str) -> Value {
    browser_error(
        code,
        message,
        &[
            "inspect browser_diagnostics",
            "verify runtime state paths and operation inputs",
        ],
        true,
    )
}

fn skill_runtime_path(repo_root: &Path) -> PathBuf {
    repo_root.join("skills/SKILL_ROUTING_RUNTIME.json")
}

fn skill_manifest_path(repo_root: &Path) -> PathBuf {
    repo_root.join("skills/SKILL_MANIFEST.json")
}

fn skill_runtime_available(repo_root: &Path) -> bool {
    skill_runtime_path(repo_root).is_file() && repo_root.join("skills").is_dir()
}

fn skill_body_path(repo_root: &Path, slug: &str) -> Result<PathBuf, String> {
    let clean = slug.trim();
    if clean.is_empty()
        || clean.contains('/')
        || clean.contains('\\')
        || clean.contains("..")
        || clean.starts_with('.')
    {
        return Err(format!("invalid skill slug: {slug}"));
    }

    let manifest_path = skill_manifest_path(repo_root);
    if manifest_path.is_file() {
        if let Some(path) = skill_body_path_from_manifest(repo_root, &manifest_path, clean)? {
            return Ok(path);
        }
    }

    let path = repo_root.join("skills").join(clean).join("SKILL.md");
    if !path.is_file() {
        return Err(format!("skill body not found: {}", path.display()));
    }
    Ok(path)
}

fn skill_body_path_from_manifest(
    repo_root: &Path,
    manifest_path: &Path,
    slug: &str,
) -> Result<Option<PathBuf>, String> {
    let payload = crate::route::read_json(manifest_path)?;
    let keys = payload
        .get("keys")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("manifest missing keys: {}", manifest_path.display()))?;
    let key_index = keys
        .iter()
        .enumerate()
        .filter_map(|(idx, key)| key.as_str().map(|raw| (raw.to_string(), idx)))
        .collect::<std::collections::HashMap<_, _>>();
    let idx_slug = *key_index
        .get("slug")
        .ok_or_else(|| format!("manifest missing slug key: {}", manifest_path.display()))?;
    let Some(idx_skill_path) = key_index.get("skill_path").copied() else {
        return Ok(None);
    };
    let rows = payload
        .get("skills")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("manifest missing skills rows: {}", manifest_path.display()))?;
    for row in rows.iter().filter_map(Value::as_array) {
        if row.get(idx_slug).and_then(Value::as_str) != Some(slug) {
            continue;
        }
        let Some(skill_path) = row.get(idx_skill_path).and_then(Value::as_str) else {
            continue;
        };
        if skill_path.starts_with('/')
            || skill_path.contains("..")
            || !skill_path.ends_with("SKILL.md")
        {
            return Err(format!("invalid skill_path for {slug}: {skill_path}"));
        }
        let path = repo_root.join(skill_path);
        if !path.is_file() {
            return Err(format!("skill body not found: {}", path.display()));
        }
        return Ok(Some(path));
    }
    Ok(None)
}

fn route_with_full_manifest_fallback(
    runtime_records: &[SkillRecord],
    manifest_path: &Path,
    query: &str,
    session_id: &str,
    allow_overlay: bool,
    first_turn: bool,
) -> Result<RouteDecision, String> {
    let hot_decision = route_task(
        runtime_records,
        query,
        session_id,
        allow_overlay,
        first_turn,
    )?;
    if !manifest_path.is_file() {
        return Ok(hot_decision);
    }
    let should_retry = should_retry_with_manifest(&hot_decision);
    let full_records = load_records_from_manifest(manifest_path)?;
    let full_decision = route_task(&full_records, query, session_id, allow_overlay, first_turn)?;
    if should_accept_manifest_fallback(
        &hot_decision,
        &full_decision,
        runtime_records,
        should_retry,
        false,
    ) {
        Ok(full_decision)
    } else {
        Ok(hot_decision)
    }
}

fn session_not_found_error() -> Value {
    browser_error(
        "SESSION_NOT_FOUND",
        "No active browser session exists.",
        &["call browser_open"],
        true,
    )
}

fn success_response(request_id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": request_id, "result": result})
}

fn error_response(request_id: Value, error: Value) -> Value {
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("Browser MCP server error");
    json!({"jsonrpc": "2.0", "id": request_id, "error": {"code": -32000, "message": message, "data": error}})
}

fn require_string(payload: &Value, key: &str) -> Result<String, Value> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            browser_error(
                "INVALID_INPUT",
                &format!("Missing required string field '{key}'"),
                &[&format!("provide a non-empty string for '{key}'")],
                true,
            )
        })
}

fn required_string_arg(payload: &Value, key: &str) -> Result<String, Value> {
    require_string(payload, key)
}

fn optional_string(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn optional_bool(payload: &Value, key: &str) -> Option<bool> {
    payload.get(key).and_then(Value::as_bool)
}

fn optional_u64(payload: &Value, key: &str) -> Result<Option<u64>, Value> {
    match payload.get(key) {
        None => Ok(None),
        Some(Value::Number(number)) => number.as_u64().map(Some).ok_or_else(|| {
            browser_error(
                "INVALID_INPUT",
                &format!("Expected unsigned integer for '{key}'"),
                &[&format!("pass '{key}' as an unsigned integer")],
                true,
            )
        }),
        Some(other) => Err(browser_error(
            "INVALID_INPUT",
            &format!(
                "Expected integer for '{key}', got {}",
                json_type_name(other)
            ),
            &[&format!("pass '{key}' as an integer")],
            true,
        )),
    }
}

fn optional_usize(payload: &Value, key: &str, default: usize) -> Result<usize, Value> {
    optional_u64(payload, key).map(|value| value.unwrap_or(default as u64) as usize)
}

fn optional_string_array(payload: &Value, key: &str) -> Option<Vec<String>> {
    payload.get(key).and_then(Value::as_array).map(|items| {
        items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect::<Vec<_>>()
    })
}

fn value_str(value: Option<&Value>) -> &str {
    value.and_then(Value::as_str).unwrap_or("")
}

fn value_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| value_string(Some(item)))
            .collect::<Vec<_>>()
            .join(" "),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::Number(_) => "int",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
    }
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut output = text
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    output.push_str("...");
    output
}

fn to_text_lines(text: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| seen.insert((*line).to_string()))
        .take(50)
        .map(|line| truncate_text(line, 240))
        .collect()
}

fn current_local_timestamp() -> String {
    Local::now()
        .to_rfc3339_opts(SecondsFormat::Secs, false)
        .to_string()
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn cdp_key_name(key: &str) -> String {
    match key {
        "Return" => "Enter".to_string(),
        other => other.to_string(),
    }
}

fn json_string_literal(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn decode_base64(input: &str) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u8;
    for ch in input.bytes() {
        let value = match ch {
            b'A'..=b'Z' => ch - b'A',
            b'a'..=b'z' => ch - b'a' + 26,
            b'0'..=b'9' => ch - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b'\r' | b'\n' | b'\t' | b' ' => continue,
            other => return Err(format!("invalid base64 byte {other}")),
        } as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push(((buffer >> bits) & 0xff) as u8);
        }
    }
    Ok(output)
}
