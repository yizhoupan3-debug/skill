use super::{ParsedEdge, ParsedSymbol};

pub(crate) struct ParseOutput {
    pub symbols: Vec<ParsedSymbol>,
    pub edges: Vec<ParsedEdge>,
}

enum FenceStyle {
    Backtick,
    Tilde,
}

/// Lightweight Markdown parser that extracts headings for codegraph indexing.
///
/// Design choices (intentional):
/// - Only ATX headings (`#` prefix) — no setext support (underlined `===`)
/// - Full body text not indexed — only heading text is searchable via FTS5
/// - No inline parsing (bold, links, etc.)
/// - Handles: fenced code blocks (``` and ~~~, mixed), indented code blocks
///   (4+ spaces), YAML frontmatter (title extraction)
pub(crate) fn parse(source: &str) -> ParseOutput {
    let mut symbols = Vec::new();
    let mut active_fence: Option<FenceStyle> = None;
    let mut in_frontmatter = false;
    let mut frontmatter_title: Option<String> = None;
    // Setext heading tracking: previous line candidate
    let mut prev_line: Option<&str> = None;
    let mut prev_idx: usize = 0;

    for (idx, line) in source.lines().enumerate() {
        let leading_spaces = line.len() - line.trim_start().len();

        // -- YAML frontmatter detection --
        if idx == 0 && line.trim() == "---" {
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter {
            if line.trim() == "---" {
                in_frontmatter = false;
                continue;
            }
            if let Some(rest) = line.trim_start().strip_prefix("title:") {
                frontmatter_title =
                    Some(rest.trim().trim_matches('"').trim_matches('\'').to_string());
            }
            continue;
        }

        let trimmed = line.trim();

        // -- Fenced code block handling (with style tracking) --
        let is_backtick_fence = trimmed.starts_with("```");
        let is_tilde_fence = trimmed.starts_with("~~~");
        if is_backtick_fence || is_tilde_fence {
            match &active_fence {
                None => {
                    active_fence = Some(if is_backtick_fence {
                        FenceStyle::Backtick
                    } else {
                        FenceStyle::Tilde
                    });
                    prev_line = None;
                    continue;
                }
                Some(style) => {
                    let same_style = matches!(
                        (style, is_backtick_fence, is_tilde_fence),
                        (FenceStyle::Backtick, true, _) | (FenceStyle::Tilde, _, true)
                    );
                    if same_style {
                        active_fence = None;
                    }
                    prev_line = None;
                    continue;
                }
            }
        }
        if active_fence.is_some() {
            prev_line = None;
            continue;
        }

        // -- Indented code block (4+ leading spaces) --
        if leading_spaces >= 4 {
            prev_line = None;
            continue;
        }

        // -- Setext heading: text followed by === or --- underline --
        if !trimmed.is_empty()
            && trimmed.chars().all(|c| c == '=' || c == '-')
            && trimmed.len() >= 3
        {
            let level = if trimmed.starts_with('=') { 1 } else { 2 };
            if let Some(text) = prev_line {
                let clean = text.trim();
                if !clean.is_empty() {
                    let kind = if level == 1 { "section" } else { "subsection" };
                    symbols.push(ParsedSymbol {
                        symbol: clean.to_string(),
                        kind: kind.to_string(),
                        line: (prev_idx + 1) as u32,
                        start_col: 0,
                        end_line: 0,
                        end_col: 0,
                    });
                }
            }
            prev_line = None;
            continue;
        }

        // -- ATX headings: ^#{1,6} text --
        let text_without_hash = trimmed.trim_start_matches('#');
        let pound_count = trimmed.len() - text_without_hash.len();
        if (1..=6).contains(&pound_count) {
            let rest = text_without_hash.trim();
            if !rest.is_empty() {
                let kind = if pound_count <= 2 {
                    "section"
                } else {
                    "subsection"
                };
                symbols.push(ParsedSymbol {
                    symbol: rest.to_string(),
                    kind: kind.to_string(),
                    line: (idx + 1) as u32,
                    start_col: 0,
                    end_line: 0,
                    end_col: 0,
                });
            }
            prev_line = None;
            continue;
        }

        // Track non-empty line for setext heading detection
        if trimmed.is_empty() {
            prev_line = None;
        } else {
            prev_line = Some(line);
            prev_idx = idx;
        }
    }

    // Add frontmatter title as a symbol if no heading captured it
    if let Some(title) = frontmatter_title {
        let already_has_title = symbols.iter().any(|s| s.symbol == title);
        if !already_has_title {
            symbols.push(ParsedSymbol {
                symbol: title,
                kind: "section".to_string(),
                line: 1,
                start_col: 0,
                end_line: 0,
                end_col: 0,
            });
        }
    }

    ParseOutput {
        symbols,
        edges: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn extracts_headings() {
        let src = "# Main Title\n\n## Section 1\n\nSome text\n\n### Sub-section\n\n## Section 2\n";
        let out = parse(src);
        assert_eq!(out.symbols.len(), 4);
        assert_eq!(out.symbols[0].symbol, "Main Title");
        assert_eq!(out.symbols[0].kind, "section");
        assert_eq!(out.symbols[0].line, 1);
        assert_eq!(out.symbols[1].symbol, "Section 1");
        assert_eq!(out.symbols[1].kind, "section");
        assert_eq!(out.symbols[2].symbol, "Sub-section");
        assert_eq!(out.symbols[2].kind, "subsection");
        assert_eq!(out.symbols[3].symbol, "Section 2");
    }

    #[test]
    fn skips_headings_in_fenced_code_blocks() {
        let src = "# Real Heading\n\n```markdown\n# Fake Heading\n```\n\n## Another\n";
        let out = parse(src);
        assert_eq!(out.symbols.len(), 2);
        assert_eq!(out.symbols[0].symbol, "Real Heading");
        assert_eq!(out.symbols[1].symbol, "Another");
    }

    #[test]
    fn skips_headings_in_tilde_fenced_code() {
        let src = "# Real\n\n~~~python\n# Fake\n~~~\n\n## Another\n";
        let out = parse(src);
        assert_eq!(out.symbols.len(), 2);
        assert_eq!(out.symbols[0].symbol, "Real");
        assert_eq!(out.symbols[1].symbol, "Another");
    }

    #[test]
    fn mixed_fence_styles_do_not_interfere() {
        // A ~~~ block stays open even if a ``` appears inside it
        let src = "# Outside\n\n~~~\n```rust\n# Inside tilde, should not be heading\n~~~\n";
        let out = parse(src);
        assert_eq!(out.symbols.len(), 1);
        assert_eq!(out.symbols[0].symbol, "Outside");
    }

    #[test]
    fn indented_code_block_not_heading() {
        let src = "# Real Heading\n\n    # Indented — not a heading\n\n## Another\n";
        let out = parse(src);
        assert_eq!(out.symbols.len(), 2);
        assert_eq!(out.symbols[0].symbol, "Real Heading");
        assert_eq!(out.symbols[1].symbol, "Another");
    }

    #[test]
    fn leading_spaces_upto_3_are_valid_headings() {
        let src =
            " # Level-1 with 1 space\n  ## Level-2 with 2 spaces\n   ### Level-3 with 3 spaces\n";
        let out = parse(src);
        assert_eq!(out.symbols.len(), 3);
    }

    #[test]
    fn frontmatter_title_is_extracted() {
        let src = "---\ntitle: Installation Guide\n---\n\nSome content.\n";
        let out = parse(src);
        assert_eq!(out.symbols.len(), 1);
        assert_eq!(out.symbols[0].symbol, "Installation Guide");
        assert_eq!(out.symbols[0].kind, "section");
    }

    #[test]
    fn frontmatter_title_does_not_dup_heading() {
        let src = "---\ntitle: My Doc\n---\n\n# My Doc\n\nContent.\n";
        let out = parse(src);
        assert_eq!(out.symbols.len(), 1);
        assert_eq!(out.symbols[0].symbol, "My Doc");
    }

    #[test]
    fn empty_frontmatter_does_not_cause_issues() {
        let src = "---\n---\n\n# Actual Heading\n";
        let out = parse(src);
        assert_eq!(out.symbols.len(), 1);
        assert_eq!(out.symbols[0].symbol, "Actual Heading");
    }

    #[test]
    fn no_headings_returns_empty() {
        let src = "Just a paragraph.\n\nAnother paragraph.\n";
        let out = parse(src);
        assert!(out.symbols.is_empty());
    }

    #[test]
    fn no_edges_produced() {
        let src = "# One\n## Two\n";
        let out = parse(src);
        assert!(out.edges.is_empty());
    }

    #[test]
    fn setext_heading_level_1() {
        let src = "My Title\n=======\n\nSome content.\n";
        let out = parse(src);
        assert_eq!(out.symbols.len(), 1);
        assert_eq!(out.symbols[0].symbol, "My Title");
        assert_eq!(out.symbols[0].kind, "section");
    }

    #[test]
    fn setext_heading_level_2() {
        let src = "# Top\n\nSub Section\n-------\n\nContent.\n";
        let out = parse(src);
        assert_eq!(out.symbols.len(), 2);
        assert_eq!(out.symbols[0].symbol, "Top");
        assert_eq!(out.symbols[1].symbol, "Sub Section");
        assert_eq!(out.symbols[1].kind, "subsection");
    }

    #[test]
    fn setext_not_fooled_by_code_fence() {
        // --- inside a code block should not create a setext heading
        let src = "# Real\n\n```\n-----\n```\n\n## Also Real\n";
        let out = parse(src);
        assert_eq!(out.symbols.len(), 2);
        assert_eq!(out.symbols[0].symbol, "Real");
        assert_eq!(out.symbols[1].symbol, "Also Real");
    }

    #[test]
    fn no_false_short_dashes() {
        // Lines like "---" (≥3) with only dashes should be skipped if prev line is empty
        let src = "Some text\n\n---\n\nMore text\n";
        let out = parse(src);
        assert!(out.symbols.is_empty());
    }
}
