//! Cursor `beforeSubmit`：论文手稿类用户提示可合并 **强对抗审稿** 短段（opt-in）。
//!
//! 文案真源：`configs/framework/PAPER_ADVERSARIAL_HOOK.txt`。**单真源**：`builtin_block()`
//! 通过 `include_str!` 在编译期嵌入同一份 txt，避免「磁盘文案 vs Rust 硬编码」双轨漂移
//! （review P0-1 修复）。环境变量：
//! - `ROUTER_RS_CURSOR_PAPER_ADVERSARIAL_HOOK`：`1`/`true`/`yes`/`on` 启用；未设置或其它值视为关闭。
//! - 受 `ROUTER_RS_OPERATOR_INJECT` 聚合闸约束（与 AUTOPILOT/RFV nudge 一致）。

use crate::router_env_flags::{
    router_rs_env_enabled_default_false, router_rs_operator_inject_globally_enabled,
};
use serde_json::Value;
use std::fs;
use std::path::Path;

const REL_PATH: &str = "configs/framework/PAPER_ADVERSARIAL_HOOK.txt";
const ENV_HOOK: &str = "ROUTER_RS_CURSOR_PAPER_ADVERSARIAL_HOOK";
/// 首行须与 `merge_hook_nudge_paragraph` strip 前缀、`apply_cursor_hook_output_policy` SILENT 放行子串一致。
pub(crate) const PREFIX_LINE: &str = "**PAPER_ADVERSARIAL_HOOK**";

/// 编译期嵌入的回落文案：与 `REL_PATH` **同源**（同一份磁盘 txt），仅在用户仓库内
/// 文件缺失 / 空 / 仅标题时启用，确保 hook 永远能注入一段一致的对抗审稿提示。
const BUILTIN_TXT: &str = include_str!("../../../configs/framework/PAPER_ADVERSARIAL_HOOK.txt");

fn builtin_block() -> String {
    BUILTIN_TXT.trim().to_string()
}

pub(crate) fn cursor_paper_adversarial_hook_requested() -> bool {
    router_rs_operator_inject_globally_enabled() && router_rs_env_enabled_default_false(ENV_HOOK)
}

/// 轻量启发：倾向少漏报论文任务、少误伤纯工程 PR/Cargo 对话。
pub(crate) fn prompt_signals_paper_manuscript_work(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let code_only_noise = (lower.contains("pull request")
        || lower.contains(".github/workflows")
        || lower.contains("cargo test")
        || lower.contains("cargo build")
        || lower.contains("cargo fmt")
        || lower.contains("clippy")
        || lower.contains("rustfmt"))
        && !text.contains("论文")
        && !text.contains("手稿")
        && !lower.contains("manuscript")
        && !lower.contains("rebuttal");

    if code_only_noise {
        return false;
    }

    let strong_zh = [
        "审稿",
        "审稿人",
        "审稿意见",
        "返修",
        "大修",
        "小修",
        "论文",
        "手稿",
        "改稿",
        "投稿",
        "能不能投",
        "rebuttal",
        "response letter",
    ];
    if strong_zh.iter().any(|k| text.contains(k)) {
        return true;
    }

    let strong_en = [
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
    if strong_en.iter().any(|k| lower.contains(k)) {
        return true;
    }

    // 不用泛词 `paper`（易与 white paper / 产品文档误触）；弱信号须凑满条数才放行，减少纯工程/ML 代码聊天误注入。
    let weak = [
        "latex", "appendix", "theorem", "lemma", "baseline", "ablation", "novelty", "claim",
    ];
    weak.iter().filter(|k| lower.contains(*k)).count() >= 4
}

pub(crate) fn resolve_paper_adversarial_block(repo_root: &Path) -> String {
    let path = repo_root.join(REL_PATH);
    match fs::read_to_string(&path) {
        Ok(t) => {
            let trimmed = t.trim();
            if trimmed.is_empty() {
                return builtin_block();
            }
            // 真源已带首行 PREFIX：整段采用；若用户只写了单行标题、无正文，回退内置（避免 PREFIX 双写）。
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

pub(crate) fn maybe_merge_paper_adversarial_before_submit(
    repo_root: &Path,
    output: &mut Value,
    prompt_text: &str,
    use_followup_message: bool,
) {
    if !cursor_paper_adversarial_hook_requested()
        || !prompt_signals_paper_manuscript_work(prompt_text)
    {
        return;
    }
    let msg = resolve_paper_adversarial_block(repo_root);
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
    fn signal_zh_reviewer() {
        assert!(prompt_signals_paper_manuscript_work(
            "请根据审稿意见逐条改 Introduction"
        ));
    }

    #[test]
    fn signal_negative_pr_without_paper() {
        assert!(!prompt_signals_paper_manuscript_work(
            "fix failing cargo test in CI and open a pull request"
        ));
    }

    #[test]
    fn signal_negative_cargo_fmt_noise() {
        assert!(!prompt_signals_paper_manuscript_work(
            "run cargo fmt and clippy before pull request"
        ));
    }

    #[test]
    fn weak_signals_need_four_hits() {
        assert!(!prompt_signals_paper_manuscript_work(
            "compare baseline ablation novelty metrics in training logs"
        ));
        assert!(prompt_signals_paper_manuscript_work(
            "compare baseline ablation novelty claim metrics in appendix table"
        ));
    }

    #[test]
    fn merge_skips_when_prompt_not_paper() {
        let mut out = json!({ "continue": true });
        let tmp = std::env::temp_dir().join("paper-adv-empty");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        maybe_merge_paper_adversarial_before_submit(&tmp, &mut out, "cargo fmt", false);
        assert!(out.get("additional_context").is_none());
    }

    #[test]
    fn resolve_prefixes_file_missing() {
        let tmp = std::env::temp_dir().join("paper-adv-missing");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let b = resolve_paper_adversarial_block(&tmp);
        assert!(b.starts_with(PREFIX_LINE));
        assert!(b.contains("强对抗"));
    }

    #[test]
    fn resolve_header_only_file_falls_back_to_builtin_no_double_prefix() {
        let tmp = std::env::temp_dir().join("paper-adv-header-only");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("configs/framework")).unwrap();
        let p = tmp.join(REL_PATH);
        std::fs::write(&p, format!("{PREFIX_LINE}\n")).unwrap();
        let b = resolve_paper_adversarial_block(&tmp);
        assert!(b.starts_with(PREFIX_LINE));
        assert_eq!(b.matches(PREFIX_LINE).count(), 1);
    }

    /// review P0-1 真源单轨：`builtin_block()` 必须用 `include_str!` 嵌入磁盘真源；
    /// 防止后续有人手改回硬编码字符串引起双轨漂移。
    #[test]
    fn builtin_block_is_compile_time_embedded_disk_txt() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("repo root above scripts/router-rs/")
            .to_path_buf();
        let on_disk = std::fs::read_to_string(repo_root.join(REL_PATH))
            .expect("PAPER_ADVERSARIAL_HOOK.txt readable");
        assert_eq!(builtin_block(), on_disk.trim());
        assert!(builtin_block().contains("强对抗审稿"));
        assert!(builtin_block().contains("closest-work"));
    }

    fn restore_env(key: &str, prior: Option<String>) {
        match prior {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }

    /// review P1-5：`ROUTER_RS_OPERATOR_INJECT=0` 即使子开关已开也必须关。
    #[test]
    fn requested_false_when_operator_inject_killed() {
        let _g = crate::harness_operator_nudges::harness_nudges_env_test_lock();
        let prior_inject = std::env::var("ROUTER_RS_OPERATOR_INJECT").ok();
        let prior_hook = std::env::var(ENV_HOOK).ok();
        std::env::set_var("ROUTER_RS_OPERATOR_INJECT", "0");
        std::env::set_var(ENV_HOOK, "1");
        assert!(!cursor_paper_adversarial_hook_requested());
        restore_env("ROUTER_RS_OPERATOR_INJECT", prior_inject);
        restore_env(ENV_HOOK, prior_hook);
    }

    /// review P1-5：未显式 opt-in 子开关时必须关（默认 opt-in 为 false）。
    #[test]
    fn requested_false_when_hook_env_unset() {
        let _g = crate::harness_operator_nudges::harness_nudges_env_test_lock();
        let prior_inject = std::env::var("ROUTER_RS_OPERATOR_INJECT").ok();
        let prior_hook = std::env::var(ENV_HOOK).ok();
        std::env::remove_var("ROUTER_RS_OPERATOR_INJECT");
        std::env::remove_var(ENV_HOOK);
        assert!(!cursor_paper_adversarial_hook_requested());
        restore_env("ROUTER_RS_OPERATOR_INJECT", prior_inject);
        restore_env(ENV_HOOK, prior_hook);
    }

    /// review P1-5：开关 + 命中 prompt 时必须真合并；用磁盘 txt 真源走完整路径。
    #[test]
    fn merge_injects_when_enabled_and_prompt_paper() {
        let _g = crate::harness_operator_nudges::harness_nudges_env_test_lock();
        let prior_inject = std::env::var("ROUTER_RS_OPERATOR_INJECT").ok();
        let prior_hook = std::env::var(ENV_HOOK).ok();
        std::env::remove_var("ROUTER_RS_OPERATOR_INJECT");
        std::env::set_var(ENV_HOOK, "1");

        let tmp = std::env::temp_dir().join("paper-adv-merge-on");
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
        );
        let ctx = out
            .get("additional_context")
            .and_then(Value::as_str)
            .unwrap_or("");
        assert!(ctx.contains(PREFIX_LINE), "expected merged: {ctx}");
        assert!(ctx.contains("短段正文"));

        restore_env("ROUTER_RS_OPERATOR_INJECT", prior_inject);
        restore_env(ENV_HOOK, prior_hook);
    }

    /// review P1-5：开关关闭时即使 prompt 强命中（`审稿`）也不注入。
    #[test]
    fn merge_skips_when_hook_disabled_even_if_prompt_paper() {
        let _g = crate::harness_operator_nudges::harness_nudges_env_test_lock();
        let prior_inject = std::env::var("ROUTER_RS_OPERATOR_INJECT").ok();
        let prior_hook = std::env::var(ENV_HOOK).ok();
        std::env::remove_var("ROUTER_RS_OPERATOR_INJECT");
        std::env::remove_var(ENV_HOOK);

        let tmp = std::env::temp_dir().join("paper-adv-merge-off");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("configs/framework")).unwrap();
        std::fs::write(tmp.join(REL_PATH), format!("{PREFIX_LINE}\n\n正文。")).unwrap();

        let mut out = json!({ "continue": true });
        maybe_merge_paper_adversarial_before_submit(
            &tmp,
            &mut out,
            "请按审稿意见修这篇 manuscript",
            false,
        );
        assert!(out.get("additional_context").is_none());
        assert!(out.get("followup_message").is_none());

        restore_env("ROUTER_RS_OPERATOR_INJECT", prior_inject);
        restore_env(ENV_HOOK, prior_hook);
    }
}
