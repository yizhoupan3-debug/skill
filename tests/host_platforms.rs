//! Normalize `SKILL.md` `metadata.platforms` tokens to the closed host ids in
//! `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`
//! (`codex-cli`, `codex-app`, `cursor`, `claude-code`, `claude-desktop`,
//! `antigravity-cli`, `antigravity-app`, `antigravity`).
//!
//! Legacy tokens:
//! - `codex` → both `codex-cli` and `codex-app`
//! - `claude` → `claude-code` AND `claude-desktop`
//! - `supported` / `all-hosts` → every supported host id

use std::collections::BTreeSet;

/// Map frontmatter / historical tokens to canonical ids. Unknown tokens are rejected.
pub fn normalize_skill_host_platforms(
    raw: &[String],
    default_supported_hosts: &[String],
    _strict_empty_default: bool,
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
                // "claude" maps to both claude-code and claude-desktop
                out.insert("claude-code".to_string());
                out.insert("claude-desktop".to_string());
            }
            "codex-cli" | "codex-app" | "cursor" | "claude-code" | "claude-desktop"
            | "antigravity-cli" | "antigravity-app" => {
                out.insert(t);
            }
            "antigravity" => {
                out.insert("antigravity-app".to_string());
                out.insert("antigravity".to_string());
            }
            other => {
                return Err(format!(
                    "unknown host platform token `{other}` (allowed raw: supported, all-hosts, codex, codex-cli, codex-app, cursor, claude, claude-code, claude-desktop, antigravity-cli, antigravity-app, antigravity)"
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
