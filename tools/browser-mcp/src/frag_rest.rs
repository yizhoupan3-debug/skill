// CDP/Chrome 助手、Attach 候选、skill 路由与 MCP 收尾工具函数。
use std::net::TcpListener;
use std::sync::LazyLock;
use framework_kernel::json_value::optional_bool;

/// Shared HTTP client for CDP requests with a 30-second timeout.
static CDP_HTTP_CLIENT: LazyLock<reqwest::blocking::Client> = LazyLock::new(|| {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("CDP HTTP client")
});

/// RAII guard: kills a Chrome child process and removes its temp dir on drop (unless consumed).
struct CleanupGuard<'a> {
    child: Option<Child>,
    user_data_dir: &'a Path,
}

impl Drop for CleanupGuard<'_> {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_dir_all(self.user_data_dir);
    }
}

/// 保留目录中最新的 N 个 `.png` 文件，删除更旧的（尽力而为，不传播错误）。
fn purge_old_screenshots(dir: &Path, keep: usize) {
    let Ok(mut entries) = fs::read_dir(dir)
        .map(|iter| iter.filter_map(Result::ok).collect::<Vec<_>>())
    else {
        return;
    };
    if entries.len() <= keep {
        return;
    }
    entries.sort_by_key(|e| e.path().metadata().and_then(|m| m.modified()).ok());
    let remove_count = entries.len() - keep;
    for entry in entries.into_iter().take(remove_count) {
        let _ = fs::remove_file(entry.path());
    }
}

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
    let url = format!("http://127.0.0.1:{port}{path}");
    CDP_HTTP_CLIENT
        .get(&url)
        .send()
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

fn allocate_debug_port() -> Result<(u16, TcpListener), Value> {
    match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => {
            let port = listener.local_addr().map(|addr| addr.port()).unwrap_or(9222);
            Ok((port, listener))
        }
        Err(err) => Err(browser_error(
            "BROWSER_LAUNCH_FAILED",
            &format!("Failed to allocate debug port: {err}"),
            &["check system network resources"],
            false,
        )),
    }
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
}).slice(0, 800);
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
    // 单 HashSet + any：任一侧出现另一侧没有的 fingerprint 即认为有变化
    let prev_fps: std::collections::HashSet<&str> = previous
        .interactive_elements
        .iter()
        .map(|e| e.fingerprint.as_str())
        .collect();
    next.interactive_elements
        .iter()
        .any(|e| !prev_fps.contains(e.fingerprint.as_str()))
        || {
            let next_fps: std::collections::HashSet<&str> = next
                .interactive_elements
                .iter()
                .map(|e| e.fingerprint.as_str())
                .collect();
            previous
                .interactive_elements
                .iter()
                .any(|e| !next_fps.contains(e.fingerprint.as_str()))
        }
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
    // newText：只在文本实际变化时才遍历，避免无变化时白建 HashSet
    let new_text: Vec<Value> = if previous.text_content != next.text_content {
        let prev_lines: std::collections::HashSet<&str> = previous
            .text_lines
            .iter()
            .map(String::as_str)
            .collect();
        next.text_lines
            .iter()
            .filter(|line| !prev_lines.contains(line.as_str()))
            .take(10)
            .cloned()
            .map(Value::String)
            .collect()
    } else {
        Vec::new()
    };
    json!({
        "fromRevision": previous.revision,
        "toRevision": next.revision,
        "urlChanged": previous.url != next.url,
        "titleChanged": previous.title != next.title,
        "newElements": next.interactive_elements.iter().filter(|element| !previous_refs.contains(element.ref_id.as_str())).take(10).map(|element| json!({"ref": element.ref_id, "role": element.role, "name": element.name})).collect::<Vec<_>>(),
        "removedRefs": previous.interactive_elements.iter().filter(|element| !next_refs.contains(element.ref_id.as_str())).take(10).map(|element| Value::String(element.ref_id.clone())).collect::<Vec<_>>(),
        "newText": new_text,
        "alerts": next.text_lines.iter().filter(|line| line.to_ascii_lowercase().contains("error") || line.to_ascii_lowercase().contains("failed") || line.to_ascii_lowercase().contains("invalid") || line.to_ascii_lowercase().contains("warning")).take(5).cloned().map(Value::String).collect::<Vec<_>>(),
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

fn network_event_value(event: &NetworkEvent) -> Value {
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
        if let Some(value) = descriptor_string(descriptor, &[field])
            && value != expected {
                return Err(format!(
                    "runtime attach descriptor must use {field}={expected}"
                ));
            }
    }
    if let Some(value) = descriptor_bool(descriptor, &["attach_capabilities", "artifact_replay"])
        && !value {
            return Err(
                "runtime attach descriptor must advertise attach_capabilities.artifact_replay=true"
                    .to_string(),
            );
        }
    if let Some(value) = descriptor_bool(
        descriptor,
        &["attach_capabilities", "cleanup_preserves_replay"],
    )
        && !value {
            return Err(
                "runtime attach descriptor must advertise attach_capabilities.cleanup_preserves_replay=true"
                    .to_string(),
            );
        }
    if let Some(value) = descriptor_bool(descriptor, &["attach_capabilities", "live_remote_stream"])
        && value {
            return Err(
                "runtime attach descriptor must advertise attach_capabilities.live_remote_stream=false"
                    .to_string(),
            );
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
    // 快速路径：对 ASCII 文本直接用字节长度判断，O(1)
    if text.len() <= max_chars {
        return text.to_string();
    }
    if max_chars < 2 {
        return "...".to_string();
    }
    // 多字节文本：只扫描到 max_chars-1 处即停止，O(max_chars) 而非 O(n)
    // 保留 max_chars-1 个原始字符 + "..." 后缀
    let end = text
        .char_indices()
        .nth(max_chars - 2)
        .map(|(pos, ch)| pos + ch.len_utf8())
        .unwrap_or(0);
    let mut output = String::with_capacity(end + 3);
    output.push_str(&text[..end]);
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
    use base64::Engine as _;
    let engine = base64::engine::general_purpose::STANDARD;
    // Fast path: no whitespace (common for screenshots from Chrome CDP)
    if input.bytes().any(|b| b.is_ascii_whitespace()) {
        let cleaned: String = input.chars().filter(|c| !c.is_ascii_whitespace()).collect();
        engine.decode(cleaned.as_bytes()).map_err(|e| format!("decode base64 failed: {e}"))
    } else {
        engine.decode(input.as_bytes()).map_err(|e| format!("decode base64 failed: {e}"))
    }
}

#[cfg(test)]
mod frag_rest_tests {
    use super::*;

    // --- fingerprint ---

    #[test]
    fn create_fingerprint_prefers_test_id() {
        let mut counts = HashMap::new();
        let desc = ElementDescriptor {
            role: "button".to_string(),
            name: "Submit".to_string(),
            text: "Submit".to_string(),
            visible: true,
            enabled: true,
            tag: "button".to_string(),
            test_id: Some("submit-btn".to_string()),
            selector: "button".to_string(),
        };
        assert_eq!(create_fingerprint(&desc, &mut counts), "tid::submit-btn");
    }

    #[test]
    fn create_fingerprint_deduplicates_by_counter() {
        let mut counts = HashMap::new();
        let desc = ElementDescriptor {
            role: "link".to_string(),
            name: "Home".to_string(),
            text: "Home".to_string(),
            visible: true,
            enabled: true,
            tag: "a".to_string(),
            test_id: None,
            selector: "a".to_string(),
        };
        assert_eq!(create_fingerprint(&desc, &mut counts), "link::Home::a");
        let desc2 = ElementDescriptor {
            ..desc.clone()
        };
        assert_eq!(create_fingerprint(&desc2, &mut counts), "link::Home::a");
        let desc3 = ElementDescriptor {
            ..desc.clone()
        };
        assert_eq!(create_fingerprint(&desc3, &mut counts), "link::Home::a");
    }

    // --- has_meaningful_change ---

    fn make_snapshot(url: &str, title: &str, text: &str, elements: Vec<InteractiveElement>) -> PageSnapshot {
        make_snapshot_with_revision(1, url, title, text, elements)
    }

    fn make_snapshot_with_revision(revision: u64, url: &str, title: &str, text: &str, elements: Vec<InteractiveElement>) -> PageSnapshot {
        PageSnapshot {
            revision,
            url: url.to_string(),
            title: title.to_string(),
            loading_state: "complete".to_string(),
            summary: json!({}),
            interactive_elements: elements,
            text_content: text.to_string(),
            text_lines: text.lines().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn meaningful_change_detects_url_diff() {
        let a = make_snapshot("http://a.com", "A", "text", vec![]);
        let b = make_snapshot("http://b.com", "A", "text", vec![]);
        assert!(has_meaningful_change(&a, &b));
    }

    #[test]
    fn meaningful_change_detects_text_diff() {
        let a = make_snapshot("http://a.com", "A", "old text", vec![]);
        let b = make_snapshot("http://a.com", "A", "new text", vec![]);
        assert!(has_meaningful_change(&a, &b));
    }

    #[test]
    fn meaningful_change_false_for_identical_snapshots() {
        let a = make_snapshot("http://a.com", "A", "text", vec![]);
        let b = make_snapshot("http://a.com", "A", "text", vec![]);
        assert!(!has_meaningful_change(&a, &b));
    }

    #[test]
    fn meaningful_change_detects_element_fingerprint_diff() {
        let elem = InteractiveElement {
            ref_id: "ref_1".to_string(),
            page_revision: 1,
            role: "button".to_string(),
            name: "Go".to_string(),
            text: "Go".to_string(),
            visible: true,
            enabled: true,
            tag: "button".to_string(),
            test_id: None,
            fingerprint: "fp-a".to_string(),
            selector: "button".to_string(),
        };
        let mut elem2 = elem.clone();
        elem2.fingerprint = "fp-b".to_string();
        let a = make_snapshot("http://a.com", "A", "text", vec![elem]);
        let b = make_snapshot("http://a.com", "A", "text", vec![elem2]);
        assert!(has_meaningful_change(&a, &b));
    }

    // --- compute_delta ---

    #[test]
    fn delta_identifies_new_and_removed_elements() {
        let elem1 = InteractiveElement {
            ref_id: "ref_1".to_string(),
            page_revision: 1,
            role: "button".to_string(),
            name: "Old".to_string(),
            text: "Old".to_string(),
            visible: true,
            enabled: true,
            tag: "button".to_string(),
            test_id: None,
            fingerprint: "fp-1".to_string(),
            selector: "button".to_string(),
        };
        let elem2 = InteractiveElement {
            ref_id: "ref_2".to_string(),
            page_revision: 2,
            role: "link".to_string(),
            name: "New".to_string(),
            text: "New".to_string(),
            visible: true,
            enabled: true,
            tag: "a".to_string(),
            test_id: None,
            fingerprint: "fp-2".to_string(),
            selector: "a".to_string(),
        };
        let prev = make_snapshot_with_revision(1, "http://x.com", "T", "text", vec![elem1]);
        let next = make_snapshot_with_revision(2, "http://x.com", "T", "text", vec![elem2]);
        let delta = compute_delta(&prev, &next);
        assert_eq!(delta["fromRevision"], json!(1));
        assert_eq!(delta["toRevision"], json!(2));
        assert_eq!(delta["urlChanged"], false);
        let new_elems = delta["newElements"].as_array().unwrap();
        assert_eq!(new_elems.len(), 1);
        assert_eq!(new_elems[0]["ref"], "ref_2");
        let removed = delta["removedRefs"].as_array().unwrap();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0], "ref_1");
    }

    // --- browser_error ---

    #[test]
    fn browser_error_structure() {
        let err = browser_error("CODE", "msg", &["action1"], true);
        assert_eq!(err["code"], "CODE");
        assert_eq!(err["message"], "msg");
        assert_eq!(err["recoverable"], true);
        assert_eq!(err["suggested_next_actions"][0], "action1");
    }

    #[test]
    fn runtime_error_wraps_browser_error() {
        let err = runtime_error("RT_ERR", "bad state");
        assert_eq!(err["code"], "RT_ERR");
        assert_eq!(err["recoverable"], true);
    }

    // --- require_string / optional helpers ---

    #[test]
    fn require_string_valid_input() {
        let payload = json!({"key": "value"});
        assert_eq!(require_string(&payload, "key").unwrap(), "value");
    }

    #[test]
    fn require_string_empty_is_error() {
        let payload = json!({"key": "  "});
        assert!(require_string(&payload, "key").is_err());
    }

    #[test]
    fn require_string_missing_is_error() {
        let payload = json!({});
        assert!(require_string(&payload, "key").is_err());
    }

    #[test]
    fn optional_string_returns_trimmed() {
        let payload = json!({"k": " hello "});
        assert_eq!(optional_string(&payload, "k"), Some("hello".to_string()));
    }

    #[test]
    fn optional_string_returns_none_for_empty() {
        let payload = json!({"k": "  "});
        assert_eq!(optional_string(&payload, "k"), None);
    }

    #[test]
    fn optional_bool_returns_value() {
        let payload = json!({"k": true});
        assert_eq!(optional_bool(&payload, "k"), Some(true));
    }

    #[test]
    fn optional_bool_returns_none_for_missing() {
        let payload = json!({});
        assert_eq!(optional_bool(&payload, "k"), None);
    }

    #[test]
    fn optional_u64_valid() {
        let payload = json!({"k": 42});
        assert_eq!(optional_u64(&payload, "k").unwrap(), Some(42));
    }

    #[test]
    fn optional_u64_none_for_missing() {
        let payload = json!({});
        assert_eq!(optional_u64(&payload, "k").unwrap(), None);
    }

    #[test]
    fn optional_u64_error_for_string() {
        let payload = json!({"k": "not_a_number"});
        assert!(optional_u64(&payload, "k").is_err());
    }

    #[test]
    fn optional_usize_uses_default() {
        let payload = json!({});
        assert_eq!(optional_usize(&payload, "k", 99).unwrap(), 99);
    }

    #[test]
    fn optional_string_array_parses() {
        let payload = json!({"k": ["a", "b", "c"]});
        let result = optional_string_array(&payload, "k").unwrap();
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn optional_string_array_none_for_missing() {
        let payload = json!({});
        assert_eq!(optional_string_array(&payload, "k"), None);
    }

    // --- json_type_name ---

    #[test]
    fn json_type_name_covers_all_variants() {
        assert_eq!(json_type_name(&Value::Null), "NoneType");
        assert_eq!(json_type_name(&json!(true)), "bool");
        assert_eq!(json_type_name(&json!(42)), "int");
        assert_eq!(json_type_name(&json!("hello")), "str");
        assert_eq!(json_type_name(&json!([])), "list");
        assert_eq!(json_type_name(&json!({})), "dict");
    }

    // --- truncate_text ---

    #[test]
    fn truncate_text_returns_original_when_short() {
        assert_eq!(truncate_text("hello", 10), "hello");
    }

    #[test]
    fn truncate_text_truncates_long_text() {
        let result = truncate_text("hello world", 5);
        assert_eq!(result, "hell...");
        assert_eq!(result.chars().count(), 7); // 4 + "..." = 7, wait no: max=5, take(5-1=4), + "..." = 7
    }

    // --- to_text_lines ---

    #[test]
    fn to_text_lines_deduplicates_and_trims() {
        let lines = to_text_lines("  hello  \nworld\nhello\n");
        assert_eq!(lines, vec!["hello", "world"]);
    }

    #[test]
    fn to_text_lines_limits_to_50() {
        let big = (0..100).map(|i| format!("line{i}")).collect::<Vec<_>>().join("\n");
        let lines = to_text_lines(&big);
        assert!(lines.len() <= 50);
    }

    // --- cdp_key_name ---

    #[test]
    fn cdp_key_name_maps_return_to_enter() {
        assert_eq!(cdp_key_name("Return"), "Enter");
        assert_eq!(cdp_key_name("Tab"), "Tab");
    }

    // --- decode_base64 ---

    #[test]
    fn decode_base64_standard() {
        assert_eq!(decode_base64("SGVsbG8=").unwrap(), b"Hello");
    }

    #[test]
    fn decode_base64_no_padding() {
        assert_eq!(decode_base64("SGVsbG8").unwrap(), b"Hello");
    }

    #[test]
    fn decode_base64_empty() {
        assert_eq!(decode_base64("").unwrap(), b"");
    }

    #[test]
    fn decode_base64_invalid_byte() {
        assert!(decode_base64("SGVs#G8=").is_err());
    }

    // --- env helpers ---

    #[test]
    fn resolve_headless_option_cli_overrides_env() {
        assert_eq!(resolve_headless_option(Some("false".to_string())), false);
        assert_eq!(resolve_headless_option(Some("true".to_string())), true);
    }

    // --- opt_string_value ---

    #[test]
    fn opt_string_value_some_and_none() {
        assert_eq!(opt_string_value(Some("hello".to_string())), json!("hello"));
        assert_eq!(opt_string_value(None), Value::Null);
    }

    // --- value_string ---

    #[test]
    fn value_string_handles_all_types() {
        assert_eq!(value_string(Some(&json!("text"))), "text");
        assert_eq!(value_string(Some(&json!(42))), "42");
        assert_eq!(value_string(Some(&json!(true))), "true");
        assert_eq!(value_string(Some(&Value::Null)), "");
        assert_eq!(value_string(None), "");
        assert_eq!(value_string(Some(&json!(["a", "b"]))), "a b");
    }

    // --- compact_summary ---

    #[test]
    fn compact_summary_truncates_text() {
        let summary = json!({
            "mainGoalArea": "a".repeat(1000),
            "visibleMessages": ["msg1", "msg2"],
            "forms": 2,
            "dialogs": 1
        });
        let compact = compact_summary(&summary, 100);
        // truncate_text adds "...", so result is max_chars + 3 chars ("...")
        let main_text = compact["mainGoalArea"].as_str().unwrap();
        assert!(main_text.chars().count() <= 103, "mainGoalArea should be truncated: {} chars", main_text.chars().count());
        assert!(main_text.chars().count() < 1000, "mainGoalArea should be shorter than original");
        assert_eq!(compact["forms"], 2);
    }

    // --- session_not_found_error ---

    #[test]
    fn session_not_found_error_format() {
        let err = session_not_found_error();
        assert_eq!(err["code"], "SESSION_NOT_FOUND");
        assert!(err["suggested_next_actions"].as_array().unwrap().iter().any(|a| a == "call browser_open"));
    }

    // --- success_response / error_response ---

    #[test]
    fn success_response_wraps_jsonrpc() {
        let resp = success_response(json!(1), json!({"ok": true}));
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["id"], 1);
        assert_eq!(resp["result"]["ok"], true);
    }

    #[test]
    fn error_response_wraps_jsonrpc() {
        let err = browser_error("CODE", "msg", &[], false);
        let resp = error_response(json!(1), err);
        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["error"]["code"], -32000);
    }

    // --- json_string_literal ---

    #[test]
    fn json_string_literal_produces_quoted_string() {
        assert_eq!(json_string_literal("hello"), "\"hello\"");
        assert_eq!(json_string_literal("a\"b"), "\"a\\\"b\"");
    }

    // --- descriptor_leaf / descriptor_string / descriptor_bool ---

    #[test]
    fn descriptor_leaf_navigates_path() {
        let val = json!({"a": {"b": {"c": 42}}});
        assert_eq!(descriptor_leaf(&val, &["a", "b", "c"]), Some(&json!(42)));
        assert_eq!(descriptor_leaf(&val, &["a", "missing"]), None);
    }

    #[test]
    fn descriptor_string_extracts_string() {
        let val = json!({"a": "hello"});
        assert_eq!(descriptor_string(&val, &["a"]), Some("hello".to_string()));
        assert_eq!(descriptor_string(&val, &["b"]), None);
    }

    #[test]
    fn descriptor_bool_extracts_bool() {
        let val = json!({"a": true, "b": "not_bool"});
        assert_eq!(descriptor_bool(&val, &["a"]), Some(true));
        assert_eq!(descriptor_bool(&val, &["b"]), None);
        assert_eq!(descriptor_bool(&val, &["c"]), None);
    }

    // --- normalize_text (via to_text_lines usage) ---

    #[test]
    fn normalize_runtime_locator_returns_original_when_missing() {
        let result = normalize_runtime_locator_for_existing_file("/nonexistent/path/file.json");
        assert_eq!(result, "/nonexistent/path/file.json");
    }

    // --- attach_candidate_ranking ---

    #[test]
    fn manifest_attach_candidate_rejects_wrong_schema() {
        let payload = json!({"schema_version": "wrong"});
        assert!(manifest_attach_candidate(&payload, "/path".to_string(), 0).is_none());
    }

    #[test]
    fn manifest_attach_candidate_accepts_valid() {
        let payload = json!({
            "schema_version": TRACE_RESUME_MANIFEST_SCHEMA_VERSION,
            "event_transport_path": "/transport.json"
        });
        let candidate = manifest_attach_candidate(&payload, "/path".to_string(), 100).unwrap();
        assert_eq!(candidate.path, "/path");
        assert_eq!(candidate.rank.recency_ms, 100);
        assert_eq!(candidate.rank.source_priority, 1);
    }

    #[test]
    fn binding_attach_candidate_rejects_wrong_schema() {
        let payload = json!({"schema_version": "wrong"});
        assert!(binding_attach_candidate(&payload, "/path".to_string(), 0).is_none());
    }

    #[test]
    fn binding_attach_candidate_rejects_filesystem_backend() {
        let payload = json!({
            "schema_version": RUNTIME_EVENT_TRANSPORT_SCHEMA_VERSION,
            "binding_backend_family": "filesystem"
        });
        assert!(binding_attach_candidate(&payload, "/path".to_string(), 0).is_none());
    }

    // --- should_skip_attach_discovery_dir ---

    #[test]
    fn should_skip_known_dirs() {
        assert!(should_skip_attach_discovery_dir(Path::new("/foo/.git")));
        assert!(should_skip_attach_discovery_dir(Path::new("/foo/node_modules")));
        assert!(should_skip_attach_discovery_dir(Path::new("/foo/target")));
        assert!(!should_skip_attach_discovery_dir(Path::new("/foo/artifacts")));
    }

    // --- parse_rfc3339_millis ---

    #[test]
    fn parse_rfc3339_millis_valid() {
        let result = parse_rfc3339_millis("2026-06-01T00:00:00Z");
        assert!(result.is_some());
        assert!(result.unwrap() > 0);
    }

    #[test]
    fn parse_rfc3339_millis_invalid() {
        assert!(parse_rfc3339_millis("not-a-date").is_none());
    }

}
