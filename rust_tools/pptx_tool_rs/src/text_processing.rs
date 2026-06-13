// Extracted from lib.rs: text_processing.


pub fn naturalize_copy_text(input: &str) -> String {
    let mut text = clean_copy_spacing(input);
    let replacements = [
        ("核心观点如下：", ""),
        ("核心观点如下:", ""),
        ("请重点关注", "重点看"),
        ("保持叙事连贯性", "让转场自然"),
        ("结合实际选取最优方案", "回到现场约束再取舍"),
        ("具有重要意义", "会影响具体决策"),
        ("多维度", "几个角度"),
        ("赋能", "支持"),
        ("打造", "做"),
        ("显著提升", "提高"),
        ("持续优化", "继续改"),
        ("进一步提升", "提高"),
        ("全面提升", "提高"),
        ("全方位", ""),
        ("一站式", ""),
        ("方法论", ""),
        ("顶层设计", ""),
        ("全链路", ""),
        ("闭环", ""),
        ("抓手", ""),
        ("生态", ""),
        ("矩阵", ""),
        ("综上所述，", ""),
        ("综上所述,", ""),
        ("值得关注的是，", ""),
        ("值得关注的是,", ""),
        ("This slide presents ", ""),
        ("This slide shows ", ""),
        ("This slide introduces ", ""),
        ("It is important to note that ", ""),
    ];
    for (from, to) in replacements {
        text = text.replace(from, to);
    }

    for prefix in [
        "本页主要展示了",
        "本页主要展示",
        "本页重点展示了",
        "本页重点展示",
        "本页展示了",
        "本页展示",
        "本页呈现了",
        "本页呈现",
        "本页介绍了",
        "本页介绍",
        "本页说明了",
        "本页说明",
        "本页从多个维度展开分析，",
        "本页从多个维度展开分析,",
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest.to_string();
            break;
        }
    }

    // Remove common template-ish wrappers while keeping the core claim.
    // Keep this conservative to avoid harming legitimate content.
    for prefix in [
        "我们认为，",
        "我们认为,",
        "我们将，",
        "我们将,",
        "我们会，",
        "我们会,",
        "需要指出的是，",
        "需要指出的是,",
        "需要说明的是，",
        "需要说明的是,",
    ] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest.to_string();
            break;
        }
    }

    clean_copy_spacing(&text)
}

pub fn clean_copy_spacing(input: &str) -> String {
    // Preserve newlines (often intentional in PPT), but normalize excessive spaces/tabs.
    let mut out = String::with_capacity(input.len());
    let mut in_space = false;
    for ch in input.chars() {
        match ch {
            '\n' => {
                // trim trailing spaces before newline
                while out.ends_with(' ') {
                    out.pop();
                }
                out.push('\n');
                in_space = false;
            }
            '\r' => {}
            ch if ch.is_whitespace() => {
                if !in_space {
                    out.push(' ');
                    in_space = true;
                }
            }
            _ => {
                out.push(ch);
                in_space = false;
            }
        }
    }
    out.trim().to_string()
}

pub fn reflow_outline_slides(slides: Option<&Vec<Value>>) -> Vec<Value> {
    let mut out = Vec::new();
    for slide in slides.into_iter().flatten() {
        let pattern = detect_outline_pattern(slide);
        if pattern == "multi-card" && value_array_len(slide, "bullets") > 4 {
            push_chunked_slide(&mut out, slide, "bullets", 4);
        } else if pattern == "process-flow" && value_array_len(slide, "steps") > 5 {
            push_chunked_slide(&mut out, slide, "steps", 4);
        } else if pattern == "timeline" && value_array_len(slide, "timeline") > 5 {
            push_chunked_slide(&mut out, slide, "timeline", 4);
        } else if pattern == "image-text-split" && joined_array_chars(slide, "bullets") > 150 {
            push_split_slide(&mut out, slide, "bullets");
        } else {
            out.push(slide.clone());
        }
    }
    out
}

pub fn push_chunked_slide(out: &mut Vec<Value>, slide: &Value, key: &str, chunk_size: usize) {
    let Some(items) = slide.get(key).and_then(Value::as_array) else {
        out.push(slide.clone());
        return;
    };
    let chunks: Vec<&[Value]> = items.chunks(chunk_size).collect();
    for (idx, chunk) in chunks.iter().enumerate() {
        let mut cloned = slide.as_object().cloned().unwrap_or_default();
        cloned.insert(
            "title".to_string(),
            Value::String(format!(
                "{} ({}/{})",
                outline_str(slide, "title", "Untitled"),
                idx + 1,
                chunks.len()
            )),
        );
        cloned.insert(key.to_string(), Value::Array(chunk.to_vec()));
        out.push(Value::Object(cloned));
    }
}

pub fn push_split_slide(out: &mut Vec<Value>, slide: &Value, key: &str) {
    let Some(items) = slide.get(key).and_then(Value::as_array) else {
        out.push(slide.clone());
        return;
    };
    let mid = items.len().div_ceil(2);
    for (idx, chunk) in [&items[..mid], &items[mid..]].iter().enumerate() {
        let mut cloned = slide.as_object().cloned().unwrap_or_default();
        cloned.insert(
            "title".to_string(),
            Value::String(format!(
                "{} ({}/2)",
                outline_str(slide, "title", "Untitled"),
                idx + 1
            )),
        );
        cloned.insert(key.to_string(), Value::Array(chunk.to_vec()));
        out.push(Value::Object(cloned));
    }
}
