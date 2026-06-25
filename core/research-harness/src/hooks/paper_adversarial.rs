//! 对抗审稿 hook — 为论文编辑场景追加对抗性审稿上下文。
//!
//! 文案真源：`configs/framework/PAPER_ADVERSARIAL_HOOK.txt`。**单真源**：`builtin_block()`
//! 通过 `include_str!` 在编译期嵌入同一份 txt，避免「磁盘文案 vs Rust 硬编码」双轨漂移。
//! 环境变量 per-host：`1`/`true`/`yes`/`on` 启用；未设置或其它值视为关闭。
//! - 受 `ROUTER_RS_OPERATOR_INJECT` 聚合闸约束。

use crate::hooks::paper_block_cache::BlockCache;
use serde_json::Value;
use std::path::Path;

// ── Constants ──

const REL_PATH: &str = "configs/framework/PAPER_ADVERSARIAL_HOOK.txt";
/// 首行须与 `merge_hook_nudge_paragraph` strip 前缀、`apply_cursor_hook_output_policy` SILENT 放行子串一致。
pub const PREFIX_LINE: &str = "**PAPER_ADVERSARIAL_HOOK**";

/// 编译期嵌入的回落文案：与 `REL_PATH` **同源**（同一份磁盘 txt），仅在用户仓库内
/// 文件缺失 / 空 / 仅标题时启用。
const BUILTIN_TXT: &str =
    include_str!("../../../../configs/framework/PAPER_ADVERSARIAL_HOOK.txt");

// ── Static state ──

static BUILTIN_BLOCK: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| BUILTIN_TXT.trim().to_string());

/// Access the compiled-in builtin block directly (no disk lookup).
pub fn builtin_block() -> String {
    BUILTIN_BLOCK.clone()
}

static BLOCK_CACHE: BlockCache = BlockCache::new(REL_PATH, PREFIX_LINE, "paper adversarial");

// ── Per-host environment variable mapping ──

/// Import per-host env var name resolution from host-projection (L0).
use host_projection::hooks::paper_adversarial_env_var;

// ── Public API ──

/// Check whether the adversarial hook is requested for a given host.
pub fn paper_adversarial_hook_requested(host: &str) -> bool {
    super::paper_common::operator_inject_globally_enabled()
        && core_policy::env_flags::env_enabled_default_false(paper_adversarial_env_var(host))
}

/// 轻量启发：检测用户提示是否涉及论文审稿/返修。
pub fn prompt_signals_manuscript_work(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();

    let has_zh_paper = text.contains("论文") || text.contains("手稿");
    let has_en_paper = lower.contains("manuscript") || lower.contains("rebuttal");
    let has_paper_signal = has_zh_paper || has_en_paper;

    // 工程噪声过滤
    let code_only_noise = (lower.contains("pull request")
        || lower.contains(".github/workflows")
        || lower.contains("cargo test")
        || lower.contains("cargo build")
        || lower.contains("cargo fmt")
        || lower.contains("clippy"))
        && !has_paper_signal;
    if code_only_noise {
        return false;
    }

    // 强中文信号
    static STRONG_ZH: &[&str] = &[
        "审稿",
        "审稿人",
        "审稿意见",
        "返修",
        "大修",
        "小修",
        "改稿",
        "投稿",
        "rebuttal",
        "response letter",
    ];
    if STRONG_ZH.iter().any(|k| text.contains(k)) {
        return true;
    }

    // 强英文信号
    static STRONG_EN: &[&str] = &[
        "manuscript",
        "revise and resubmit",
        "meta-review",
        "reviewer comment",
        "major revision",
        "minor revision",
        "point-by-point",
        "\\begin{abstract}",
        "supplementary material",
    ];
    if STRONG_EN.iter().any(|k| lower.contains(k)) {
        return true;
    }

    // 弱信号组合（需 >= 5 个同时命中才放行）
    static WEAK: &[&str] = &[
        "latex", "appendix", "theorem", "lemma", "baseline", "ablation", "novelty", "claim",
    ];
    let weak_count = WEAK.iter().filter(|k| lower.contains(*k)).count();

    // ML 行话降权
    static ANTI_SIGNALS: &[&str] = &[
        "transformer",
        "attention",
        "convolution",
        "normalization",
        "optimizer",
        "gradient descent",
        "batch size",
        "learning rate",
    ];
    let anti_hits = ANTI_SIGNALS.iter().filter(|k| lower.contains(*k)).count();
    let adjusted = if anti_hits >= 2 && !has_paper_signal {
        weak_count.saturating_sub(2)
    } else {
        weak_count
    };

    adjusted >= 5
}

/// Alias used by runtime-core wrappers.
pub fn prompt_signals_paper_manuscript_work(text: &str) -> bool {
    prompt_signals_manuscript_work(text)
}

/// Resolve the adversarial hook block from disk or builtin.
pub fn resolve_paper_adversarial_block(repo_root: &Path) -> String {
    BLOCK_CACHE.resolve(repo_root, || BUILTIN_BLOCK.clone())
}

/// Append adversarial hook context if enabled and the prompt signals manuscript work.
pub fn maybe_append_paper_adversarial_context(
    repo_root: &Path,
    prompt_text: &str,
    contexts: &mut Vec<String>,
    host: &str,
) {
    super::paper_common::maybe_append_context(
        paper_adversarial_hook_requested(host),
        prompt_signals_manuscript_work(prompt_text),
        repo_root,
        &BLOCK_CACHE,
        &BUILTIN_BLOCK,
        contexts,
    );
}

/// Merge adversarial hook context into Cursor-compatible JSON output.
pub fn maybe_merge_paper_adversarial_before_submit(
    repo_root: &Path,
    output: &mut Value,
    prompt_text: &str,
    use_followup_message: bool,
    host: &str,
) {
    super::paper_common::maybe_merge_context(
        paper_adversarial_hook_requested(host),
        prompt_signals_manuscript_work(prompt_text),
        repo_root,
        &BLOCK_CACHE,
        &BUILTIN_BLOCK,
        output,
        PREFIX_LINE,
        use_followup_message,
    );
}

/// 简单版：在检测到论文审稿相关操作时，追加对抗性审稿上下文片段。
/// 返回 `None` 表示不追加。
pub fn maybe_append_adversarial_context(context: &str) -> Option<String> {
    if !prompt_signals_manuscript_work(context) {
        return None;
    }
    Some(BUILTIN_BLOCK.clone())
}

#[cfg(test)]
mod tests {
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
    fn signal_zh_reviewer() {
        assert!(prompt_signals_manuscript_work(
            "请根据审稿意见逐条改 Introduction"
        ));
    }

    #[test]
    fn signal_negative_pr_without_paper() {
        assert!(!prompt_signals_manuscript_work(
            "fix failing cargo test in CI and open a pull request"
        ));
    }

    #[test]
    fn signal_negative_cargo_fmt_noise() {
        assert!(!prompt_signals_manuscript_work(
            "run cargo fmt and clippy before pull request"
        ));
    }

    #[test]
    fn weak_signals_need_five_hits() {
        assert!(!prompt_signals_manuscript_work(
            "compare baseline ablation novelty metrics in training logs"
        ));
        // 4 weak hits (baseline, ablation, novelty, claim) below threshold of 5
        assert!(!prompt_signals_manuscript_work(
            "baseline, ablation, novelty, claim"
        ));
        // 5 weak hits (appendix, baseline, ablation, novelty, claim) meets threshold
        assert!(prompt_signals_manuscript_work(
            "appendix: baseline ablation novelty claim metrics"
        ));
    }

    #[test]
    fn ml_tech_discussion_suppressed() {
        assert!(!prompt_signals_manuscript_work(
            "The training loss uses a transformer architecture with layer normalization and attention"
        ));
        assert!(!prompt_signals_manuscript_work(
            "our model uses convolution with batch normalization and gradient descent optimizer"
        ));
    }

    #[test]
    fn ml_tech_with_paper_keyword_not_suppressed() {
        assert!(prompt_signals_manuscript_work(
            "请根据审稿意见修改这篇手稿的 baseline 和 ablation 实验设计"
        ));
    }

    #[test]
    fn merge_skips_when_prompt_not_paper() {
        let mut out = json!({ "continue": true });
        let tmp = std::env::temp_dir().join("paper-adv-empty-research");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        maybe_merge_paper_adversarial_before_submit(&tmp, &mut out, "cargo fmt", false, "cursor");
        assert!(out.get("additional_context").is_none());
    }

    #[test]
    fn resolve_prefixes_file_missing() {
        let tmp = std::env::temp_dir().join("paper-adv-missing-research");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let b = resolve_paper_adversarial_block(&tmp);
        assert!(b.starts_with(PREFIX_LINE));
        assert!(b.contains("强对抗"));
    }

    #[test]
    fn resolve_header_only_file_falls_back_to_builtin_no_double_prefix() {
        let tmp = std::env::temp_dir().join("paper-adv-header-only-research");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("configs/framework")).unwrap();
        let p = tmp.join(REL_PATH);
        std::fs::write(&p, format!("{PREFIX_LINE}\n")).unwrap();
        let b = resolve_paper_adversarial_block(&tmp);
        assert!(b.starts_with(PREFIX_LINE));
        assert_eq!(b.matches(PREFIX_LINE).count(), 1);
    }

    #[test]
    fn builtin_block_is_compile_time_embedded_disk_txt() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("repo root above core/research-harness/")
            .to_path_buf();
        let on_disk =
            std::fs::read_to_string(repo_root.join(REL_PATH)).expect("PAPER_ADVERSARIAL_HOOK.txt readable");
        assert_eq!(BUILTIN_BLOCK.clone(), on_disk.trim());
        assert!(BUILTIN_BLOCK.contains("强对抗审稿"));
        assert!(BUILTIN_BLOCK.contains("closest-work"));
    }

    #[test]
    fn requested_false_when_operator_inject_killed() {
        let _guard = env_test_lock().lock().unwrap();
        let prior_inject = std::env::var("ROUTER_RS_OPERATOR_INJECT").ok();
        let hook_var = paper_adversarial_env_var("cursor");
        let prior_hook = std::env::var(hook_var).ok();
        // SAFETY: test-only; env_test_lock() prevents concurrent env access from other tests.
        unsafe { core_state_utils::env_sync::set_env("ROUTER_RS_OPERATOR_INJECT", "0") };
        // SAFETY: test-only; env_test_lock() prevents concurrent env access from other tests.
        unsafe { core_state_utils::env_sync::set_env(hook_var, "1") };
        assert!(!paper_adversarial_hook_requested("cursor"));
        restore_env("ROUTER_RS_OPERATOR_INJECT", prior_inject);
        restore_env(hook_var, prior_hook);
    }

    #[test]
    fn requested_false_when_hook_env_unset() {
        let _guard = env_test_lock().lock().unwrap();
        let prior_inject = std::env::var("ROUTER_RS_OPERATOR_INJECT").ok();
        let hook_var = paper_adversarial_env_var("cursor");
        let prior_hook = std::env::var(hook_var).ok();
        // SAFETY: test-only; env_test_lock() prevents concurrent env access from other tests.
        unsafe { core_state_utils::env_sync::remove_env("ROUTER_RS_OPERATOR_INJECT") };
        // SAFETY: test-only; env_test_lock() prevents concurrent env access from other tests.
        unsafe { core_state_utils::env_sync::remove_env(hook_var) };
        assert!(!paper_adversarial_hook_requested("cursor"));
        restore_env("ROUTER_RS_OPERATOR_INJECT", prior_inject);
        restore_env(hook_var, prior_hook);
    }

    #[test]
    fn merge_injects_when_enabled_and_prompt_paper() {
        let _guard = env_test_lock().lock().unwrap();
        let prior_inject = std::env::var("ROUTER_RS_OPERATOR_INJECT").ok();
        let hook_var = paper_adversarial_env_var("cursor");
        let prior_hook = std::env::var(hook_var).ok();
        // SAFETY: test-only; env_test_lock() prevents concurrent env access from other tests.
        unsafe { core_state_utils::env_sync::remove_env("ROUTER_RS_OPERATOR_INJECT") };
        // SAFETY: test-only; env_test_lock() prevents concurrent env access from other tests.
        unsafe { core_state_utils::env_sync::set_env(hook_var, "1") };

        let tmp = std::env::temp_dir().join("paper-adv-merge-on-research");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("configs/framework")).unwrap();
        std::fs::write(
            tmp.join(REL_PATH),
            format!("{PREFIX_LINE}\n\n短段正文：测试用。"),
        )
        .unwrap();

        let mut out = json!({ "continue": true });
        maybe_merge_paper_adversarial_before_submit(
            &tmp,
            &mut out,
            "请按审稿意见修这篇 manuscript",
            false,
            "cursor",
        );
        let ctx = out
            .get("additional_context")
            .and_then(Value::as_str)
            .unwrap_or("");
        assert!(ctx.contains(PREFIX_LINE), "expected merged: {ctx}");
        assert!(ctx.contains("短段正文"));

        restore_env("ROUTER_RS_OPERATOR_INJECT", prior_inject);
        restore_env(hook_var, prior_hook);
    }

    #[test]
    fn merge_skips_when_hook_disabled_even_if_prompt_paper() {
        let _guard = env_test_lock().lock().unwrap();
        let prior_inject = std::env::var("ROUTER_RS_OPERATOR_INJECT").ok();
        let hook_var = paper_adversarial_env_var("cursor");
        let prior_hook = std::env::var(hook_var).ok();
        // SAFETY: test-only; env_test_lock() prevents concurrent env access from other tests.
        unsafe { core_state_utils::env_sync::remove_env("ROUTER_RS_OPERATOR_INJECT") };
        // SAFETY: test-only; env_test_lock() prevents concurrent env access from other tests.
        unsafe { core_state_utils::env_sync::remove_env(hook_var) };

        let tmp = std::env::temp_dir().join("paper-adv-merge-off-research");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("configs/framework")).unwrap();
        std::fs::write(tmp.join(REL_PATH), format!("{PREFIX_LINE}\n\n正文。")).unwrap();

        let mut out = json!({ "continue": true });
        maybe_merge_paper_adversarial_before_submit(
            &tmp,
            &mut out,
            "请按审稿意见修这篇 manuscript",
            false,
            "cursor",
        );
        assert!(out.get("additional_context").is_none());
        assert!(out.get("followup_message").is_none());

        restore_env("ROUTER_RS_OPERATOR_INJECT", prior_inject);
        restore_env(hook_var, prior_hook);
    }

    #[test]
    fn prompt_signals_manuscript_work_is_send() {
        let result = std::thread::spawn(move || {
            prompt_signals_manuscript_work(
                "请根据审稿意见逐条修改这篇论文的 Introduction",
            )
        })
        .join()
        .expect("thread panicked");
        assert!(result);
    }

    #[test]
    fn prompt_signals_negative_is_send() {
        let result = std::thread::spawn(move || {
            prompt_signals_manuscript_work(
                "run cargo fmt and clippy before pull request",
            )
        })
        .join()
        .expect("thread panicked");
        assert!(!result);
    }

    #[test]
    fn resolve_adversarial_block_is_send() {
        let tmp = std::env::temp_dir().join("paper-adv-send-test-research");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("configs/framework")).unwrap();
        let p = tmp.join(REL_PATH);
        std::fs::write(&p, format!("{PREFIX_LINE}\n\nSend-safe 测试正文。")).unwrap();
        let result = std::thread::spawn(move || resolve_paper_adversarial_block(&tmp))
            .join()
            .expect("thread panicked");
        assert!(result.contains(PREFIX_LINE));
        assert!(result.contains("Send-safe"));
    }

    #[test]
    fn maybe_append_adversarial_returns_context() {
        let result =
            maybe_append_adversarial_context("请根据审稿意见修改论文");
        assert!(result.is_some());
        assert!(result.unwrap().contains(PREFIX_LINE));
    }

    #[test]
    fn maybe_append_adversarial_no_signal_returns_none() {
        let result = maybe_append_adversarial_context("fix cargo fmt");
        assert!(result.is_none());
    }
}
