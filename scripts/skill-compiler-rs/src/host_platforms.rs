//! Normalize `SKILL.md` `metadata.platforms` tokens to the closed host ids in
//! `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`
//! (`codex-cli`, `codex-app`, `cursor`, `claude-code`, `qoder`).
//!
//! Legacy tokens used in skill frontmatter:
//! - `codex` → both `codex-cli` and `codex-app`
//! - `claude` → `claude-code`
//! - `supported` / `all-hosts` → every id in `default_supported_hosts` (from registry at compile time)

use serde_json::Value;
use std::collections::BTreeSet;

/// Parse `RUNTIME_REGISTRY.json` text and return sorted `host_targets.supported`.
pub fn supported_hosts_from_registry_text(registry_text: &str) -> Result<Vec<String>, String> {
    let v: Value =
        serde_json::from_str(registry_text).map_err(|e| format!("RUNTIME_REGISTRY json: {e}"))?;
    let arr = v
        .get("host_targets")
        .and_then(|h| h.get("supported"))
        .and_then(|s| s.as_array())
        .ok_or_else(|| "RUNTIME_REGISTRY.host_targets.supported missing".to_string())?;
    let mut out: Vec<String> = arr
        .iter()
        .filter_map(|x| x.as_str().map(str::to_string))
        .collect();
    if out.is_empty() {
        return Err("host_targets.supported empty".to_string());
    }
    out.sort();
    Ok(out)
}

/// Map frontmatter / historical tokens to canonical ids. Unknown tokens are rejected.
///
/// When `raw` is empty or only whitespace tokens, expands to **all** `default_supported_hosts`.
/// `default_supported_hosts` must be non-empty (typically loaded from `RUNTIME_REGISTRY.json`).
/// When `strict_empty_default` is true, **empty** `metadata.platforms` (no tokens after trim)
/// is rejected instead of expanding to all `default_supported_hosts`.
pub fn normalize_skill_host_platforms(
    raw: &[String],
    default_supported_hosts: &[String],
    strict_empty_default: bool,
) -> Result<Vec<String>, String> {
    if default_supported_hosts.is_empty() {
        return Err("default_supported_hosts must be non-empty".to_string());
    }
    let mut out: BTreeSet<String> = BTreeSet::new();
    for s in raw {
        let t = s.trim().to_ascii_lowercase();
        if t.is_empty() {
            continue;
        }
        match t.as_str() {
            "supported" | "all-hosts" => {
                for h in default_supported_hosts {
                    out.insert(h.clone());
                }
            }
            "codex" => {
                out.insert("codex-cli".to_string());
                out.insert("codex-app".to_string());
            }
            "claude" => {
                out.insert("claude-code".to_string());
            }
            "codex-cli" | "codex-app" | "cursor" | "claude-code" | "qoder" => {
                out.insert(t);
            }
            other => {
                return Err(format!(
                    "unknown host platform token `{other}` (allowed raw: supported, all-hosts, codex, codex-cli, codex-app, cursor, claude, claude-code, qoder)"
                ));
            }
        }
    }
    if out.is_empty() {
        if strict_empty_default {
            return Err(
                "strict_host_platforms: empty metadata.platforms is not allowed; list host ids or use [supported] / all-hosts"
                    .to_string(),
            );
        }
        for h in default_supported_hosts {
            out.insert(h.clone());
        }
    }
    Ok(out.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_supported() -> Vec<String> {
        vec![
            "claude-code".to_string(),
            "codex-app".to_string(),
            "codex-cli".to_string(),
            "cursor".to_string(),
            "qoder".to_string(),
        ]
    }

    #[test]
    fn default_empty_maps_to_all_supported() {
        let sup = sample_supported();
        let v = normalize_skill_host_platforms(&[], &sup, false).expect("ok");
        assert_eq!(v, sup);
    }

    #[test]
    fn strict_empty_rejects_default_expansion() {
        let sup = sample_supported();
        let err = normalize_skill_host_platforms(&[], &sup, true).expect_err("strict");
        assert!(
            err.contains("strict_host_platforms"),
            "unexpected err: {err}"
        );
    }

    #[test]
    fn supported_token_expands() {
        let sup = sample_supported();
        let v =
            normalize_skill_host_platforms(&["supported".to_string()], &sup, false).expect("ok");
        assert_eq!(v, sup);
    }

    #[test]
    fn all_hosts_token_expands() {
        let sup = sample_supported();
        let v =
            normalize_skill_host_platforms(&["all-hosts".to_string()], &sup, false).expect("ok");
        assert_eq!(v, sup);
    }

    #[test]
    fn codex_token_expands() {
        let sup = sample_supported();
        let v = normalize_skill_host_platforms(&["codex".to_string()], &sup, false).expect("ok");
        assert_eq!(v, vec!["codex-app", "codex-cli"]);
    }

    #[test]
    fn codex_and_cursor_merge_sorted() {
        let sup = sample_supported();
        let v = normalize_skill_host_platforms(
            &["codex".to_string(), "cursor".to_string()],
            &sup,
            false,
        )
        .expect("ok");
        assert_eq!(v, vec!["codex-app", "codex-cli", "cursor"]);
    }

    #[test]
    fn claude_alias() {
        let sup = sample_supported();
        let v = normalize_skill_host_platforms(&["claude".to_string()], &sup, false).expect("ok");
        assert_eq!(v, vec!["claude-code"]);
    }

    #[test]
    fn rejects_unknown() {
        let sup = sample_supported();
        assert!(normalize_skill_host_platforms(&["vscode".to_string()], &sup, false).is_err());
    }

    #[test]
    fn parses_registry_supported() {
        let text = r#"{"host_targets":{"supported":["cursor","codex-cli","claude-code","codex-app","qoder"]}}"#;
        let v = supported_hosts_from_registry_text(text).expect("ok");
        assert_eq!(
            v,
            vec!["claude-code", "codex-app", "codex-cli", "cursor", "qoder"]
        );
    }
}
