//! Prose 质量 hook — 为论文编辑场景追加语言质量检查上下文。
//!
//! 文案真源：`configs/framework/PAPER_PROSE_QUALITY_HOOK.txt`（`include_str!` 单轨）。
//! - per-host env：默认开；`0`/`false`/`off`/`no` 关闭。
//! - 受 `ROUTER_RS_OPERATOR_INJECT` 总闸约束。

use crate::hooks::paper_block_cache::BlockCache;
use serde_json::Value;
use std::path::Path;

// ── Constants ──

const REL_PATH: &str = "configs/framework/PAPER_PROSE_QUALITY_HOOK.txt";
pub const PREFIX_LINE: &str = "**PAPER_PROSE_QUALITY_HOOK**";

const BUILTIN_TXT: &str =
    include_str!("../../../../configs/framework/PAPER_PROSE_QUALITY_HOOK.txt");

// ── Static state ──

static BUILTIN_BLOCK: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| BUILTIN_TXT.trim().to_string());

/// Access the compiled-in builtin block directly (no disk lookup).
pub fn builtin_block() -> String {
    BUILTIN_BLOCK.clone()
}

static BLOCK_CACHE: BlockCache = BlockCache::new(REL_PATH, PREFIX_LINE, "paper prose");

// ── Per-host environment variable mapping ──

/// Import per-host env var name resolution from host-projection (L0).
use host_projection::hooks::paper_prose_env_var;

// ── Public API ──

/// Check whether the prose hook is requested for a given host.
pub fn paper_prose_hook_requested(host: &str) -> bool {
    super::paper_common::operator_inject_globally_enabled()
        && framework_core::env_flags::env_enabled_default_true(paper_prose_env_var(host))
}

/// 信号检测：用户提示是否涉及论文写作/润色。
pub fn prompt_signals_prose_work(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();

    // 工程噪声过滤：abstract base class / abstract class 是 OOP 术语
    if lower.contains("abstract base class") || lower.contains("abstract class") {
        return false;
    }

    // 强信号：论文写作/润色
    static STRONG_ZH: &[&str] = &[
        "润色",
        "改稿",
        "论文",
        "手稿",
        "引言",
        "摘要",
        "讨论节",
        "结论节",
        "相关工作",
        "方法论",
    ];
    if STRONG_ZH.iter().any(|k| text.contains(k)) {
        return true;
    }

    static STRONG_EN: &[&str] = &[
        "polish",
        "manuscript",
        "abstract",
        "introduction",
        "discussion",
        "related work",
        "methodology",
    ];
    if STRONG_EN.iter().any(|k| lower.contains(k)) {
        return true;
    }

    // LaTeX 片段信号
    if text.contains("\\begin{abstract}") || text.contains("\\cite{") {
        return true;
    }

    false
}

/// Resolve the prose hook block from disk or builtin.
pub fn resolve_paper_prose_block(repo_root: &Path) -> String {
    BLOCK_CACHE.resolve(repo_root, || BUILTIN_BLOCK.clone())
}

/// Append prose hook context if the hook is enabled and the prompt signals prose work.
pub fn maybe_append_paper_prose_context(
    repo_root: &Path,
    prompt_text: &str,
    contexts: &mut Vec<String>,
    host: &str,
) {
    super::paper_common::maybe_append_context(
        paper_prose_hook_requested(host),
        prompt_signals_prose_work(prompt_text),
        repo_root,
        &BLOCK_CACHE,
        &BUILTIN_BLOCK,
        contexts,
    );
}

/// Merge prose hook context into Cursor-compatible JSON output.
pub fn maybe_merge_paper_prose_before_submit(
    repo_root: &Path,
    output: &mut Value,
    prompt_text: &str,
    use_followup_message: bool,
    host: &str,
) {
    super::paper_common::maybe_merge_context(
        paper_prose_hook_requested(host),
        prompt_signals_prose_work(prompt_text),
        repo_root,
        &BLOCK_CACHE,
        &BUILTIN_BLOCK,
        output,
        PREFIX_LINE,
        use_followup_message,
    );
}

/// 简单版：在检测到论文编辑相关操作时，追加 prose 质量检查上下文片段。
/// 返回 `None` 表示不追加。
pub fn maybe_append_prose_context(context: &str) -> Option<String> {
    if !prompt_signals_prose_work(context) {
        return None;
    }
    Some(BUILTIN_BLOCK.clone())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use serde_json::json;
    use std::sync::{Mutex, OnceLock};

    fn env_test_lock() -> &'static Mutex<()> {
        static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        TEST_LOCK.get_or_init(|| Mutex::new(()))
    }

    fn restore_env(key: &str, prior: Option<String>) {
        match prior {
            // SAFETY: test-only; env_test_lock() prevents concurrent env access from other tests.
            Some(v) => unsafe { core_state_utils::env_sync::set_env(key, &v) },
            // SAFETY: test-only; env_test_lock() prevents concurrent env access from other tests.
            None => unsafe { core_state_utils::env_sync::remove_env(key) },
        }
    }

    #[test]
    fn signal_polish_zh() {
        assert!(prompt_signals_prose_work(
            "帮我把这段引言润色一下，中文正文"
        ));
    }

    #[test]
    fn signal_colloquial_edit_without_polish_keyword() {
        assert!(prompt_signals_prose_work(
            "论文讨论节这段读起来不通顺，帮我改改"
        ));
    }

    #[test]
    fn signal_pasted_latex_with_paper_context() {
        assert!(prompt_signals_prose_work(
            "论文 改一下下面这段 \\begin{abstract} We propose a method \\cite{foo}"
        ));
    }

    #[test]
    fn signal_negative_ci_only() {
        assert!(!prompt_signals_prose_work(
            "fix cargo test in pull request workflow"
        ));
    }

    #[test]
    fn signal_negative_abstract_base_class() {
        assert!(!prompt_signals_prose_work(
            "edit the abstract base class in this Java module"
        ));
    }

    #[test]
    fn signal_polish_abstract_matches_nl() {
        assert!(prompt_signals_prose_work("polish this abstract"));
    }

    #[test]
    fn maybe_append_prose_returns_context() {
        let result = maybe_append_prose_context("帮我把这段引言润色一下");
        assert!(result.is_some());
        assert!(result.unwrap().contains(PREFIX_LINE));
    }

    #[test]
    fn maybe_append_prose_no_signal_returns_none() {
        let result = maybe_append_prose_context("fix the CI pipeline");
        assert!(result.is_none());
    }

    #[test]
    fn builtin_embedded_disk_txt() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("repo root")
            .to_path_buf();
        let on_disk = std::fs::read_to_string(repo_root.join(REL_PATH)).expect("readable");
        assert_eq!(BUILTIN_BLOCK.clone(), on_disk.trim());
        assert!(BUILTIN_BLOCK.contains("language_register"));
    }

    #[test]
    fn merge_when_enabled_by_default_unset() {
        let _guard = env_test_lock().lock().unwrap();
        let env = paper_prose_env_var("cursor");
        let prior_hook = std::env::var(env).ok();
        // SAFETY: test-only; env_test_lock() prevents concurrent env access from other tests.
        unsafe { core_state_utils::env_sync::remove_env(env) };

        let tmp = std::env::temp_dir().join("paper-prose-merge-default-research");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("configs/framework")).unwrap();
        std::fs::write(
            tmp.join(REL_PATH),
            format!("{PREFIX_LINE}\n\nprose test body"),
        )
        .unwrap();

        assert!(paper_prose_hook_requested("cursor"));
        let mut out = json!({ "continue": true });
        maybe_merge_paper_prose_before_submit(
            &tmp,
            &mut out,
            "英文论文润色 abstract",
            false,
            "cursor",
        );
        let ctx = out
            .get("additional_context")
            .and_then(Value::as_str)
            .unwrap_or("");
        assert!(ctx.contains(PREFIX_LINE));

        restore_env(env, prior_hook);
    }

    #[test]
    fn merge_skips_when_hook_explicitly_off() {
        let _guard = env_test_lock().lock().unwrap();
        let env = paper_prose_env_var("cursor");
        let prior_hook = std::env::var(env).ok();
        // SAFETY: test-only; env_test_lock() prevents concurrent env access from other tests.
        unsafe { core_state_utils::env_sync::set_env(env, "0") };

        let tmp = std::env::temp_dir().join("paper-prose-merge-off-research");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("configs/framework")).unwrap();
        std::fs::write(tmp.join(REL_PATH), format!("{PREFIX_LINE}\n\n正文。")).unwrap();

        assert!(!paper_prose_hook_requested("cursor"));
        let mut out = json!({ "continue": true });
        maybe_merge_paper_prose_before_submit(&tmp, &mut out, "SCI润色 abstract", false, "cursor");
        assert!(out.get("additional_context").is_none());

        restore_env(env, prior_hook);
    }

    #[test]
    fn append_context_codex_host() {
        let _guard = env_test_lock().lock().unwrap();
        let env = paper_prose_env_var("codex");
        let prior_hook = std::env::var(env).ok();
        // SAFETY: test-only; env_test_lock() prevents concurrent env access from other tests.
        unsafe { core_state_utils::env_sync::remove_env(env) };

        let tmp = std::env::temp_dir().join("paper-prose-append-codex-research");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("configs/framework")).unwrap();
        std::fs::write(
            tmp.join(REL_PATH),
            format!("{PREFIX_LINE}\n\ncodex prose body"),
        )
        .unwrap();

        let mut contexts = Vec::new();
        maybe_append_paper_prose_context(&tmp, "SCI润色 abstract", &mut contexts, "codex");
        assert_eq!(contexts.len(), 1);
        assert!(contexts[0].contains(PREFIX_LINE));

        restore_env(env, prior_hook);
    }
}
