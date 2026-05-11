//! Runtime/manifest record loading and cache.
use rayon::prelude::*;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use super::constants::{PARALLEL_RECORD_SCAN_MIN, RECORDS_CACHE_MAX_KEYS};
use super::skill_record::negative_trigger_tokens;
use super::text::{read_json, value_to_string, value_to_string_list};
use super::types::{
    InlineSkillRecordPayload, RawSkillRecord, RecordRowIndexes, RecordsCacheEntry, RecordsCacheKey,
    RecordsCacheState, RouteMetadataPatch, SkillRecord,
};

pub(crate) fn load_records(
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
) -> Result<Vec<SkillRecord>, String> {
    let default_runtime_path = default_runtime_path();
    let runtime_path = runtime_path.or(default_runtime_path.as_deref());
    if let Some(path) = runtime_path {
        if path.exists() {
            let mut records = load_records_from_runtime(path)?;
            if let Some(manifest) = manifest_path {
                if manifest.exists() {
                    let meta = load_manifest_route_meta(manifest)?;
                    apply_manifest_route_meta(&mut records, &meta);
                }
            }
            return Ok(records);
        }
    }
    if let Some(path) = manifest_path {
        if path.exists() {
            return load_records_from_manifest(path);
        }
    }
    Err("No routing index found.".to_string())
}

pub(crate) fn load_inline_records(payload: &Value) -> Result<Vec<SkillRecord>, String> {
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
    let skill = serde_json::from_value::<InlineSkillRecordPayload>(row.clone())
        .map_err(|err| format!("parse inline skill payload failed: {err}"))?;
    Ok(SkillRecord::from_raw(RawSkillRecord {
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
    }))
}

fn build_skill_record_from_indexed_row(row: &[Value], indexes: &RecordRowIndexes) -> SkillRecord {
    SkillRecord::from_raw(RawSkillRecord {
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

fn apply_route_metadata_patch(record: &mut SkillRecord, patch: &RouteMetadataPatch) {
    if let Some(priority) = &patch.priority {
        record.priority = priority.clone();
    }
    if let Some(session_start) = &patch.session_start {
        record.session_start = session_start.clone();
    }
    record.do_not_use_tokens.extend(negative_trigger_tokens(
        patch.negative_triggers.iter().map(String::as_str),
    ));
}

fn default_runtime_path() -> Option<PathBuf> {
    if let Some(root) = crate::skill_repo::discover_skill_policy_repo_root() {
        let path = crate::skill_repo::skill_routing_runtime_json(&root);
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

#[cfg(test)]
pub(crate) fn load_records_cached_for_stdio_with_default_runtime_path(
    default_runtime_path: &Path,
    manifest_path: Option<&Path>,
) -> Result<Arc<Vec<SkillRecord>>, String> {
    load_records_cached_for_stdio_resolved(Some(default_runtime_path), manifest_path)
}

fn records_cache_key(runtime_path: Option<&Path>, manifest_path: Option<&Path>) -> RecordsCacheKey {
    RecordsCacheKey {
        runtime_path: runtime_path.map(Path::to_path_buf),
        manifest_path: manifest_path.map(Path::to_path_buf),
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

fn records_cache_state() -> &'static Mutex<RecordsCacheState> {
    static RECORDS_CACHE: OnceLock<Mutex<RecordsCacheState>> = OnceLock::new();
    RECORDS_CACHE.get_or_init(|| Mutex::new(RecordsCacheState::default()))
}

fn evict_records_cache_over_capacity(state: &mut RecordsCacheState) {
    while state.map.len() > RECORDS_CACHE_MAX_KEYS {
        let Some(candidate) = state.fifo.pop_front() else {
            let Some(arbitrary) = state.map.keys().next().cloned() else {
                break;
            };
            state.map.remove(&arbitrary);
            continue;
        };
        if state.map.remove(&candidate).is_none() {
            // Stale fifo slot (defensive); keep draining.
            continue;
        }
    }
}

pub(crate) fn load_records_cached_for_stdio(
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
) -> Result<Arc<Vec<SkillRecord>>, String> {
    let runtime_path = effective_runtime_path(runtime_path);
    let runtime_path = runtime_path.as_deref();
    load_records_cached_for_stdio_resolved(runtime_path, manifest_path)
}

pub(crate) fn load_records_cached_for_stdio_resolved(
    runtime_path: Option<&Path>,
    manifest_path: Option<&Path>,
) -> Result<Arc<Vec<SkillRecord>>, String> {
    let key = records_cache_key(runtime_path, manifest_path);
    let runtime_mtime = file_modified_at(runtime_path);
    let manifest_mtime = file_modified_at(manifest_path);
    let metadata_sidecar = route_metadata_sidecar(runtime_path, manifest_path);
    let metadata_mtime = file_modified_at(metadata_sidecar.as_deref());

    {
        let state = records_cache_state()
            .lock()
            .map_err(|_| "route records cache lock poisoned".to_string())?;
        if let Some(entry) = state.map.get(&key) {
            if entry.runtime_mtime == runtime_mtime
                && entry.manifest_mtime == manifest_mtime
                && entry.metadata_mtime == metadata_mtime
            {
                return Ok(Arc::clone(&entry.records));
            }
        }
    }

    let records = Arc::new(load_records(runtime_path, manifest_path)?);
    let entry = RecordsCacheEntry {
        runtime_mtime,
        manifest_mtime,
        metadata_mtime,
        records: Arc::clone(&records),
    };
    let mut state = records_cache_state()
        .lock()
        .map_err(|_| "route records cache lock poisoned".to_string())?;
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

    let key_index = keys
        .iter()
        .enumerate()
        .filter_map(|(idx, key)| key.as_str().map(|raw| (raw.to_string(), idx)))
        .collect::<HashMap<_, _>>();

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
                negative_triggers: Vec::new(),
            },
        );
    }
    merge_sidecar_route_metadata(path, &mut meta)?;
    Ok(meta)
}

fn merge_sidecar_route_metadata(
    manifest_path: &Path,
    meta: &mut HashMap<String, RouteMetadataPatch>,
) -> Result<(), String> {
    let Some(sidecar) = route_metadata_sidecar_path(manifest_path) else {
        return Ok(());
    };
    if !sidecar.is_file() {
        return Ok(());
    }
    let payload = read_json(&sidecar)?;
    merge_route_metadata_payload(&payload, meta);
    Ok(())
}

fn merge_route_metadata_payload(payload: &Value, meta: &mut HashMap<String, RouteMetadataPatch>) {
    let Some(skills) = payload.get("skills").and_then(Value::as_object) else {
        return;
    };
    for (slug, record) in skills {
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
        if negative_triggers.is_empty() {
            continue;
        }
        meta.entry(slug.clone())
            .or_default()
            .negative_triggers
            .extend(negative_triggers);
    }
}

pub(crate) fn load_records_from_runtime(path: &Path) -> Result<Vec<SkillRecord>, String> {
    let payload = read_json(path)?;
    let rows = payload
        .get("skills")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("runtime index missing skills rows: {}", path.display()))?;
    let keys = payload
        .get("keys")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("runtime index missing keys: {}", path.display()))?;

    let mut index: HashMap<String, usize> = HashMap::new();
    for (idx, key) in keys.iter().enumerate() {
        if let Some(raw) = key.as_str() {
            index.insert(raw.to_string(), idx);
        }
    }

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
        ..indexes
    };

    let mut records = collect_skill_records_from_rows(rows, indexes);
    let mut meta = HashMap::new();
    merge_sidecar_route_metadata_from_runtime(path, &mut meta)?;
    apply_manifest_route_meta(&mut records, &meta);
    Ok(records)
}

fn merge_sidecar_route_metadata_from_runtime(
    runtime_path: &Path,
    meta: &mut HashMap<String, RouteMetadataPatch>,
) -> Result<(), String> {
    let Some(sidecar) = route_metadata_sidecar_for_runtime(runtime_path) else {
        return Ok(());
    };
    if !sidecar.is_file() {
        return Ok(());
    }
    let payload = read_json(&sidecar)?;
    merge_route_metadata_payload(&payload, meta);
    Ok(())
}

pub(crate) fn load_records_from_manifest(path: &Path) -> Result<Vec<SkillRecord>, String> {
    let payload = read_json(path)?;
    let rows = payload
        .get("skills")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("manifest missing skills rows: {}", path.display()))?;
    let keys = payload
        .get("keys")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("manifest missing keys: {}", path.display()))?;

    let key_index = keys
        .iter()
        .enumerate()
        .filter_map(|(idx, key)| key.as_str().map(|raw| (raw.to_string(), idx)))
        .collect::<HashMap<_, _>>();

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
        ..indexes
    };

    Ok(collect_skill_records_from_rows(rows, indexes))
}
