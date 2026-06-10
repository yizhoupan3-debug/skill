/// Split extracted PDF text into overlapping chunks for FTS indexing.
pub fn chunk_text(text: &str, max_chars: usize, overlap: usize) -> Vec<(usize, String)> {
    let normalized = text.replace('\r', "");
    let paragraphs: Vec<&str> = normalized
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut page_hint = 1usize;
    for para in paragraphs {
        if para.contains('\x0c') {
            page_hint += para.matches('\x0c').count().max(1);
        }
        if buf.is_empty() {
            buf.push_str(para);
        } else if buf.len() + 2 + para.len() <= max_chars {
            buf.push_str("\n\n");
            buf.push_str(para);
        } else {
            flush_chunk(&mut out, page_hint, &buf);
            buf = tail_overlap(&buf, overlap);
            if !buf.is_empty() {
                buf.push_str("\n\n");
            }
            buf.push_str(para);
        }
        while buf.len() > max_chars {
            let (head, rest) = split_at_char_boundary(&buf, max_chars);
            flush_chunk(&mut out, page_hint, head);
            buf = format!("{}{}", tail_overlap(head, overlap), rest);
        }
    }
    if !buf.trim().is_empty() {
        flush_chunk(&mut out, page_hint, &buf);
    }
    out
}

fn flush_chunk(out: &mut Vec<(usize, String)>, page_hint: usize, text: &str) {
    let t = text.trim();
    if !t.is_empty() {
        out.push((page_hint.max(1), t.to_string()));
    }
}

fn tail_overlap(s: &str, overlap: usize) -> String {
    if overlap == 0 || s.is_empty() {
        return String::new();
    }
    let take = s.chars().count().min(overlap);
    s.chars().rev().take(take).collect::<String>().chars().rev().collect()
}

fn split_at_char_boundary(s: &str, max_chars: usize) -> (&str, &str) {
    if s.len() <= max_chars {
        return (s, "");
    }
    let mut end = max_chars;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s.split_at(end)
}

#[cfg(test)]
mod tests {
    use super::chunk_text;

    #[test]
    fn chunks_long_paragraph() {
        let text = "word ".repeat(500);
        let chunks = chunk_text(&text, 400, 40);
        assert!(chunks.len() >= 2);
    }
}
