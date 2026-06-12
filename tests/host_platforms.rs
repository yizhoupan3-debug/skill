//! Normalize `SKILL.md` `metadata.platforms` tokens to the closed host ids in
//! `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`
//! (`codex`, `claude-code`, `cursor`, `opencode`).
//!
//! Legacy tokens:
//! - `claude` → `claude-code`
//! - `codex-cli` → `codex` (deprecated alias)
//! - `codex-app` → retired (rejected)
//! - `supported` / `all-hosts` → every supported host id

use std::collections::BTreeSet;

/// Map frontmatter / historical tokens to canonical ids. Unknown tokens are rejected.
pub fn normalize_skill_host_platforms(
    raw: &[String],
    default_supported_hosts: &[String],

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
                out.insert("codex".to_string());
            }
            "codex-cli" => {
                out.insert("codex".to_string());
            }
            "codex-app" => {
                return Err(format!(
                    "retired host platform token `{t}` (closed-set ids: codex, cursor, claude-code, opencode)"
                ));
            }
            "claude" => {
                out.insert("claude-code".to_string());
            }
            "cursor" | "claude-code" | "opencode" => {
                out.insert(t);
            }
            other => {
                return Err(format!(
                    "unknown host platform token `{other}` (allowed raw: supported, all-hosts, codex, codex-cli, cursor, claude, claude-code, opencode)"
                ));
            }
        }
    }
    if out.is_empty() {
        // Default: expand to all supported hosts
        for h in default_supported_hosts {
            out.insert(h.clone());
        }
    }
    Ok(out.into_iter().collect())
}
