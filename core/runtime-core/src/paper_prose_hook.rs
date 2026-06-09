//! 四宿主 UserPromptSubmit / Cursor `beforeSubmit`：手稿**写作/润色**类提示合并 **prose quality chain** 短段。
//!
//! 文案真源：`configs/framework/PAPER_PROSE_QUALITY_HOOK.txt`（`include_str!` 单轨）。
//! - per-host env：**默认开**；`0`/`false`/`off`/`no` 关闭。
//! - 受 `ROUTER_RS_OPERATOR_INJECT` 总闸约束。

use crate::router_env_flags::{
    router_rs_env_enabled_default_true, router_rs_operator_inject_globally_enabled,
};
use crate::route::{has_paper_prose_edit_context, tokenize_route_text};
use serde_json::Value;
use std::fs;
use std::path::Path;

const REL_PATH: &str = "configs/framework/PAPER_PROSE_QUALITY_HOOK.txt";
pub const PREFIX_LINE: &str = "**PAPER_PROSE_QUALITY_HOOK**";

const BUILTIN_TXT: &str = include_str!("../../../configs/framework/PAPER_PROSE_QUALITY_HOOK.txt");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaperProseHookHost {
    Cursor,
    Codex,
    Claude,
}

impl PaperProseHookHost {
    pub fn env_var(self) -> &'static str {
        match self {
            Self::Cursor => "ROUTER_RS_CURSOR_PAPER_PROSE_HOOK",
            Self::Codex => "ROUTER_RS_CODEX_PAPER_PROSE_HOOK",
            Self::Claude => "ROUTER_RS_CLAUDE_PAPER_PROSE_HOOK",
        }
    }

    pub fn from_codex_lifecycle_state_dir(_state_dir_leaf: &str) -> Self {
        Self::Codex
    }
}

fn builtin_block() -> String {
    BUILTIN_TXT.trim().to_string()
}

pub fn paper_prose_hook_requested(host: PaperProseHookHost) -> bool {
    router_rs_operator_inject_globally_enabled()
        && router_rs_env_enabled_default_true(host.env_var())
}

pub fn cursor_paper_prose_hook_requested() -> bool {
    paper_prose_hook_requested(PaperProseHookHost::Cursor)
}

/// 主动触发：与 NL `has_paper_prose_edit_context` 单真源。
pub fn prompt_signals_paper_prose_work(text: &str) -> bool {
    let tokens = tokenize_route_text(text);
    has_paper_prose_edit_context(text, &tokens)
}

pub fn resolve_paper_prose_block(repo_root: &Path) -> String {
    let path = repo_root.join(REL_PATH);
    match fs::read_to_string(&path) {
        Ok(t) => {
            let trimmed = t.trim();
            if trimmed.is_empty() {
                return builtin_block();
            }
            if let Some(after) = trimmed.strip_prefix(PREFIX_LINE) {
                let after = after.trim();
                if after.is_empty() {
                    return builtin_block();
                }
                return trimmed.to_string();
            }
            format!("{PREFIX_LINE}\n\n{trimmed}")
        }
        Err(_) => builtin_block(),
    }
}

pub fn maybe_append_paper_prose_context(
    repo_root: &Path,
    prompt_text: &str,
    contexts: &mut Vec<String>,
    host: PaperProseHookHost,
) {
    if !paper_prose_hook_requested(host) || !prompt_signals_paper_prose_work(prompt_text) {
        return;
    }
    let msg = resolve_paper_prose_block(repo_root);
    if msg.trim().is_empty() {
        return;
    }
    contexts.push(msg);
}

pub fn maybe_merge_paper_prose_before_submit(
    repo_root: &Path,
    output: &mut Value,
    prompt_text: &str,
    use_followup_message: bool,
) {
    if !cursor_paper_prose_hook_requested() || !prompt_signals_paper_prose_work(prompt_text) {
        return;
    }
    let msg = resolve_paper_prose_block(repo_root);
    if msg.trim().is_empty() {
        return;
    }
    crate::autopilot_goal::merge_hook_nudge_paragraph(
        output,
        &msg,
        PREFIX_LINE,
        use_followup_message,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn signal_polish_zh() {
        assert!(prompt_signals_paper_prose_work("帮我把这段引言润色一下，中文正文"));
    }

    #[test]
    fn signal_colloquial_edit_without_polish_keyword() {
        assert!(prompt_signals_paper_prose_work(
            "论文讨论节这段读起来不通顺，帮我改改"
        ));
    }

    #[test]
    fn signal_pasted_latex_with_paper_context() {
        assert!(prompt_signals_paper_prose_work(
            "论文 改一下下面这段 \\begin{abstract} We propose a method \\cite{foo}"
        ));
    }

    #[test]
    fn signal_negative_ci_only() {
        assert!(!prompt_signals_paper_prose_work(
            "fix cargo test in pull request workflow"
        ));
    }

    #[test]
    fn signal_negative_abstract_base_class() {
        assert!(!prompt_signals_paper_prose_work(
            "edit the abstract base class in this Java module"
        ));
    }

    #[test]
    fn signal_polish_abstract_matches_nl() {
        assert!(prompt_signals_paper_prose_work("polish this abstract"));
    }

    #[test]
    fn builtin_embedded_disk_txt() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("repo root")
            .to_path_buf();
        let on_disk = std::fs::read_to_string(repo_root.join(REL_PATH)).expect("readable");
        assert_eq!(builtin_block(), on_disk.trim());
        assert!(builtin_block().contains("language_register"));
    }

    #[test]
    fn merge_when_enabled_by_default_unset() {
        let _g = crate::harness_operator_nudges::harness_nudges_env_test_lock();
        let env = PaperProseHookHost::Cursor.env_var();
        let prior_hook = std::env::var(env).ok();
        std::env::remove_var(env);

        let tmp = std::env::temp_dir().join("paper-prose-merge-default");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("configs/framework")).unwrap();
        std::fs::write(
            tmp.join(REL_PATH),
            format!("{PREFIX_LINE}\n\nprose test body"),
        )
        .unwrap();

        assert!(cursor_paper_prose_hook_requested());
        let mut out = json!({ "continue": true });
        maybe_merge_paper_prose_before_submit(&tmp, &mut out, "英文论文润色 abstract", false);
        let ctx = out
            .get("additional_context")
            .and_then(Value::as_str)
            .unwrap_or("");
        assert!(ctx.contains(PREFIX_LINE));

        match prior_hook {
            Some(v) => std::env::set_var(env, v),
            None => std::env::remove_var(env),
        }
    }

    #[test]
    fn merge_skips_when_hook_explicitly_off() {
        let _g = crate::harness_operator_nudges::harness_nudges_env_test_lock();
        let env = PaperProseHookHost::Cursor.env_var();
        let prior_hook = std::env::var(env).ok();
        std::env::set_var(env, "0");

        let tmp = std::env::temp_dir().join("paper-prose-merge-off");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("configs/framework")).unwrap();
        std::fs::write(tmp.join(REL_PATH), format!("{PREFIX_LINE}\n\n正文。")).unwrap();

        assert!(!cursor_paper_prose_hook_requested());
        let mut out = json!({ "continue": true });
        maybe_merge_paper_prose_before_submit(&tmp, &mut out, "SCI润色 abstract", false);
        assert!(out.get("additional_context").is_none());

        match prior_hook {
            Some(v) => std::env::set_var(env, v),
            None => std::env::remove_var(env),
        }
    }

    #[test]
    fn append_context_codex_host() {
        let _g = crate::harness_operator_nudges::harness_nudges_env_test_lock();
        let env = PaperProseHookHost::Codex.env_var();
        let prior_hook = std::env::var(env).ok();
        std::env::remove_var(env);

        let tmp = std::env::temp_dir().join("paper-prose-append-codex");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("configs/framework")).unwrap();
        std::fs::write(
            tmp.join(REL_PATH),
            format!("{PREFIX_LINE}\n\ncodex prose body"),
        )
        .unwrap();

        let mut contexts = Vec::new();
        maybe_append_paper_prose_context(
            &tmp,
            "SCI润色 abstract",
            &mut contexts,
            PaperProseHookHost::Codex,
        );
        assert_eq!(contexts.len(), 1);
        assert!(contexts[0].contains(PREFIX_LINE));

        match prior_hook {
            Some(v) => std::env::set_var(env, v),
            None => std::env::remove_var(env),
        }
    }
}
