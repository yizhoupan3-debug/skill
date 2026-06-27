use core_errors::FrameworkError;
use super::RuntimeStorageRequestPayload;
use super::backend::normalized_backend_family;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{ErrorKind, Read};
use std::path::{Component, Path, PathBuf};

#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_runtime_path(value: &str) -> Result<PathBuf, String> {
    let candidate = PathBuf::from(value.trim());
    if candidate.as_os_str().is_empty() {
        return Err("runtime storage path must be non-empty".to_string());
    }
    let absolute = if candidate.is_absolute() {
        candidate
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(candidate))
            .map_err(|err| format!("resolve runtime storage path failed: {err}"))?
    };
    canonicalize_or_clean_absolute_path(&absolute)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn clean_absolute_path(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!(
            "runtime storage path must be absolute after resolution: {}",
            path.display()
        ));
    }
    let mut cleaned = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => cleaned.push(prefix.as_os_str()),
            Component::RootDir => cleaned.push(component.as_os_str()),
            Component::CurDir => {}
            Component::Normal(segment) => cleaned.push(segment),
            Component::ParentDir => {
                if !cleaned.pop() {
                    return Err(format!(
                        "runtime storage path escapes filesystem root: {}",
                        path.display()
                    ));
                }
            }
        }
    }
    Ok(cleaned)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn canonicalize_or_clean_absolute_path(path: &Path) -> Result<PathBuf, String> {
    clean_absolute_path(path)
}

/// Resolve symlinks on the longest existing ancestor of `path`, then re-attach
/// any non-existing tail components verbatim. The returned path reflects the
/// real filesystem location after symlink resolution and is suitable for
/// containment checks against a canonical storage root, even when the final
/// target (or some intermediate components) does not yet exist.
#[tracing::instrument(level = "debug", skip_all)]
pub fn canonicalize_existing_ancestors(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!(
            "runtime storage path must be absolute before symlink resolution: {}",
            path.display()
        ));
    }

    let mut current = path.to_path_buf();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    loop {
        match fs::symlink_metadata(&current) {
            Ok(_) => break,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                let Some(file_name) = current.file_name().map(|name| name.to_os_string()) else {
                    return Err(format!(
                        "runtime storage path has no existing ancestor: {}",
                        path.display()
                    ));
                };
                tail.push(file_name);
                if !current.pop() {
                    return Err(format!(
                        "runtime storage path has no existing ancestor: {}",
                        path.display()
                    ));
                }
            }
            Err(err) => {
                return Err(format!(
                    "stat runtime storage path {} failed: {err}",
                    current.display()
                ));
            }
        }
    }

    let canonical = current.canonicalize().map_err(|err| {
        format!(
            "canonicalize runtime storage ancestor {} failed: {err}",
            current.display()
        )
    })?;

    let mut result = canonical;
    for name in tail.iter().rev() {
        result.push(name);
    }
    Ok(result)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn resolve_runtime_storage_path_with_root(
    request_path: &str,
    request_storage_root: Option<&str>,
) -> Result<(PathBuf, PathBuf), String> {
    let storage_root = match request_storage_root {
        Some(value) => normalize_runtime_path(value)?,
        None => {
            let cwd = std::env::current_dir()
                .map_err(|err| format!("resolve current dir failed: {err}"))?;
            canonicalize_or_clean_absolute_path(&cwd)?
        }
    };
    let trimmed_path = request_path.trim();
    if trimmed_path.is_empty() {
        return Err("runtime storage path must be non-empty".to_string());
    }
    let candidate = PathBuf::from(trimmed_path);
    let absolute_candidate = if candidate.is_absolute() {
        candidate
    } else {
        storage_root.join(candidate)
    };
    let resolved_path = canonicalize_or_clean_absolute_path(&absolute_candidate)?;
    if !resolved_path.starts_with(&storage_root) {
        return Err(format!(
            "runtime storage path {} must stay under storage root {}",
            resolved_path.display(),
            storage_root.display()
        ));
    }

    // Real-path containment: resolve any symlinks along the existing parent
    // chain on both sides before comparing. A lexical `starts_with` alone is
    // insufficient because a symlink in the parent directory chain (e.g. a
    // pre-existing `escape -> /outside` link inside `storage_root`) would
    // otherwise let writes leak outside `storage_root` even though every
    // textual component still appears to live under it.
    let canonical_storage_root = canonicalize_existing_ancestors(&storage_root)?;
    let canonical_resolved_path = canonicalize_existing_ancestors(&resolved_path)?;
    if !canonical_resolved_path.starts_with(&canonical_storage_root) {
        return Err(format!(
            "runtime storage path {} must stay under storage root {} after symlink resolution",
            canonical_resolved_path.display(),
            canonical_storage_root.display()
        ));
    }

    Ok((resolved_path, storage_root))
}

/// Pick the effective `storage_root` string for a runtime_storage request.
///
/// Order of resolution:
///   1. explicit non-empty `storage_root` from the request,
///   2. for sqlite/sqlite3 backends without an explicit root, fall back to
///      `sqlite_db_path.parent()` to preserve the historical default
///      semantics for sqlite-backed runtime storage,
///   3. opt-in host-aware fallback **only** when the caller explicitly
///      sets `ROUTER_RS_STORAGE_ROOT`. We deliberately do NOT silently
///      consult `CODEX_HOME` / `CURSOR_HOME` here, because callers in
///      codex/cursor environments routinely have those env vars set and
///      pass relative `path` arguments expecting cwd anchoring; redirecting
///      writes to the host home directory would be a silent breaking
///      change. Callers that want host-home anchoring must pass
///      `storage_root` explicitly in the request payload.
///   4. otherwise return `None` so the caller falls back to the
///      current working directory (legacy default for non-sqlite backends).
#[tracing::instrument(level = "debug", skip_all)]
pub fn effective_storage_root_for_request(
    request: &RuntimeStorageRequestPayload,
) -> Option<String> {
    if let Some(value) = request.storage_root.as_deref() {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    let backend_family = normalized_backend_family(&request.backend_family);
    if matches!(backend_family.as_str(), "sqlite" | "sqlite3")
        && let Some(db_path_str) = request.sqlite_db_path.as_deref() {
            let trimmed = db_path_str.trim();
            if !trimmed.is_empty()
                && let Ok(normalized) = normalize_runtime_path(trimmed)
                    && let Some(parent) = normalized.parent()
                        && !parent.as_os_str().is_empty() {
                            return Some(parent.display().to_string());
                        }
        }
    explicit_storage_root_override()
}

/// Read the explicit `ROUTER_RS_STORAGE_ROOT` override. Unlike `CODEX_HOME`
/// or `CURSOR_HOME`, this env var exists solely to point router-rs at a
/// storage root and is therefore safe to consult silently. Returns `None`
/// when the var is unset or empty.
#[tracing::instrument(level = "debug", skip_all)]
pub fn explicit_storage_root_override() -> Option<String> {
    match std::env::var("ROUTER_RS_STORAGE_ROOT") {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Err(_) => None,
    }
}
pub fn stable_memory_key(path: &Path) -> Result<String, String> {
    Ok(normalize_runtime_path(&path.display().to_string())?
        .display()
        .to_string())
}

pub fn payload_sha256(payload_text: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(payload_text.as_bytes());
    hex::encode(digest.finalize())
}

pub fn stream_sha256_hex_reader(reader: &mut impl Read) -> Result<String, std::io::Error> {
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65_536];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn stream_sha256_hex_path(path: &Path) -> Result<String, std::io::Error> {
    let mut file = fs::File::open(path)?;
    stream_sha256_hex_reader(&mut file)
}
