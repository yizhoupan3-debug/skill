//! E7 migration contract — which hosts may bypass [`super::host_hook::HostHook`] default methods.
//!
//! New hosts **must not** override `run_cli_hook`, `dispatch`, or `read_stdin_payload` unless
//! their `host_id()` is listed in the corresponding legacy allowlist below. Step 2 (handler
//! extraction) removes entries from these lists.

/// Host ids temporarily allowed to override [`super::host_hook::HostHook::run_cli_hook`].
#[cfg_attr(not(test), allow(dead_code))]
pub const LEGACY_RUN_CLI_HOOK_OVERRIDE_HOSTS: &[&str] = &[];

/// Host ids temporarily allowed to override [`super::host_hook::HostHook::read_stdin_payload`].
#[cfg_attr(not(test), allow(dead_code))]
pub const LEGACY_READ_STDIN_OVERRIDE_HOSTS: &[&str] = &[];

/// Host ids temporarily allowed to override [`super::host_hook::HostHook::dispatch`].
#[cfg_attr(not(test), allow(dead_code))]
pub const LEGACY_DISPATCH_OVERRIDE_HOSTS: &[&str] = &[];

#[cfg(test)]
mod tests {
    use super::*;

    fn host_impl_declares_method(src: &str, method: &str) -> bool {
        src.contains(&format!("fn {method}("))
    }

    #[test]
    fn only_allowlisted_hosts_may_override_run_cli_hook() {
        let hosts: [(&str, &str); 3] = [
            ("claude", include_str!("claude_hook_host.rs")),
            ("cursor", include_str!("cursor_hook_host.rs")),
            ("codex", include_str!("codex_hook_host.rs")),
        ];
        for (host_id, src) in hosts {
            let overrides = host_impl_declares_method(src, "run_cli_hook");
            let allowed = LEGACY_RUN_CLI_HOOK_OVERRIDE_HOSTS.contains(&host_id);
            assert_eq!(
                overrides, allowed,
                "host {host_id}: run_cli_hook override={overrides}, allowlisted={allowed}"
            );
        }
        assert!(
            !host_impl_declares_method(include_str!("host_hook_example.rs"), "run_cli_hook"),
            "example host must use default run_cli_hook"
        );
    }

    #[test]
    fn no_host_overrides_dispatch_or_read_stdin_without_allowlist() {
        let hosts: [(&str, &str); 4] = [
            ("claude", include_str!("claude_hook_host.rs")),
            ("cursor", include_str!("cursor_hook_host.rs")),
            ("codex", include_str!("codex_hook_host.rs")),
            ("example", include_str!("host_hook_example.rs")),
        ];
        for (host_id, src) in &hosts {
            for method in ["dispatch", "read_stdin_payload"] {
                let overrides = host_impl_declares_method(src, method);
                let allowed = match method {
                    "dispatch" => LEGACY_DISPATCH_OVERRIDE_HOSTS.contains(host_id),
                    "read_stdin_payload" => LEGACY_READ_STDIN_OVERRIDE_HOSTS.contains(host_id),
                    _ => false,
                };
                assert_eq!(
                    overrides, allowed,
                    "host {host_id}: {method} override={overrides}, allowlisted={allowed}"
                );
            }
        }
    }
}
