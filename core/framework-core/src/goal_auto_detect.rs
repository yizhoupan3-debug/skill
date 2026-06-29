//! Goal auto-detection: heuristic complexity analysis + scope change detection.
//!
//! Used by UserPromptSubmit to decide whether to suggest creating a Goal contract
//! (complex task) or amending an existing Goal (scope change).

// ── Constants ──────────────────────────────────────────────────────────────

/// Chinese implementation verbs indicating a task may be complex.
const ZH_IMPLEMENT_VERBS: &[&str] = &[
    "实现",
    "重构",
    "添加",
    "添加新",
    "修改",
    "编写",
    "构建",
    "迁移",
    "优化",
    "修复",
    "设计",
    "创建",
    "整合",
    "改造",
    "升级",
    "扩展",
    "调优",
    "开发",
    "搭建",
    "建立",
    "实现一个",
    "写一个",
    "做一个",
];

/// English implementation verbs.
const EN_IMPLEMENT_VERBS: &[&str] = &[
    "implement",
    "refactor",
    "add",
    "create",
    "build",
    "write",
    "migrate",
    "optimize",
    "fix",
    "design",
    "develop",
    "integrate",
    "upgrade",
    "extend",
    "rework",
];

/// Common file extensions that signal code references.
const CODE_EXTENSIONS: &[&str] = &[
    ".rs", ".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".java", ".kt", ".swift", ".c", ".cpp",
    ".h", ".hpp", ".css", ".scss", ".json", ".yaml", ".toml", ".md", ".sql",
];

/// Keywords for scope / requirement changes (amend detection).
const SCOPE_CHANGE_KEYWORDS_ZH: &[&str] = &[
    "增加",
    "修改",
    "补充",
    "额外",
    "调整",
    "变化",
    "新需求",
    "再加",
    "扩展",
    "改成",
    "还要",
    "对了",
    "等一下",
    "还有",
    "但是",
    "不过",
    "另外",
    "顺便",
    "追加",
    "变更",
    "改动一下",
    "换个方式",
    "重新",
];

const SCOPE_CHANGE_KEYWORDS_EN: &[&str] = &[
    "apart from",
    "also need",
    "additionally",
    "one more thing",
    "actually",
    "instead",
    "change",
    "update",
    "modify",
    "revise",
    "add",
    "extra",
    "new requirement",
    "on second thought",
    "by the way",
    "while you're at it",
];

/// Minimum task description length (trimmed) to be considered potentially complex.
const MIN_COMPLEX_CHARS: usize = 80;

/// Minimum count of matched indicators required to classify as "complex".
pub const COMPLEXITY_THRESHOLD: usize = 2;

// ── Result types ───────────────────────────────────────────────────────────

/// Result of complexity analysis on a user message.
#[derive(Debug, Clone)]
pub struct ComplexityResult {
    /// Whether the message appears to describe a complex task.
    pub is_complex: bool,
    /// Number of matched indicators out of the checked set.
    pub matched_count: usize,
    /// Names of matched indicators (for debugging / logging).
    pub matched_indicators: Vec<&'static str>,
    /// Whether the message appears to be a scope/requirement change to an active goal.
    pub is_scope_change: bool,
}

// ── Heuristic checks ───────────────────────────────────────────────────────

fn contains_implement_verb(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    ZH_IMPLEMENT_VERBS.iter().any(|v| text.contains(v))
        || EN_IMPLEMENT_VERBS.iter().any(|v| {
            // Match word boundaries for English
            let pat = format!(" {} ", v);
            lower.contains(&pat) || lower.starts_with(v) || lower.ends_with(v)
        })
}

fn references_code_paths(text: &str) -> bool {
    let count = CODE_EXTENSIONS
        .iter()
        .filter(|ext| text.contains(*ext))
        .count();
    // Also check for common path patterns
    let path_patterns = ["src/", "core/", "lib/", "app/", "components/"];
    let path_count = path_patterns.iter().filter(|p| text.contains(*p)).count();
    // At least 2 distinct file references
    (count + path_count) >= 2
}

fn task_description_long_enough(text: &str) -> bool {
    let trimmed = text.trim();
    // Count actual content characters (excluding leading/trailing whitespace)
    let content_len = trimmed.chars().filter(|c| !c.is_whitespace()).count();
    let has_chinese = trimmed.chars().any(|c| {
        let cp = c as u32;
        (0x4E00..=0x9FFF).contains(&cp) || (0x3400..=0x4DBF).contains(&cp)
    });
    if has_chinese {
        content_len > MIN_COMPLEX_CHARS
    } else {
        content_len > 150
    }
}

fn has_structured_markers(text: &str) -> bool {
    let markers = [
        "Goal:",
        "Non-goals:",
        "Done when:",
        "目标：",
        "非目标：",
        "完成条件：",
        "验证命令:",
        "Validation",
    ];
    let count = markers.iter().filter(|m| text.contains(*m)).count();
    count >= 2
}

fn has_multi_step_description(text: &str) -> bool {
    // Bullet points (markdown-style or numbered)
    let bullet_count = text
        .lines()
        .filter(|l| {
            let l = l.trim();
            l.starts_with("- ")
                || l.starts_with("* ")
                || l.starts_with("• ")
                || l.starts_with(|c: char| c.is_ascii_digit()) && l[1..].starts_with(". ")
        })
        .count();
    // Numbered inline: "1)", "2)", "3)"
    let numbered_inline = ["1.", "2.", "3.", "1)", "2)", "3)"]
        .iter()
        .filter(|n| text.contains(*n))
        .count();
    (bullet_count + numbered_inline) >= 3
}

fn references_multiple_crates_or_modules(text: &str) -> bool {
    // Module/crate path patterns
    let path_patterns = [
        "core/",
        "src/",
        "lib/",
        "app/",
        "components/",
        "utils/",
        "services/",
        "models/",
    ];
    let path_count = path_patterns.iter().filter(|p| text.contains(*p)).count();
    // Common multi-crate references: crate names in Rust/Cargo context
    let crate_refs = [
        "crate::",
        "mod ",
        "use ",
        "import ",
        "require(",
        "from '",
        "from \"",
        "package ",
        "namespace ",
    ];
    let ref_count = crate_refs.iter().filter(|r| text.contains(*r)).count();
    (path_count + ref_count) >= 2
}

// ── Scope change detection ─────────────────────────────────────────────────

/// Detect if a message suggests a scope / requirement change to an active goal.
/// Called when there IS already an active goal.
pub fn detect_scope_change(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let zh_hit = SCOPE_CHANGE_KEYWORDS_ZH.iter().any(|k| text.contains(k));
    let en_hit = SCOPE_CHANGE_KEYWORDS_EN.iter().any(|k| lower.contains(k));
    zh_hit || en_hit
}

// ── Public API ─────────────────────────────────────────────────────────────

/// Analyze a user message for task complexity.
///
/// Returns a [`ComplexityResult`] indicating whether the message describes a
/// complex task that would benefit from a structured Goal contract, and
/// whether it appears to be a scope change to an existing goal.
///
/// The analysis uses multiple heuristic indicators (implement verbs, file
/// references, length, structured markers, multi-step, cross-module refs).
/// At least [`COMPLEXITY_THRESHOLD`] indicators must match to be classified
/// as "complex".
pub fn analyze_complexity(text: &str) -> ComplexityResult {
    let mut matched: Vec<&'static str> = Vec::new();

    if contains_implement_verb(text) {
        matched.push("implementation_verb");
    }
    if references_code_paths(text) {
        matched.push("code_path_references");
    }
    if task_description_long_enough(text) {
        matched.push("long_description");
    }
    if has_structured_markers(text) {
        matched.push("structured_markers");
    }
    if has_multi_step_description(text) {
        matched.push("multi_step");
    }
    if references_multiple_crates_or_modules(text) {
        matched.push("cross_module_or_crate");
    }

    let is_complex = matched.len() >= COMPLEXITY_THRESHOLD;
    let is_scope_change = if is_complex {
        // Only check scope change for messages that aren't clearly new task descriptions
        false
    } else {
        detect_scope_change(text)
    };

    ComplexityResult {
        is_complex,
        matched_count: matched.len(),
        matched_indicators: matched,
        is_scope_change,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn complex_implementation_task_zh() {
        let text =
            "重构 core/state_manager 模块中的 goal_ops.rs，需要拆分 checkpoint 逻辑到单独的文件，
            修改 start/complete/clear 的调用链，添加 amend 操作支持。涉及 3 个文件变更。";
        let r = analyze_complexity(text);
        assert!(r.is_complex, "expected complex: {:?}", r.matched_indicators);
    }

    #[test]
    fn complex_implementation_task_en() {
        let text = "I need to implement a new routing algorithm in src/router.rs.
            It should support dynamic scoring and fallback routing across 5 different backends.
            The implementation will modify core/route.rs and core/scoring.rs.";
        let r = analyze_complexity(text);
        assert!(r.is_complex, "expected complex: {:?}", r.matched_indicators);
    }

    #[test]
    fn simple_question_not_complex() {
        let text = "今天天气怎么样？";
        let r = analyze_complexity(text);
        assert!(!r.is_complex, "not expected complex");
    }

    #[test]
    fn simple_english_not_complex() {
        let text = "What's the capital of France?";
        let r = analyze_complexity(text);
        assert!(!r.is_complex, "not expected complex");
    }

    #[test]
    fn short_code_fix_not_complex() {
        let text = "Fix the typo in main.rs line 42";
        let r = analyze_complexity(text);
        // Only 1 indicator (code_path_references), need 2+
        assert!(!r.is_complex, "one indicator only");
    }

    #[test]
    fn scope_change_detected() {
        let text = "对了，再加一个导出函数到 lib.rs";
        assert!(detect_scope_change(text), "scope change expected");
    }

    #[test]
    fn scope_change_detected_en() {
        let text = "Actually, I also need to add authentication to the login flow.";
        assert!(detect_scope_change(text), "scope change expected");
    }

    #[test]
    fn no_false_positive_for_normal_scenarios() {
        let text = "Let me review what we've done so far.";
        assert!(!detect_scope_change(text), "not a scope change");
    }

    #[test]
    fn empty_or_minimal_text_not_complex() {
        assert!(!analyze_complexity("").is_complex);
        assert!(!analyze_complexity("ok").is_complex);
        assert!(!analyze_complexity("好的").is_complex);
    }

    #[test]
    fn multi_step_structured_task() {
        let text = "我需要完成以下步骤：
            1. 创建新的 trait
            2. 实现默认行为
            3. 编写单元测试
            4. 更新文档注释";
        let r = analyze_complexity(text);
        assert!(r.is_complex, "multi-step task");
    }

    #[test]
    fn mixed_zh_en_complex_task() {
        let text = "重构 core/state_manager 模块中的 goal_ops.rs。refactor the amend operation to support natural language updates. 添加 3 个新的 test cases.";
        let r = analyze_complexity(text);
        assert!(r.is_complex, "mixed zh/en task should be complex");
    }

    #[test]
    fn scope_change_not_complex_only() {
        // Scope change without complex indicators should NOT be complex
        let text = "对了，再加一个功能";
        let r = analyze_complexity(text);
        assert!(!r.is_complex, "simple scope change is not a complex task");
        assert!(r.is_scope_change, "should detect scope change");
    }

    #[test]
    fn cross_crate_refs_detected() {
        let text = "修改 core/foo/src/lib.rs 和 core/bar/src/lib.rs 以及 core/baz/src/config.rs";
        let r = analyze_complexity(text);
        assert!(r.is_complex, "cross-crate refs should be complex");
        let has_cross = r.matched_indicators.contains(&"cross_module_or_crate");
        assert!(
            has_cross,
            "cross-module indicator should match: {:?}",
            r.matched_indicators
        );
    }

    #[test]
    fn long_english_description_complex() {
        let text = "I need to implement a complete user authentication system with JWT tokens, \
                    including login, registration, password reset, and email verification. \
                    This will require changes to the auth module, user model, email service, \
                    and frontend login page at src/auth/, src/models/, and src/services/.";
        let r = analyze_complexity(text);
        assert!(r.is_complex, "long english with multiple references");
    }

    #[test]
    fn scope_change_not_detected_for_simple_questions() {
        let cases = [
            "好的",
            "继续",
            "明白了",
            "下一步",
            "yes",
            "ok",
            "looks good",
            "继续推进",
        ];
        for case in &cases {
            assert!(!detect_scope_change(case), "not scope change: {case:?}");
        }
    }
}
