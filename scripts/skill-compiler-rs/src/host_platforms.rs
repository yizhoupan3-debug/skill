//! Normalize `SKILL.md` `metadata.platforms` tokens to the closed host ids in
//! `configs/framework/RUNTIME_REGISTRY.json` → `host_targets.supported`
//! (`codex-cli`, `codex-app`, `cursor`, `claude-code`).
//!
//! Legacy tokens used in skill frontmatter:
//! - `codex` → both `codex-cli` and `codex-app`
//! - `claude` → `claude-code`

use std::collections::BTreeSet;

/// Map frontmatter / historical tokens to canonical ids. Unknown tokens are rejected.
pub fn normalize_skill_host_platforms(raw: &[String]) -> Result<Vec<String>, String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for s in raw {
        let t = s.trim().to_ascii_lowercase();
        if t.is_empty() {
            continue;
        }
        match t.as_str() {
            "codex" => {
                out.insert("codex-cli".to_string());
                out.insert("codex-app".to_string());
            }
            "claude" => {
                out.insert("claude-code".to_string());
            }
            "codex-cli" | "codex-app" | "cursor" | "claude-code" => {
                out.insert(t);
            }
            other => {
                return Err(format!(
                    "unknown host platform token `{other}` (allowed raw: codex, codex-cli, codex-app, cursor, claude, claude-code)"
                ));
            }
        }
    }
    if out.is_empty() {
        out.insert("codex-cli".to_string());
        out.insert("codex-app".to_string());
    }
    Ok(out.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_empty_maps_to_codex_cli_and_app() {
        let v = normalize_skill_host_platforms(&[]).expect("ok");
        assert_eq!(v, vec!["codex-app", "codex-cli"]);
    }

    #[test]
    fn codex_token_expands() {
        let v = normalize_skill_host_platforms(&["codex".to_string()]).expect("ok");
        assert_eq!(v, vec!["codex-app", "codex-cli"]);
    }

    #[test]
    fn codex_and_cursor_merge_sorted() {
        let v = normalize_skill_host_platforms(&["codex".to_string(), "cursor".to_string()])
            .expect("ok");
        assert_eq!(v, vec!["codex-app", "codex-cli", "cursor"]);
    }

    #[test]
    fn claude_alias() {
        let v = normalize_skill_host_platforms(&["claude".to_string()]).expect("ok");
        assert_eq!(v, vec!["claude-code"]);
    }

    #[test]
    fn rejects_unknown() {
        assert!(normalize_skill_host_platforms(&["vscode".to_string()]).is_err());
    }
}
