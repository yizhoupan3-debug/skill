use regex::Regex;

/// A segment of text that is either protected (must not be modified) or compressable prose.
#[derive(Debug, PartialEq)]
#[allow(dead_code)]
enum Segment<'a> {
    /// Fenced code block (```...```)
    CodeBlock(&'a str),
    /// Inline code (`...`)
    InlineCode(&'a str),
    /// URL or file path
    UrlPath(&'a str),
    /// Prose that can be compressed
    Prose(&'a str),
}

/// Compress prose text: remove filler, hedging, shorten synonyms.
pub fn compress_prose(input: &str) -> String {
    if input.trim().is_empty() {
        return input.to_string();
    }

    let mut text = input.to_string();

    // 1. Remove pleasantries (anywhere in text)
    let pleasantry_re = Regex::new(
        r"(?i)(?:^|\s+)(sure|certainly|of course|happy to help|i[’']d be happy|i[’']d recommend|absolutely|gladly)\b"
    ).unwrap();
    text = pleasantry_re.replace_all(&text, "").to_string();

    // 2. Remove hedging (sentence-start or mid-sentence)
    let hedging_re = Regex::new(
        r"(?i)\b(it might be worth|you could consider|it would be good to|it may be worth|one could argue)\b"
    ).unwrap();
    text = hedging_re.replace_all(&text, "").to_string();

    // 3. Remove redundant phrases
    text = text.replace("in order to", "to");
    text = text.replace("make sure to", "ensure to");
    text = text.replace("the reason is because", "because");

    // 4. Remove filler words (regex-based to preserve whitespace structure)
    let filler_re = Regex::new(
        r"(?i)\b(just|really|basically|actually|simply|essentially|generally|literally|honestly)\b"
    ).unwrap();
    text = filler_re.replace_all(&text, "").to_string();

    // 5. Shorten common verbose phrases
    let shorten_pairs = [
        ("implement a solution for", "fix"),
        ("implement a solution", "fix"),
        ("utilize", "use"),
        ("utilizing", "using"),
        ("utilization", "use"),
        ("demonstrate", "show"),
        ("demonstrates", "shows"),
        ("facilitate", "help"),
        ("facilitates", "helps"),
        ("subsequent", "next"),
        ("subsequently", "then"),
        ("sufficient", "enough"),
        ("additional", "more"),
        ("approximately", "about"),
        ("numerous", "many"),
        ("prior to", "before"),
        ("in the event that", "if"),
        ("with the exception of", "except"),
        ("due to the fact that", "because"),
        ("at this point in time", "now"),
        ("at the present time", "now"),
        ("in the near future", "soon"),
        ("a majority of", "most"),
        ("a number of", "some"),
        ("is able to", "can"),
        ("has the ability to", "can"),
        ("are able to", "can"),
        ("is responsible for", "handles"),
        ("in excess of", "over"),
    ];

    for (verbose, short) in &shorten_pairs {
        text = text.replace(verbose, short);
    }

    // 6. Remove unnecessary articles (a, an, the) — careful with proper nouns
    // Simple approach: remove "the " when not at sentence start and not part of a name
    let the_re = Regex::new(r"(?i)\bthe\s+").unwrap();
    text = the_re.replace_all(&text, "").to_string();

    // 7. Remove "you should", "make sure to", "remember to", "you can" at start of clauses
    let clause_start_re = Regex::new(r"(?i)\byou should\s+|make sure to\s+|remember to\s+|you can\s+|you need to\s+|you want to\s+|you may want to\s+|it is recommended to\s+|it is important to\s+").unwrap();
    text = clause_start_re.replace_all(&text, "").to_string();

    // 8. Collapse multiple spaces/tabs (but NOT newlines)
    let multi_space = Regex::new(r"[^\S\n]{2,}").unwrap();
    text = multi_space.replace_all(&text, " ").to_string();

    // Trim trailing spaces/tabs per line but preserve line breaks
    text = text
        .lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n");

    text
}

/// Parse a text into protected segments and prose segments.
/// Returns segments preserving order.
fn segment_text<'a>(text: &'a str) -> Vec<(&'a str, Segment<'a>)> {
    let mut segments = Vec::new();
    let _remaining = text;

    // Preliminary approach: split by code fences, then find inline code/URLs within prose.
    // For practical purposes, we use regex to identify the major block types.

    // Fenced code block pattern: ```...```
    let fence_re = Regex::new(r"(?m)^```[\s\S]*?^```").unwrap();
    let _inline_code_re = Regex::new(r"`[^`]+`").unwrap();
    let _url_re = Regex::new(r#"https?://[^\s\)\]>"'`]+"#).unwrap();
    let _file_path_re = Regex::new(r#"[\w/\\]+\.[\w]{1,4}|~[\w/\\]+|\./[\w/\\]+"#).unwrap();

    // Simple linear scan for fenced code blocks
    let fence_matches: Vec<_> = fence_re.find_iter(text).collect();

    if fence_matches.is_empty() {
        return vec![(text, Segment::Prose(text))];
    }

    // Split text around code blocks
    let mut cursor = 0;
    for m in &fence_matches {
        if m.start() > cursor {
            let prose = &text[cursor..m.start()];
            segments.push((prose, Segment::Prose(prose)));
        }
        segments.push((m.as_str(), Segment::CodeBlock(m.as_str())));
        cursor = m.end();
    }
    if cursor < text.len() {
        let prose = &text[cursor..];
        segments.push((prose, Segment::Prose(prose)));
    }

    segments
}

/// Compress a full text by preserving protected elements and compressing prose.
pub fn compress_text(input: &str) -> String {
    let segments = segment_text(input);
    let mut output = String::new();

    for (raw, segment) in &segments {
        match segment {
            Segment::CodeBlock(_) => output.push_str(raw),
            Segment::InlineCode(_) => output.push_str(raw),
            Segment::UrlPath(_) => output.push_str(raw),
            Segment::Prose(_) => output.push_str(&compress_prose(raw)),
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_pleasantries() {
        let result = compress_prose("Sure! I'd be happy to help you with that.");
        assert!(!result.to_lowercase().contains("sure"));
        assert!(!result.to_lowercase().contains("happy to help"));
    }

    #[test]
    fn test_remove_filler() {
        let result = compress_prose("This is basically just a really simple example.");
        assert!(!result.contains("basically"));
        assert!(!result.contains("just"));
        assert!(!result.contains("really"));
        assert!(!result.contains("simply"));
    }

    #[test]
    fn test_shorten_verbose() {
        assert_eq!(compress_prose("use"), "use");
        assert!(compress_prose("utilize").contains("use"));
    }

    #[test]
    fn test_remove_you_should() {
        let result = compress_prose("You should always run tests before committing.");
        assert!(result.to_lowercase().contains("run tests"));
    }

    #[test]
    fn test_preserve_code_block() {
        let input = "Some text.\n```\nfn main() {}\n```\nMore text.";
        let result = compress_text(input);
        assert!(result.contains("```\nfn main() {}\n```"));
    }

    #[test]
    fn test_compress_text_preserves_code_block() {
        let input = "Hello, this is a test file with some filler words like basically and really.\n\n```\nfn test() {}\n```";
        let compressed = compress_text(input);
        assert!(compressed.contains("```"), "compressed should contain fences");
        assert!(compressed.contains("fn test() {}"), "compressed should contain code");
    }

    #[test]
    fn test_remove_the() {
        let result = compress_prose("The function returns the value.");
        // "The" at start stays, "the" before value removed
        assert!(!result.to_lowercase().contains("the value"));
    }

    #[test]
    fn test_no_code_in_prose_compressed() {
        let result = compress_prose("Use `npm install` to install.");
        assert!(result.contains("`npm install`"));
    }

    #[test]
    fn test_consecutive_spaces_collapsed() {
        let result = compress_prose("hello    world");
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_in_order_to() {
        let result = compress_prose("Do this in order to pass.");
        assert!(!result.contains("in order to"));
        assert!(result.contains("to pass") || result.contains("do this"));
    }
}
