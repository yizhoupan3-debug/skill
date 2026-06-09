use router_rs::task_state::resolve_task_view;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Cache entry for framework_snapshot responses (30 second TTL).
pub struct SnapshotCache {
    pub content: String,
    pub expires_at: Instant,
}

impl SnapshotCache {
    pub fn is_valid(&self) -> bool {
        Instant::now() < self.expires_at
    }
}

/// Rate limiter state for tool call frequency control.
pub struct RateLimiter {
    last_call: HashMap<String, Instant>,
    min_interval: Duration,
}

impl RateLimiter {
    pub fn new(min_interval_ms: u64) -> Self {
        RateLimiter {
            last_call: HashMap::new(),
            min_interval: Duration::from_millis(min_interval_ms),
        }
    }

    pub fn check_and_record(&mut self, tool_name: &str) -> Result<(), String> {
        let now = Instant::now();
        if let Some(last) = self.last_call.get(tool_name) {
            if now.duration_since(*last) < self.min_interval {
                return Err(format!(
                    "Rate limit exceeded for {}. Wait {}ms between calls.",
                    tool_name,
                    self.min_interval.as_millis()
                ));
            }
        }
        self.last_call.insert(tool_name.to_string(), now);
        Ok(())
    }
}

// Global caches and rate limiter (session-scoped via OnceLock)
static SNAPSHOT_CACHE: OnceLock<std::sync::Mutex<Option<SnapshotCache>>> = OnceLock::new();
static TASK_VIEW_CACHE: OnceLock<
    std::sync::Mutex<Option<(router_rs::task_state::ResolvedTaskView, Instant)>>,> = OnceLock::new();
static RATE_LIMITER: OnceLock<std::sync::Mutex<RateLimiter>> = OnceLock::new();

/// Poison-safe lock helper that recovers from mutex poisoning.
/// Returns the guard, or None if lock acquisition failed.
macro_rules! poison_safe_lock {
    ($mutex:expr) => {{
        match $mutex.lock() {
            Ok(guard) => Some(guard),
            Err(poisoned) => {
                eprintln!(
                    "[router-rs warning] mutex poisoned, recovering (thread panicked while holding lock)"
                );
                Some(poisoned.into_inner())
            }
        }
    }};
}

pub fn get_snapshot_cache() -> &'static std::sync::Mutex<Option<SnapshotCache>> {
    SNAPSHOT_CACHE.get_or_init(|| std::sync::Mutex::new(None))
}

pub fn get_task_view_cache(
) -> &'static std::sync::Mutex<Option<(router_rs::task_state::ResolvedTaskView, Instant)>> {
    TASK_VIEW_CACHE.get_or_init(|| std::sync::Mutex::new(None))
}

pub fn get_rate_limiter() -> &'static std::sync::Mutex<RateLimiter> {
    RATE_LIMITER.get_or_init(|| {
        let interval = if cfg!(test) { 0 } else { 100 };
        std::sync::Mutex::new(RateLimiter::new(interval))
    })
}

/// Get snapshot cache TTL from environment variable.
/// Default: 30 seconds. Env: ROUTER_RS_DESKTOP_SNAPSHOT_CACHE_TTL_SECS
pub fn snapshot_cache_ttl_secs() -> u64 {
    static CACHED: OnceLock<u64> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("ROUTER_RS_DESKTOP_SNAPSHOT_CACHE_TTL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(30)
    })
}

/// Get task view cache TTL from environment variable.
/// Default: 5 seconds. Env: ROUTER_RS_DESKTOP_TASK_VIEW_CACHE_TTL_SECS
fn task_view_cache_ttl_secs() -> u64 {
    static CACHED: OnceLock<u64> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("ROUTER_RS_DESKTOP_TASK_VIEW_CACHE_TTL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(5)
    })
}

/// Get cached task view with configurable TTL (default 5 seconds).
pub fn get_cached_task_view(repo_root: &Path) -> router_rs::task_state::ResolvedTaskView {
    let ttl_secs = task_view_cache_ttl_secs();
    {
        let cache = get_task_view_cache();
        if let Some(guard) = poison_safe_lock!(cache) {
            if let Some((ref view, ref expires_at)) = *guard {
                if Instant::now() < *expires_at {
                    return view.clone();
                }
            }
        }
    }

    // Cache miss: recompute
    let view = resolve_task_view(repo_root, None);

    // Update cache with configurable TTL
    {
        let cache = get_task_view_cache();
        if let Some(mut guard) = poison_safe_lock!(cache) {
            *guard = Some((view.clone(), Instant::now() + Duration::from_secs(ttl_secs)));
        }
    }

    view
}
