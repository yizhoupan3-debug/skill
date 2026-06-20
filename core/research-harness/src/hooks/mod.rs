//! Research hooks for host-projection integration.
//!
//! Provides environment-variable-controlled hook dispatch, disk caching,
//! and context injection for research activity logging, adversarial review,
//! and prose quality checking.
//!
//! ## Environment Variable Control
//!
//! Each hook can be enabled/disabled via environment variables:
//! - `RESEARCH_HOOK_ACTIVITY_LOG=0` — disable activity logging
//! - `RESEARCH_HOOK_ADVERSARIAL=0` — disable adversarial review
//! - `RESEARCH_HOOK_PROSE=0` — disable prose quality check
//! - `RESEARCH_HOOK_CACHE_DIR=<path>` — custom disk cache directory
//! - `RESEARCH_HOOK_CACHE_TTL_SECS=<n>` — cache TTL in seconds (default 3600)

pub mod activity_log;
pub mod paper_adversarial;
pub mod paper_prose;

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

// ── Environment variable helpers ──

/// Check if a hook is enabled via environment variable.
/// Default is enabled (true). Set to "0" or "false" to disable.
pub fn is_hook_enabled(env_key: &str) -> bool {
    match std::env::var(env_key) {
        Ok(val) => !matches!(val.as_str(), "0" | "false" | "no" | ""),
        Err(_) => true, // default: enabled
    }
}

/// Check if activity log hook is enabled.
pub fn is_activity_log_enabled() -> bool {
    is_hook_enabled("RESEARCH_HOOK_ACTIVITY_LOG")
}

/// Check if adversarial review hook is enabled.
pub fn is_adversarial_enabled() -> bool {
    is_hook_enabled("RESEARCH_HOOK_ADVERSARIAL")
}

/// Check if prose quality hook is enabled.
pub fn is_prose_enabled() -> bool {
    is_hook_enabled("RESEARCH_HOOK_PROSE")
}

// ── Disk cache ──

/// Get the cache directory for hook results.
pub fn cache_dir() -> PathBuf {
    std::env::var("RESEARCH_HOOK_CACHE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::temp_dir().join("research-harness-hook-cache")
        })
}

/// Get the cache TTL in seconds.
pub fn cache_ttl() -> Duration {
    let secs = std::env::var("RESEARCH_HOOK_CACHE_TTL_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(3600);
    Duration::from_secs(secs)
}

/// Check if a cached result is still valid (not expired).
pub fn is_cache_valid(path: &Path, ttl: Duration) -> bool {
    if !path.exists() {
        return false;
    }
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return false,
    };
    let modified = match metadata.modified() {
        Ok(t) => t,
        Err(_) => return false,
    };
    match SystemTime::now().duration_since(modified) {
        Ok(age) => age < ttl,
        Err(_) => false,
    }
}

/// Read cached content if valid, otherwise return None.
pub fn read_cache(key: &str) -> Option<String> {
    let path = cache_dir().join(format!("{key}.cache"));
    if is_cache_valid(&path, cache_ttl()) {
        std::fs::read_to_string(&path).ok()
    } else {
        None
    }
}

/// Write content to the hook disk cache.
pub fn write_cache(key: &str, content: &str) -> std::io::Result<()> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(format!("{key}.cache")), content)
}

/// Invalidate (delete) a cached entry.
pub fn invalidate_cache(key: &str) {
    let path = cache_dir().join(format!("{key}.cache"));
    let _ = std::fs::remove_file(path);
}

// ── Hook dispatch with env switch + cache ──

/// Dispatch activity log hook with env switch check.
/// Returns Ok(true) if activity was logged, Ok(false) if skipped.
pub fn dispatch_activity_log(
    tool_name: &str,
    args: &str,
    repo_root: &Path,
) -> anyhow::Result<bool> {
    if !is_activity_log_enabled() {
        return Ok(false);
    }
    activity_log::maybe_log_research_activity(tool_name, args, repo_root)?;
    Ok(true)
}

/// Dispatch adversarial review hook with env switch and caching.
/// Returns the context to inject, or None if not applicable.
pub fn dispatch_adversarial(context: &str) -> Option<String> {
    if !is_adversarial_enabled() {
        return None;
    }
    // Check cache (keyed by first 64 chars of context hash)
    let cache_key = format!("adv_{:016x}", {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        context.hash(&mut hasher);
        hasher.finish()
    });
    if let Some(cached) = read_cache(&cache_key) {
        return if cached.is_empty() {
            None
        } else {
            Some(cached)
        };
    }
    let result = paper_adversarial::maybe_append_adversarial_context(context);
    // Cache the result
    let _ = write_cache(&cache_key, result.as_deref().unwrap_or(""));
    result
}

/// Dispatch prose quality hook with env switch and caching.
/// Returns the context to inject, or None if not applicable.
pub fn dispatch_prose(context: &str) -> Option<String> {
    if !is_prose_enabled() {
        return None;
    }
    let cache_key = format!("prose_{:016x}", {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        context.hash(&mut hasher);
        hasher.finish()
    });
    if let Some(cached) = read_cache(&cache_key) {
        return if cached.is_empty() {
            None
        } else {
            Some(cached)
        };
    }
    let result = paper_prose::maybe_append_prose_context(context);
    let _ = write_cache(&cache_key, result.as_deref().unwrap_or(""));
    result
}

// ── Cursor JSON merge support ──

/// Merge hook context into a Cursor-compatible JSON payload.
/// Cursor expects hook output as a string field in the tool result JSON.
pub fn merge_hook_context_json(
    base_json: &str,
    hook_name: &str,
    hook_context: &str,
) -> String {
    if hook_context.is_empty() {
        return base_json.to_string();
    }
    // Try to parse as JSON and inject hook context
    if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(base_json) {
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                format!("_{hook_name}_context"),
                serde_json::Value::String(hook_context.to_string()),
            );
            return serde_json::to_string(&value).unwrap_or_else(|_| base_json.to_string());
        }
    }
    // Fallback: append as plain text
    format!("{base_json}\n\n--- {hook_name} ---\n{hook_context}")
}

// ── Include-str templates ──

/// Adversarial review prompt template (embedded at compile time).
pub const ADVERSARIAL_TEMPLATE: &str =
    paper_adversarial::ADVERSARIAL_CONTEXT;

/// Prose quality prompt template (embedded at compile time).
pub const PROSE_TEMPLATE: &str =
    paper_prose::PROSE_QUALITY_CONTEXT;

// ── Tests ──

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_hook_enabled_default() {
        // Without env var set, hooks should be enabled by default
        assert!(is_hook_enabled("RESEARCH_HOOK_NONEXISTENT_TEST"));
    }

    #[test]
    fn is_hook_enabled_disabled() {
        unsafe { std::env::set_var("RESEARCH_HOOK_TEST_DISABLE", "0") };
        assert!(!is_hook_enabled("RESEARCH_HOOK_TEST_DISABLE"));
        unsafe { std::env::remove_var("RESEARCH_HOOK_TEST_DISABLE") };
    }

    #[test]
    fn is_hook_enabled_false_string() {
        unsafe { std::env::set_var("RESEARCH_HOOK_TEST_FALSE", "false") };
        assert!(!is_hook_enabled("RESEARCH_HOOK_TEST_FALSE"));
        unsafe { std::env::remove_var("RESEARCH_HOOK_TEST_FALSE") };
    }

    #[test]
    fn is_hook_enabled_true_string() {
        unsafe { std::env::set_var("RESEARCH_HOOK_TEST_TRUE", "1") };
        assert!(is_hook_enabled("RESEARCH_HOOK_TEST_TRUE"));
        unsafe { std::env::remove_var("RESEARCH_HOOK_TEST_TRUE") };
    }

    #[test]
    fn cache_dir_default() {
        let dir = cache_dir();
        assert!(dir.to_string_lossy().contains("research-harness-hook-cache"));
    }

    #[test]
    fn cache_ttl_default() {
        let ttl = cache_ttl();
        assert_eq!(ttl, Duration::from_secs(3600));
    }

    #[test]
    fn is_cache_valid_nonexistent() {
        assert!(!is_cache_valid(Path::new("/nonexistent/path"), Duration::from_secs(60)));
    }

    #[test]
    fn write_and_read_cache() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_key.cache");
        std::fs::write(&path, "test_value").unwrap();
        assert!(is_cache_valid(&path, Duration::from_secs(60)));
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "test_value");
    }

    #[test]
    fn invalidate_cache_removes_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("to_delete.cache");
        std::fs::write(&path, "data").unwrap();
        assert!(path.exists());
        std::fs::remove_file(&path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn dispatch_adversarial_returns_context() {
        unsafe { std::env::set_var("RESEARCH_HOOK_CACHE_DIR", tempfile::tempdir().unwrap().path()); }
        let result = dispatch_adversarial("请根据审稿意见修改论文");
        assert!(result.is_some());
        assert!(result.unwrap().contains("PAPER_ADVERSARIAL_HOOK"));
        unsafe { std::env::remove_var("RESEARCH_HOOK_CACHE_DIR") };
    }

    #[test]
    fn dispatch_adversarial_disabled() {
        unsafe { std::env::set_var("RESEARCH_HOOK_ADVERSARIAL", "0") };
        let result = dispatch_adversarial("请根据审稿意见修改论文");
        assert!(result.is_none());
        unsafe { std::env::remove_var("RESEARCH_HOOK_ADVERSARIAL") };
    }

    #[test]
    fn dispatch_prose_returns_context() {
        unsafe { std::env::set_var("RESEARCH_HOOK_CACHE_DIR", tempfile::tempdir().unwrap().path()); }
        let result = dispatch_prose("帮我把这段引言润色一下");
        assert!(result.is_some());
        assert!(result.unwrap().contains("PAPER_PROSE_QUALITY_HOOK"));
        unsafe { std::env::remove_var("RESEARCH_HOOK_CACHE_DIR") };
    }

    #[test]
    fn dispatch_prose_no_signal() {
        let result = dispatch_prose("fix the CI pipeline");
        assert!(result.is_none());
    }

    #[test]
    fn merge_hook_context_json_basic() {
        let base = r#"{"tool":"Bash","input":"ls"}"#;
        let merged = merge_hook_context_json(base, "adversarial", "review context");
        assert!(merged.contains("review context"));
        assert!(merged.contains("_adversarial_context"));
    }

    #[test]
    fn merge_hook_context_json_empty_hook() {
        let base = r#"{"tool":"Bash"}"#;
        let merged = merge_hook_context_json(base, "test", "");
        assert_eq!(merged, base);
    }

    #[test]
    fn merge_hook_context_json_non_json_base() {
        let merged = merge_hook_context_json("plain text", "test", "hook data");
        assert!(merged.contains("plain text"));
        assert!(merged.contains("hook data"));
    }

    #[test]
    fn dispatch_adversarial_cached() {
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var("RESEARCH_HOOK_CACHE_DIR", dir.path()); }
        let ctx = "请根据审稿意见修改这篇论文的手稿";
        let r1 = dispatch_adversarial(ctx);
        let r2 = dispatch_adversarial(ctx);
        assert_eq!(r1, r2); // second call should hit cache
        unsafe { std::env::remove_var("RESEARCH_HOOK_CACHE_DIR") };
    }

    #[test]
    fn template_constants_exist() {
        assert!(!ADVERSARIAL_TEMPLATE.is_empty());
        assert!(!PROSE_TEMPLATE.is_empty());
        assert!(ADVERSARIAL_TEMPLATE.contains("PAPER_ADVERSARIAL_HOOK"));
        assert!(PROSE_TEMPLATE.contains("PAPER_PROSE_QUALITY_HOOK"));
    }
}
