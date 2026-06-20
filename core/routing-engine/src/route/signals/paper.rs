use super::has_signal_by_name;
use crate::text::text_matches_phrase;

pub fn has_prose_naturalization_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("prose_naturalization", query_text, query_token_list)
}

pub fn has_explicit_prose_polish_marker(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("explicit_prose_polish", query_text, query_token_list)
}

/// 不要求 `has_paper_context` 的显式学术润色（如「SCI润色 abstract」「polish this abstract」）。
pub fn has_standalone_academic_polish_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    if has_paper_prose_negation_context(query_text, query_token_list) {
        return false;
    }
    // query_text is already lowercased at routing entry — all markers are lowercase.
    // NOTE: "sci润色" / "sci 润色" are intentionally omitted here because the
    // whitespace-variant ambiguity (no-space "sci润色" vs spaced "sci 润色") is
    // unreliable via `contains`.  The signal is reliably caught by the
    // `explicit_prose_polish` SIGNAL_DEFS entry ("sci润色", "sci 润色" markers)
    // via `has_explicit_prose_polish_marker` below, which uses token-level
    // `text_matches_phrase` (order-independent, whitespace-tolerant).
    if [
        "学术润色",
        "英文论文润色",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(marker)
            || text_matches_phrase(query_token_list, marker)
    }) {
        return true;
    }
    let has_polish = has_explicit_prose_polish_marker(query_text, query_token_list)
        || query_text.contains("polish")
        || query_text.contains("proofread")
        || query_text.contains("copyedit");
    has_polish && text_has_manuscript_section(query_text, query_text)
}

pub fn has_paper_writing_context(query_text: &str, query_token_list: &[String]) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    if has_paper_prose_negation_context(query_text, query_token_list) {
        return false;
    }
    if has_paper_ref_first_workflow_context(query_text, query_token_list) {
        return false;
    }
    if has_paper_review_judgment_context(query_text, query_token_list)
        && !has_explicit_prose_polish_marker(query_text, query_token_list)
    {
        return false;
    }
    [
        "润色",
        "文字精修",
        "表达",
        "故事线",
        "重写摘要",
        "重写引言",
        "写摘要",
        "写引言",
        "写论文",
        "论文写作",
        "写 related work",
        "related work 部分",
        "SCI润色",
        "英文论文润色",
        "学术润色",
        "只改表达",
        "降AI味",
        "去AI味",
        "polish",
        "rewrite introduction",
        "rewrite abstract",
        "manuscript editing",
        "academic writing",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(*marker)
            || text_matches_phrase(query_token_list, marker)
    })
}

/// 比 `has_paper_writing_context` 更宽：口语改稿、粘贴段落、LaTeX 块——**无需**用户说「润色/language_register」。
pub fn looks_like_pasted_manuscript_prose(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if text.contains("\\begin{")
        || text.contains("\\section")
        || text.contains("\\cite{")
        || text.contains("\\ref{")
    {
        return true;
    }
    if [
        "abstract",
        "introduction",
        "related work",
        "methods",
        "results",
        "discussion",
        "conclusion",
    ]
    .iter()
    .any(|h| lower.contains(h))
        && text.len() > 100
    {
        return true;
    }
    if ["摘要", "引言", "相关工作", "方法", "结果", "讨论", "结论"]
        .iter()
        .any(|h| text.contains(h))
        && text.len() > 80
    {
        return true;
    }
    if text.len() > 320 {
        let en_hits = [
            "we propose",
            "we present",
            "however,",
            "experiments show",
            "our method",
            "in this paper",
        ]
        .iter()
        .filter(|m| lower.contains(*m))
        .count();
        let zh_hits = [
            "本文",
            "我们提出",
            "实验表明",
            "然而，",
            "综上所述",
            "本研究",
        ]
        .iter()
        .filter(|m| text.contains(*m))
        .count();
        if en_hits >= 2 || zh_hits >= 2 {
            return true;
        }
    }
    false
}

pub fn has_paper_prose_edit_context(query_text: &str, query_token_list: &[String]) -> bool {
    if has_paper_prose_negation_context(query_text, query_token_list) {
        return false;
    }
    if has_standalone_academic_polish_context(query_text, query_token_list) {
        return true;
    }
    if has_paper_writing_context(query_text, query_token_list) {
        return true;
    }
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    if looks_like_pasted_manuscript_prose(query_text) {
        return true;
    }
    if has_paper_ref_first_workflow_context(query_text, query_token_list) {
        return false;
    }
    if has_paper_review_judgment_context(query_text, query_token_list)
        && !has_explicit_prose_polish_marker(query_text, query_token_list)
    {
        return false;
    }
    [
        "改这段",
        "这段文字",
        "这一段",
        "这段话",
        "不通顺",
        "读起来",
        "拗口",
        "不好读",
        "太难读",
        "表达不好",
        "写得太",
        "改改",
        "帮我改",
        "顺一下",
        "改一下",
        "科研文本",
        "正文",
        "caption",
        "图注",
        "表注",
        "polish this",
        "proofread",
        "copyedit",
        "readability",
        "wording",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(*marker)
            || text_matches_phrase(query_token_list, marker)
    })
}

pub fn has_paper_context(query_text: &str, query_token_list: &[String]) -> bool {
    has_signal_by_name("paper", query_text, query_token_list)
}

/// Detect research workspace context: keyword match + directory-based detection.
///
/// Returns true when the query contains research-log keywords OR when the
/// current working directory (or an ancestor) contains a `research-state.yaml`
/// or `.research.toml` marker file. Directory detection is re-evaluated on
/// each call (single `stat` per ancestor — negligible cost).
/// Check ancestor directories for research workspace marker files.
pub fn has_paper_workbench_frontdoor_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    [
        "整体推进这篇论文",
        "现在该审",
        "该审",
        "该改",
        "该补实验",
        "怎么处理",
        "先审再改",
        "改到能投",
        "该删就删",
        "藏到附录",
        "根据 reviewer comments 修改论文",
        "根据 reviewer comments 改论文",
        "能不能投",
        "整篇严审",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(*marker)
            || text_matches_phrase(query_token_list, marker)
    })
}

pub fn has_paper_figure_layout_review_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    let visual_markers = [
        "图表", "排版", "figure", "figures", "table", "tables", "layout",
    ];
    let review_markers = ["只看", "审", "review", "检查", "别检查别的维度"];
    visual_markers
        .iter()
        .any(|marker| paper_route_marker_matches(query_text, query_token_list, marker))
        && review_markers
            .iter()
            .any(|marker| paper_route_marker_matches(query_text, query_token_list, marker))
}

pub fn has_paper_logic_evidence_review_context(
    query_text: &str,
    query_token_list: &[String],
) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    let logic_markers = [
        "claim",
        "claims",
        "evidence",
        "证据",
        "支撑",
        "实验支撑",
        "对齐",
        "够不够",
    ];
    let review_markers = ["看", "检查", "评估", "review", "审", "别润色"];
    logic_markers
        .iter()
        .any(|marker| paper_route_marker_matches(query_text, query_token_list, marker))
        && review_markers
            .iter()
            .any(|marker| paper_route_marker_matches(query_text, query_token_list, marker))
}

pub fn has_paper_prose_negation_context(query_text: &str, query_token_list: &[String]) -> bool {
    if query_text.contains("别润色") || query_text.contains("不润色") {
        return true;
    }
    [
        "no polish",
        "do not polish",
        "don't polish",
        "dont polish",
        "不要润色",
        "只审不改",
        "critique only",
        "critique-only",
        "review only",
        "review-only",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(*marker)
            || text_matches_phrase(query_token_list, marker)
    })
}

pub fn paper_skill_requires_context(slug: &str) -> bool {
    matches!(
        slug,
        "paper-workbench" | "paper-reviewer" | "paper-reviser" | "paper-writing"
    )
}

/// Substring-prone single-token markers must use whole-token match only (e.g. `review` vs `preview`).
fn paper_route_marker_matches(query_text: &str, query_token_list: &[String], marker: &str) -> bool {
    let token_only = matches!(marker, "review" | "审" | "看" | "检查" | "评估");
    if token_only {
        return text_matches_phrase(query_token_list, marker);
    }
    if marker.split_whitespace().count() > 1 {
        return query_text.contains(marker) || text_matches_phrase(query_token_list, marker);
    }
    query_text.contains(marker) || text_matches_phrase(query_token_list, marker)
}

fn text_has_manuscript_section(lower: &str, query_text: &str) -> bool {
    [
        "abstract",
        "introduction",
        "related work",
        "methods",
        "results",
        "discussion",
        "conclusion",
    ]
    .iter()
    .any(|h| lower.contains(h))
        || ["摘要", "引言", "相关工作", "方法", "结果", "讨论", "结论"]
            .iter()
            .any(|h| query_text.contains(h))
}

/// 显式润色/写作 marker（用于审稿+润色并存时不阻断 prose 路径）。
pub fn has_paper_review_revision_intent(query_text: &str, query_token_list: &[String]) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    let review_markers = [
        "review",
        "reviewer comments",
        "review comments",
        "审稿意见",
        "评审意见",
    ];
    let revise_markers = ["改论文", "修改论文", "改稿", "修改稿", "进入修改", "直接改"];
    review_markers
        .iter()
        .any(|marker| paper_route_marker_matches(query_text, query_token_list, marker))
        && revise_markers.iter().any(|marker| {
        query_text.contains(*marker)
            || text_matches_phrase(query_token_list, marker)
        })
}

pub fn has_paper_direct_revision_context(query_text: &str, query_token_list: &[String]) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    if [
        "该删就删",
        "藏到附录",
        "改到能投",
        "根据 reviewer comments 修改论文",
        "根据 reviewer comments 改论文",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(*marker)
            || text_matches_phrase(query_token_list, marker)
    }) {
        return false;
    }
    [
        "别先给方案",
        "直接进入修改",
        "直接改稿",
        "不要再审",
        "只进改稿",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(*marker)
            || text_matches_phrase(query_token_list, marker)
    })
}

pub fn has_paper_review_judgment_context(query_text: &str, query_token_list: &[String]) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    [
        "paper review",
        "review paper",
        "审稿",
        "审一下",
        "严审",
        "投稿前",
        "能不能投",
        "投稿判断",
        "reviewer-style",
        "reviewer style",
        "外部调研",
        "查文献后审",
        "科学性批评",
        "科学批评",
        "只要批评",
        "只批评",
        "只要科学",
        "不要改稿",
        "别改稿",
        "只审不改",
        "critique only",
        "critique-only",
        "review only",
    ]
    .iter()
    .any(|marker| {
        query_text.contains(*marker)
            || text_matches_phrase(query_token_list, marker)
    })
}

pub fn has_paper_ref_first_workflow_context(query_text: &str, query_token_list: &[String]) -> bool {
    if !has_paper_context(query_text, query_token_list) {
        return false;
    }
    let ref_markers = [
        "下载ref",
        "目标期刊",
        "相近ref",
        "相近 ref",
        "reference corpus",
        "target journal",
    ];
    let story_or_write_markers = [
        "讲故事",
        "故事线",
        "写作套路",
        "重写摘要",
        "重写引言",
        "再写",
        "再帮我重写",
    ];
    ref_markers.iter().any(|marker| {
        query_text.contains(*marker)
            || text_matches_phrase(query_token_list, marker)
    }) && story_or_write_markers.iter().any(|marker| {
        query_text.contains(*marker)
            || text_matches_phrase(query_token_list, marker)
    })
}

pub fn has_math_review_context(query_text: &str, query_token_list: &[String]) -> bool {
    let math_review_markers = [
        "审一下这个证明",
        "审这个证明",
        "审一下推导",
        "证明是否正确",
        "推导是否正确",
        "证明有没有漏洞",
        "推导有没有漏洞",
        "check this proof",
        "review this proof",
        "verify this derivation",
        "数学推导审查",
        "证明审查",
    ];
    let has_math_review = math_review_markers.iter().any(|marker| {
        query_text.contains(*marker)
            || text_matches_phrase(query_token_list, marker)
    });
    if !has_math_review {
        return false;
    }
    // Exclude when there's a full paper context (those route to paper-reviewer)
    !has_paper_context(query_text, query_token_list)
}
