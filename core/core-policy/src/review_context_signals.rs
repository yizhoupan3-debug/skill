//! Injectable B1 routing probes for manuscript / PR context (keeps core-policy free of `route/`).

use std::sync::OnceLock;

pub type ReviewContextProbe = fn(&str, &[String]) -> bool;

struct Installed {
    has_paper: ReviewContextProbe,
    has_github_pr: ReviewContextProbe,
}

static PROBES: OnceLock<Installed> = OnceLock::new();

fn noop(_: &str, _: &[String]) -> bool {
    false
}

/// B1 installs route-backed probes during `kernel_bootstrap`.
pub fn install_review_context_probes(
    has_paper: ReviewContextProbe,
    has_github_pr: ReviewContextProbe,
) {
    let _ = PROBES.set(Installed {
        has_paper,
        has_github_pr,
    });
}

fn installed() -> &'static Installed {
    PROBES.get_or_init(|| Installed {
        has_paper: noop,
        has_github_pr: noop,
    })
}

pub fn has_paper_context(query_text: &str, query_token_list: &[String]) -> bool {
    (installed().has_paper)(query_text, query_token_list)
}

pub fn has_github_pr_context(query_text: &str, query_token_list: &[String]) -> bool {
    (installed().has_github_pr)(query_text, query_token_list)
}

#[cfg(test)]
pub fn install_test_review_context_probes() {
    fn normalize_query_text(text: &str) -> String {
        text.to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
    fn alnum_token(token: &str) -> String {
        token
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_lowercase())
            .collect()
    }
    fn phrase_token_matches(task_token: &str, phrase_token: &str) -> bool {
        let task = alnum_token(task_token);
        let phrase = alnum_token(phrase_token);
        if phrase.is_empty() {
            return false;
        }
        if phrase.chars().all(|c| c.is_ascii_alphanumeric()) {
            task == phrase
        } else {
            task.contains(&phrase)
        }
    }
    fn text_matches_phrase(tokens: &[String], phrase: &str) -> bool {
        let phrase_tokens: Vec<String> = phrase
            .split_whitespace()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_ascii_lowercase())
            .collect();
        if phrase_tokens.is_empty() {
            return false;
        }
        if phrase_tokens.len() == 1 {
            return tokens
                .iter()
                .any(|t| phrase_token_matches(t, &phrase_tokens[0]));
        }
        if phrase_tokens.len() > tokens.len() {
            return false;
        }
        for start in 0..=(tokens.len() - phrase_tokens.len()) {
            if phrase_tokens.iter().enumerate().all(|(offset, phrase_token)| {
                phrase_token_matches(&tokens[start + offset], phrase_token)
            }) {
                return true;
            }
        }
        false
    }
    fn probe_paper(query_text: &str, query_token_list: &[String]) -> bool {
        let normalized = normalize_query_text(query_text);
        [
            "paper",
            "manuscript",
            "论文",
            "稿子",
            "稿件",
            "摘要",
            "引言",
            "审稿意见",
            "reviewer comments",
            "rebuttal",
            "appendix",
            "claim",
            "投稿",
            "期刊",
            "科研",
        ]
        .iter()
        .any(|marker| {
            normalized.contains(&normalize_query_text(marker))
                || text_matches_phrase(query_token_list, marker)
        })
    }
    fn probe_github(query_text: &str, query_token_list: &[String]) -> bool {
        let normalized = normalize_query_text(query_text);
        normalized.contains("github")
            || text_matches_phrase(query_token_list, "github")
            || text_matches_phrase(query_token_list, "gh")
            || normalized.contains("pull request")
            || text_matches_phrase(query_token_list, "pull request")
            || text_matches_phrase(query_token_list, "pr")
    }
    install_review_context_probes(probe_paper, probe_github);
}
