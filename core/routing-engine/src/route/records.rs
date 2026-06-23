//! Runtime/manifest record loading and cache.
use rayon::prelude::*;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::SystemTime;

use super::constants::{PARALLEL_RECORD_SCAN_MIN, RECORDS_CACHE_MAX_KEYS};
use super::skill_record::{negative_trigger_tokens, skill_record_from_raw};
use super::text::{read_json, value_to_string, value_to_string_list};
use super::types::{
    InlineSkillRecordPayload, RawSkillRecord, RecordRowIndexes, RecordsCacheEntry, RecordsCacheKey,
    RecordsCacheState, RouteMetadataPatch, SkillRecord,
};

pub fn load_records(
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
) -> Result<Vec<SkillRecord>, String> {
    let default_runtime_path = default_runtime_path();
    let runtime_path = runtime_path.or(default_runtime_path.as_deref());
    if let Some(path) = runtime_path
        && path.exists() {
            // Try lightweight index first; fall back to full runtime on failure.
            let mut records = load_records_from_index_or_runtime(path)?;
            if let Some(manifest) = manifest_path
                && manifest.exists() {
                    let meta = load_manifest_route_meta(manifest)?;
                    apply_manifest_route_meta(&mut records, &meta);
                    apply_manifest_host_platforms(&mut records, manifest)?;
                }
            return Ok(records);
        }
    if let Some(path) = manifest_path
        && path.exists() {
            return load_records_from_manifest(path);
        }
    Err("No routing index found.".to_string())
}

/// Try loading from `SKILL_ROUTING_INDEX.json` (sibling of runtime file).
/// Falls back to the full runtime file if the index is missing or unparseable.
fn load_records_from_index_or_runtime(runtime_path: &Path) -> Result<Vec<SkillRecord>, String> {
    if let Some(index_path) = index_sibling_path(runtime_path)
        && index_path.is_file() {
            match load_records_from_index(&index_path) {
                Ok(records) if !records.is_empty() => return Ok(records),
                Ok(_) => tracing::info!("[router-rs] index file empty, falling back to full runtime"),
                Err(err) => tracing::warn!(
                    "[router-rs] index load failed ({}), falling back to full runtime",
                    err
                ),
            }
        }
    load_records_from_runtime(runtime_path)
}

/// Derive the index JSON path next to the runtime JSON.
fn index_sibling_path(runtime_path: &Path) -> Option<PathBuf> {
    runtime_path
        .parent()
        .map(|parent| parent.join("SKILL_ROUTING_INDEX.json"))
}

/// Create a `(key → position)` index from a JSON `keys` array.
fn build_key_index(keys: &[Value], path: &Path) -> Result<HashMap<String, usize>, String> {
    let mut index: HashMap<String, usize> = HashMap::new();
    for (pos, key) in keys.iter().enumerate() {
        if let Some(raw) = key.as_str() {
            index.insert(raw.to_string(), pos);
        }
    }
    if index.is_empty() {
        return Err(format!("empty keys array: {}", path.display()));
    }
    Ok(index)
}

/// Parse the lightweight index file (`SKILL_ROUTING_INDEX.json`).
/// The index has the same array-of-arrays format but with fewer keys and
/// truncated description / trigger_hints fields.
fn load_records_from_index(path: &Path) -> Result<Vec<SkillRecord>, String> {
    let payload = read_json(path)?;
    let rows = payload
        .get("skills")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("index missing skills rows: {}", path.display()))?;
    let keys = payload
        .get("keys")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("index missing keys: {}", path.display()))?;

    let index = build_key_index(keys, path)?;

    let idx_slug = *index
        .get("slug")
        .ok_or_else(|| format!("index missing slug key: {}", path.display()))?;
    let idx_layer = *index
        .get("layer")
        .ok_or_else(|| format!("index missing layer key: {}", path.display()))?;
    let idx_owner = *index
        .get("owner")
        .ok_or_else(|| format!("index missing owner key: {}", path.display()))?;
    let idx_gate = *index
        .get("gate")
        .ok_or_else(|| format!("index missing gate key: {}", path.display()))?;
    let idx_summary = *index
        .get("summary")
        .or_else(|| index.get("description"))
        .ok_or_else(|| format!("index missing summary/description key: {}", path.display()))?;
    let idx_trigger_hints = *index
        .get("trigger_hints")
        .or_else(|| index.get("triggers"))
        .ok_or_else(|| format!("index missing trigger_hints key: {}", path.display()))?;
    let idx_priority = index.get("priority").copied();
    let idx_session_start = index.get("session_start").copied();
    let indexes = RecordRowIndexes::from_required(
        [
            idx_slug,
            idx_layer,
            idx_owner,
            idx_gate,
            idx_summary,
            idx_trigger_hints,
        ],
        idx_priority,
        idx_session_start,
    );
    let indexes = RecordRowIndexes {
        skill_path: index.get("skill_path").copied(),
        host_platforms: index
            .get("host_platforms")
            .or_else(|| index.get("source_position"))
            .copied(),
        record_kind: index.get("kind").copied(),
        skill_flags: index.get("skill_flags").copied(),
        ..indexes
    };

    // Merge sidecar metadata (same as full runtime path).
    let mut records = collect_skill_records_from_rows(rows, indexes);
    let mut meta = HashMap::new();
    merge_sidecar_route_metadata(path, route_metadata_sidecar_for_runtime, &mut meta)?;
    apply_manifest_route_meta(&mut records, &meta);
    Ok(records)
}

pub fn load_inline_records(payload: &Value) -> Result<Vec<SkillRecord>, String> {
    let rows = payload
        .get("skills")
        .and_then(Value::as_array)
        .ok_or_else(|| "inline route requires a skills array".to_string())?;
    if rows.len() < PARALLEL_RECORD_SCAN_MIN {
        return rows.iter().map(inline_skill_record).collect();
    }
    rows.par_iter().map(inline_skill_record).collect()
}

fn inline_skill_record(row: &Value) -> Result<SkillRecord, String> {
    let name = row
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if name.is_empty() {
        return Err("inline skill payload missing name".to_string());
    }
    let skill = InlineSkillRecordPayload {
        name,
        description: optional_string_value(row, "description"),
        short_description: optional_string_value(row, "short_description"),
        when_to_use: optional_string_value(row, "when_to_use"),
        do_not_use: optional_string_value(row, "do_not_use"),
        routing_layer: optional_string_value(row, "routing_layer"),
        routing_owner: optional_string_value(row, "routing_owner"),
        routing_gate: optional_string_value(row, "routing_gate"),
        routing_priority: optional_string_value(row, "routing_priority"),
        session_start: optional_string_value(row, "session_start"),
        tags: optional_string_list_value(row, "tags"),
        trigger_hints: optional_string_list_value(row, "trigger_hints"),
    };
    Ok(skill_record_from_raw(RawSkillRecord {
        slug: skill.name,
        skill_path: None,
        layer: skill.routing_layer,
        owner: skill.routing_owner,
        gate: skill.routing_gate,
        priority: skill.routing_priority,
        session_start: skill.session_start,
        summary: skill.description,
        short_description: skill.short_description,
        when_to_use: skill.when_to_use,
        do_not_use: skill.do_not_use,
        tags: skill.tags,
        trigger_hints: skill.trigger_hints,
        host_platforms: Vec::new(),
        record_kind: "skill".to_string(),
        skill_flags: Vec::new(),
    }))
}

fn optional_string_value(row: &Value, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn optional_string_list_value(row: &Value, key: &str) -> Vec<String> {
    row.get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn build_skill_record_from_indexed_row(row: &[Value], indexes: &RecordRowIndexes) -> SkillRecord {
    skill_record_from_raw(RawSkillRecord {
        slug: value_to_string(&row[indexes.slug]),
        skill_path: indexes
            .skill_path
            .and_then(|idx| row.get(idx))
            .map(value_to_string)
            .filter(|value| !value.trim().is_empty()),
        layer: value_to_string(&row[indexes.layer]),
        owner: value_to_string(&row[indexes.owner]),
        gate: value_to_string(&row[indexes.gate]),
        priority: indexes
            .priority
            .and_then(|idx| row.get(idx))
            .map(value_to_string)
            .unwrap_or_else(|| "P2".to_string()),
        session_start: indexes
            .session_start
            .and_then(|idx| row.get(idx))
            .map(value_to_string)
            .unwrap_or_else(|| "n/a".to_string()),
        summary: value_to_string(&row[indexes.summary]),
        short_description: String::new(),
        when_to_use: String::new(),
        do_not_use: String::new(),
        tags: Vec::new(),
        trigger_hints: value_to_string_list(&row[indexes.trigger_hints]),
        host_platforms: indexes
            .host_platforms
            .and_then(|idx| row.get(idx))
            .filter(|value| value.is_array())
            .map(value_to_string_list)
            .unwrap_or_default(),
        record_kind: indexes
            .record_kind
            .and_then(|idx| row.get(idx))
            .map(value_to_string)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "skill".to_string()),
        skill_flags: indexes
            .skill_flags
            .and_then(|idx| row.get(idx))
            .filter(|value| value.is_array())
            .map(value_to_string_list)
            .unwrap_or_default(),
    })
}

fn collect_skill_records_from_rows(rows: &[Value], indexes: RecordRowIndexes) -> Vec<SkillRecord> {
    let iter = || {
        rows.iter()
            .filter_map(Value::as_array)
            .filter(|row| row.len() > indexes.required_max)
            .map(|row| build_skill_record_from_indexed_row(row, &indexes))
            .collect::<Vec<_>>()
    };
    if rows.len() < PARALLEL_RECORD_SCAN_MIN {
        return iter();
    }
    rows.par_iter()
        .filter_map(Value::as_array)
        .filter(|row| row.len() > indexes.required_max)
        .map(|row| build_skill_record_from_indexed_row(row, &indexes))
        .collect()
}

fn apply_manifest_route_meta(
    records: &mut [SkillRecord],
    meta: &HashMap<String, RouteMetadataPatch>,
) {
    if records.len() < PARALLEL_RECORD_SCAN_MIN {
        for record in records {
            if let Some(patch) = meta.get(&record.slug) {
                apply_route_metadata_patch(record, patch);
            }
        }
        return;
    }
    records.par_iter_mut().for_each(|record| {
        if let Some(patch) = meta.get(&record.slug) {
            apply_route_metadata_patch(record, patch);
        }
    });
}

/// Cached host platforms from `RUNTIME_REGISTRY.json` keyed by canonical repo root.
static HOST_PLATFORMS_REGISTRY: OnceLock<RwLock<HashMap<PathBuf, Vec<String>>>> = OnceLock::new();

/// Load the list of registered host platforms from `configs/framework/RUNTIME_REGISTRY.json`,
/// using a module-level cache to avoid repeated file I/O and JSON parsing.
///
/// `repo_root` points to the project root directory. The cache key is derived from its
/// canonical (realpath) form so that different path representations of the same root
/// share one cache entry.
fn load_host_platforms_registry(repo_root: &Path) -> Result<Vec<String>, String> {
    let cache = HOST_PLATFORMS_REGISTRY
        .get_or_init(|| RwLock::new(HashMap::new()));
    let canonical = fs::canonicalize(repo_root)
        .unwrap_or_else(|_| repo_root.to_path_buf());

    // Fast path: cached entry exists.
    {
        let guard = cache.read().map_err(|e| {
            format!("host platforms registry cache lock poisoned: {e}")
        })?;
        if let Some(hosts) = guard.get(&canonical) {
            return Ok(hosts.clone());
        }
    }

    // Load and parse the registry file.
    let reg_path = canonical.join("configs/framework/RUNTIME_REGISTRY.json");
    let payload = read_json(&reg_path)
        .map_err(|e| format!("failed to read {}: {e}", reg_path.display()))?;
    let mut hosts: Vec<String> = payload
        .get("host_targets")
        .and_then(|v| v.get("supported"))
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    hosts.sort();

    // Populate cache.
    let mut guard = cache.write().map_err(|e| {
        format!("host platforms registry cache lock poisoned: {e}")
    })?;
    guard.insert(canonical, hosts.clone());
    Ok(hosts)
}

/// Walk ancestors from `start` until a directory containing
/// `configs/framework/RUNTIME_REGISTRY.json` is found.  Returns `None`
/// if the repo root cannot be determined (e.g. the file was moved or
/// the project layout does not match expectations).
fn discover_repo_root(start: &Path) -> Option<PathBuf> {
    let canonical = fs::canonicalize(start).ok()?;
    for ancestor in canonical.ancestors() {
        if ancestor.join("configs/framework/RUNTIME_REGISTRY.json").is_file() {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}

/// Expand `["supported"]` / `["all-hosts"]` wildcard in host_platforms to all registered hosts.
fn expand_supported_host_platforms(records: &mut [SkillRecord], any_sibling_path: &Path) {
    let is_wildcard =
        |hp: &[String]| hp.len() == 1 && (hp[0] == "supported" || hp[0] == "all-hosts");
    if !records.iter().any(|r| is_wildcard(&r.host_platforms)) {
        return;
    }
    let Some(repo_root) = discover_repo_root(any_sibling_path) else {
        return;
    };
    let Ok(all_hosts) = load_host_platforms_registry(&repo_root) else {
        return;
    };
    if all_hosts.is_empty() {
        return;
    }
    for record in records.iter_mut() {
        if is_wildcard(&record.host_platforms) {
            record.host_platforms = all_hosts.clone();
        }
    }
}

fn apply_manifest_host_platforms(
    records: &mut [SkillRecord],
    manifest_path: &Path,
) -> Result<(), String> {
    let payload = read_json(manifest_path)?;
    let rows = payload
        .get("skills")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("manifest missing skills rows: {}", manifest_path.display()))?;
    let keys = payload
        .get("keys")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("manifest missing keys: {}", manifest_path.display()))?;
    let key_index = keys
        .iter()
        .enumerate()
        .filter_map(|(idx, key)| key.as_str().map(|raw| (raw.to_string(), idx)))
        .collect::<HashMap<_, _>>();
    let idx_slug = *key_index
        .get("slug")
        .ok_or_else(|| format!("manifest missing slug key: {}", manifest_path.display()))?;
    let Some(idx_hosts) = key_index.get("host_platforms") else {
        return Ok(());
    };

    // Load all registered hosts so [supported] can be expanded.
    let all_hosts: Vec<String> = manifest_path
        .parent()
        .and_then(|p| p.parent())
        .and_then(|repo| load_host_platforms_registry(repo).ok())
        .unwrap_or_default();

    let mut hosts_by_slug = HashMap::new();
    for row in rows.iter().filter_map(Value::as_array) {
        if row.len() <= idx_slug.max(*idx_hosts) {
            continue;
        }
        let slug = value_to_string(&row[idx_slug]);
        let raw_hosts = value_to_string_list(&row[*idx_hosts]);
        // Expand [supported] / [all-hosts] wildcard to all registered hosts.
        let hosts = if raw_hosts.len() == 1
            && (raw_hosts[0] == "supported" || raw_hosts[0] == "all-hosts")
            && !all_hosts.is_empty()
        {
            all_hosts.clone()
        } else {
            raw_hosts
        };
        if !hosts.is_empty() {
            hosts_by_slug.insert(slug, hosts);
        }
    }
    for record in records.iter_mut() {
        if let Some(hosts) = hosts_by_slug.get(&record.slug) {
            record.host_platforms = hosts.clone();
        }
    }
    Ok(())
}

fn apply_route_metadata_patch(record: &mut SkillRecord, patch: &RouteMetadataPatch) {
    if let Some(priority) = &patch.priority {
        record.priority = priority.clone();
    }
    if let Some(session_start) = &patch.session_start {
        record.session_start = session_start.clone();
    }
    if !patch.positive_triggers.is_empty() {
        record
            .metadata_positive_triggers
            .extend(patch.positive_triggers.iter().cloned());
    }
    record.do_not_use_tokens.extend(negative_trigger_tokens(
        patch.negative_triggers.iter().map(String::as_str),
    ));
    if let Some(primary_allowed) = patch.primary_allowed {
        record.primary_allowed = primary_allowed;
    }
    if let Some(mode) = &patch.fallback_policy_mode {
        record.fallback_policy_mode = mode.clone();
    }
}

fn default_runtime_path() -> Option<PathBuf> {
    if let Some(root) = crate::hooks::discover_skill_repo_root() {
        let path = crate::hooks::skill_routing_runtime_json(&root);
        if path.is_file() {
            return Some(path);
        }
    }
    let fallback = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("skills")
        .join("SKILL_ROUTING_RUNTIME.json");
    fallback.is_file().then_some(fallback)
}

fn effective_runtime_path(runtime_path: Option<&Path>) -> Option<PathBuf> {
    runtime_path
        .map(Path::to_path_buf)
        .or_else(default_runtime_path)
}

pub fn load_records_cached_for_stdio_with_default_runtime_path(
    default_runtime_path: &Path,
    manifest_path: Option<&Path>,
) -> Result<Arc<Vec<SkillRecord>>, String> {
    load_records_cached_for_stdio_resolved(Some(default_runtime_path), manifest_path)
}

fn records_cache_key(runtime_path: Option<&Path>, manifest_path: Option<&Path>) -> RecordsCacheKey {
    let metadata_sidecar = route_metadata_sidecar(runtime_path, manifest_path);
    RecordsCacheKey {
        runtime_path: runtime_path.map(Path::to_path_buf),
        manifest_path: manifest_path.map(Path::to_path_buf),
        metadata_sidecar_path: metadata_sidecar,
    }
}

fn file_modified_at(path: Option<&Path>) -> Option<SystemTime> {
    path.and_then(|item| fs::metadata(item).ok()?.modified().ok())
}

fn route_metadata_sidecar_path(manifest_path: &Path) -> Option<PathBuf> {
    manifest_path
        .parent()
        .map(|parent| parent.join("SKILL_ROUTING_METADATA.json"))
}

fn route_metadata_sidecar_for_runtime(runtime_path: &Path) -> Option<PathBuf> {
    runtime_path
        .parent()
        .map(|parent| parent.join("SKILL_ROUTING_METADATA.json"))
}

fn route_metadata_sidecar(
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
) -> Option<PathBuf> {
    runtime_path
        .and_then(route_metadata_sidecar_for_runtime)
        .or_else(|| manifest_path.and_then(route_metadata_sidecar_path))
}

fn records_cache_state() -> &'static RwLock<RecordsCacheState> {
    static RECORDS_CACHE: OnceLock<RwLock<RecordsCacheState>> = OnceLock::new();
    RECORDS_CACHE.get_or_init(|| RwLock::new(RecordsCacheState::default()))
}

fn evict_records_cache_over_capacity(state: &mut RecordsCacheState) {
    while state.map.len() > RECORDS_CACHE_MAX_KEYS {
        let Some(candidate) = state.fifo.pop_front() else {
            // 当 fifo 耗尽，按插入时间（inserted_at）淘汰最旧的条目。
            // 使用 inserted_at（加载时间）而非文件 mtime，避免刚加载的条目
            // 因文件 mtime 很旧而被立即淘汰导致的缓存颠簸。
            let oldest_key = state
                .map
                .iter()
                .map(|(k, v)| (k.clone(), v.inserted_at))
                .min_by_key(|(_, inserted_at)| *inserted_at)
                .map(|(k, _)| k);
            if let Some(key) = oldest_key {
                state.map.remove(&key);
            }
            continue;
        };
        if state.map.remove(&candidate).is_none() {
            // Stale fifo slot (defensive); keep draining.
            continue;
        }
    }
}

/// Invalidate all records cache entries (full flush).
pub fn invalidate_records_cache() -> Result<(), String> {
    let mut state = records_cache_state().write().map_err(|e| {
        tracing::warn!("[router-rs] route records cache lock poisoned: {e}");
        "route records cache lock poisoned".to_string()
    })?;
    state.map.clear();
    state.fifo.clear();
    Ok(())
}

pub fn load_records_cached_for_stdio(
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
) -> Result<Arc<Vec<SkillRecord>>, String> {
    let runtime_path = effective_runtime_path(runtime_path);
    let runtime_path = runtime_path.as_deref();
    load_records_cached_for_stdio_resolved(runtime_path, manifest_path)
}

pub fn load_records_cached_for_stdio_resolved(
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
) -> Result<Arc<Vec<SkillRecord>>, String> {
    let key = records_cache_key(runtime_path, manifest_path);
    let runtime_mtime = file_modified_at(runtime_path);
    let manifest_mtime = file_modified_at(manifest_path);
    let metadata_sidecar = route_metadata_sidecar(runtime_path, manifest_path);
    let metadata_mtime = file_modified_at(metadata_sidecar.as_deref());
    let index_mtime = file_modified_at(runtime_path.and_then(index_sibling_path).as_deref());

    {
        let state = records_cache_state().read().map_err(|e| {
            tracing::warn!("[router-rs] route records cache lock poisoned: {e}");
            "route records cache lock poisoned".to_string()
        })?;
        if let Some(entry) = state.map.get(&key)
            && entry.runtime_mtime == runtime_mtime
                && entry.manifest_mtime == manifest_mtime
                && entry.metadata_mtime == metadata_mtime
                && entry.index_mtime == index_mtime
            {
                return Ok(Arc::clone(&entry.records));
            }
    }

    let records = Arc::new(load_records(runtime_path, manifest_path)?);
    let now = SystemTime::now();
    let entry = RecordsCacheEntry {
        runtime_mtime,
        manifest_mtime,
        metadata_mtime,
        index_mtime,
        inserted_at: now,
        records: Arc::clone(&records),
    };
    let mut state = records_cache_state().write().map_err(|e| {
        tracing::warn!("[router-rs] route records cache lock poisoned: {e}");
        "route records cache lock poisoned".to_string()
    })?;
    let is_new_key = !state.map.contains_key(&key);
    state.map.insert(key.clone(), entry);
    if is_new_key {
        state.fifo.push_back(key);
    }
    evict_records_cache_over_capacity(&mut state);
    Ok(records)
}

fn load_manifest_route_meta(path: &Path) -> Result<HashMap<String, RouteMetadataPatch>, String> {
    let payload = read_json(path)?;
    let rows = payload
        .get("skills")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("manifest missing skills rows: {}", path.display()))?;
    let keys = payload
        .get("keys")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("manifest missing keys: {}", path.display()))?;

    let key_index = build_key_index(keys, path)?;

    let idx_slug = *key_index
        .get("slug")
        .ok_or_else(|| format!("manifest missing slug key: {}", path.display()))?;
    let idx_priority = key_index.get("priority").copied();
    let idx_session_start = key_index.get("session_start").copied();

    let mut meta = HashMap::new();
    for row in rows.iter().filter_map(Value::as_array) {
        if row.len() <= idx_slug {
            continue;
        }
        let slug = value_to_string(&row[idx_slug]);
        let priority = idx_priority
            .and_then(|idx| row.get(idx))
            .map(value_to_string)
            .filter(|value| !value.trim().is_empty());
        let session_start = idx_session_start
            .and_then(|idx| row.get(idx))
            .map(value_to_string)
            .filter(|value| !value.trim().is_empty());
        meta.insert(
            slug,
            RouteMetadataPatch {
                priority,
                session_start,
                positive_triggers: Vec::new(),
                negative_triggers: Vec::new(),
                primary_allowed: None,
                fallback_policy_mode: None,
            },
        );
    }
    merge_sidecar_route_metadata(path, route_metadata_sidecar_path, &mut meta)?;
    Ok(meta)
}

/// Merge route metadata from a sidecar JSON file.
/// `config_path` is passed to `sidecar_fn` to locate the sidecar.
fn merge_sidecar_route_metadata(
    config_path: &Path,
    sidecar_fn: fn(&Path) -> Option<PathBuf>,
    meta: &mut HashMap<String, RouteMetadataPatch>,
) -> Result<(), String> {
    let Some(sidecar) = sidecar_fn(config_path) else {
        return Ok(());
    };
    if !sidecar.is_file() {
        return Ok(());
    }
    let payload = read_json(&sidecar)?;
    merge_route_metadata_payload(&payload, meta)?;
    Ok(())
}

fn merge_route_metadata_payload(
    payload: &Value,
    meta: &mut HashMap<String, RouteMetadataPatch>,
) -> Result<(), String> {
    let Some(skills) = payload.get("skills").and_then(Value::as_object) else {
        return Ok(());
    };
    for (slug, record) in skills {
        let positive_triggers = record
            .get("positive_triggers")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let negative_triggers = record
            .get("negative_triggers")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let primary_allowed = record
            .get("overlay_policy")
            .and_then(|policy| policy.get("primary_allowed"))
            .and_then(Value::as_bool);
        let fallback_policy_mode = record
            .get("fallback_policy")
            .and_then(|policy| policy.get("mode"))
            .and_then(Value::as_str)
            .map(str::to_string);
        if let Some(mode) = fallback_policy_mode.as_deref()
            && !matches!(
                mode,
                "eligible-in-runtime" | "explicit-or-fallback" | "explicit-only" | "never"
            ) {
                return Err(format!(
                    "unsupported fallback_policy.mode `{mode}` for skill `{slug}`"
                ));
            }
        if positive_triggers.is_empty()
            && negative_triggers.is_empty()
            && primary_allowed.is_none()
            && fallback_policy_mode.is_none()
        {
            continue;
        }
        let patch = meta.entry(slug.clone()).or_default();
        patch.positive_triggers.extend(positive_triggers);
        patch.negative_triggers.extend(negative_triggers);
        if primary_allowed.is_some() {
            patch.primary_allowed = primary_allowed;
        }
        if fallback_policy_mode.is_some() {
            patch.fallback_policy_mode = fallback_policy_mode;
        }
    }
    Ok(())
}

pub fn load_records_from_runtime(path: &Path) -> Result<Vec<SkillRecord>, String> {
    let payload = read_json(path)?;
    let rows = payload
        .get("skills")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("runtime index missing skills rows: {}", path.display()))?;
    let keys = payload
        .get("keys")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("runtime index missing keys: {}", path.display()))?;

    let index = build_key_index(keys, path)?;

    let idx_slug = *index
        .get("slug")
        .ok_or_else(|| format!("runtime index missing slug key: {}", path.display()))?;
    let idx_layer = *index
        .get("layer")
        .ok_or_else(|| format!("runtime index missing layer key: {}", path.display()))?;
    let idx_owner = *index
        .get("owner")
        .ok_or_else(|| format!("runtime index missing owner key: {}", path.display()))?;
    let idx_gate = *index
        .get("gate")
        .ok_or_else(|| format!("runtime index missing gate key: {}", path.display()))?;
    let idx_summary = *index
        .get("summary")
        .or_else(|| index.get("description"))
        .ok_or_else(|| format!("runtime index missing summary key: {}", path.display()))?;
    let idx_trigger_hints = *index
        .get("trigger_hints")
        .or_else(|| index.get("triggers"))
        .ok_or_else(|| {
            format!(
                "runtime index missing trigger_hints key: {}",
                path.display()
            )
        })?;
    let idx_priority = index.get("priority").copied();
    let idx_session_start = index.get("session_start").copied();
    let indexes = RecordRowIndexes::from_required(
        [
            idx_slug,
            idx_layer,
            idx_owner,
            idx_gate,
            idx_summary,
            idx_trigger_hints,
        ],
        idx_priority,
        idx_session_start,
    );
    let indexes = RecordRowIndexes {
        skill_path: index.get("skill_path").copied(),
        host_platforms: index
            .get("host_platforms")
            .or_else(|| index.get("source_position"))
            .copied(),
        record_kind: index.get("kind").copied(),
        skill_flags: index.get("skill_flags").copied(),
        ..indexes
    };

    let mut records = collect_skill_records_from_rows(rows, indexes);
    let mut meta = HashMap::new();
    merge_sidecar_route_metadata(path, route_metadata_sidecar_for_runtime, &mut meta)?;
    apply_manifest_route_meta(&mut records, &meta);
    Ok(records)
}

pub fn load_records_from_manifest(path: &Path) -> Result<Vec<SkillRecord>, String> {
    let payload = read_json(path)?;
    let rows = payload
        .get("skills")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("manifest missing skills rows: {}", path.display()))?;
    let keys = payload
        .get("keys")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("manifest missing keys: {}", path.display()))?;

    let key_index = build_key_index(keys, path)?;

    let idx_slug = *key_index
        .get("slug")
        .ok_or_else(|| format!("manifest missing slug key: {}", path.display()))?;
    let idx_layer = *key_index
        .get("layer")
        .ok_or_else(|| format!("manifest missing layer key: {}", path.display()))?;
    let idx_owner = *key_index
        .get("owner")
        .ok_or_else(|| format!("manifest missing owner key: {}", path.display()))?;
    let idx_gate = *key_index
        .get("gate")
        .ok_or_else(|| format!("manifest missing gate key: {}", path.display()))?;
    let idx_desc = *key_index
        .get("description")
        .or_else(|| key_index.get("summary"))
        .ok_or_else(|| format!("manifest missing description key: {}", path.display()))?;
    let idx_trigger_hints = *key_index
        .get("trigger_hints")
        .or_else(|| key_index.get("triggers"))
        .ok_or_else(|| format!("manifest missing trigger_hints key: {}", path.display()))?;
    let idx_priority = key_index.get("priority").copied();
    let idx_session_start = key_index.get("session_start").copied();
    let indexes = RecordRowIndexes::from_required(
        [
            idx_slug,
            idx_layer,
            idx_owner,
            idx_gate,
            idx_desc,
            idx_trigger_hints,
        ],
        idx_priority,
        idx_session_start,
    );
    let indexes = RecordRowIndexes {
        skill_path: key_index.get("skill_path").copied(),
        host_platforms: key_index.get("host_platforms").copied(),
        record_kind: key_index.get("kind").copied(),
        skill_flags: key_index.get("skill_flags").copied(),
        ..indexes
    };

    let mut records = collect_skill_records_from_rows(rows, indexes);
    // Expand [supported] / [all-hosts] wildcard in host_platforms.
    expand_supported_host_platforms(&mut records, path);
    Ok(records)
}
