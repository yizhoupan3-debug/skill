// Cursor hook outbound context truncation + silent/output policy (P4 handlers split).
/// When `ROUTER_RS_CURSOR_HOOK_SILENT=1`: drop advisory `additional_context`; keep hard
/// `followup_message` lines that start with the `router-rs ` leader prefix.
pub(crate) fn apply_cursor_hook_silent_policy(output: &mut Value) {
    if !crate::router_env_flags::router_rs_cursor_hook_silent_enabled() {
        return;
    }
    if let Some(obj) = output.as_object_mut() {
        obj.remove("additional_context");
    }
    if let Some(Value::String(s)) = output.get_mut("followup_message") {
        let kept: Vec<&str> = s
            .lines()
            .filter(|line| line.trim_start().starts_with("router-rs "))
            .collect();
        if kept.is_empty() {
            if let Some(obj) = output.as_object_mut() {
                obj.remove("followup_message");
            }
        } else {
            *s = kept.join("\n");
        }
    }
}

pub(crate) fn apply_cursor_hook_output_policy(output: &mut Value) {
    crate::router_rs_observation::attach_router_rs_observation(
        output,
        crate::router_rs_observation::HookObservationHost::Cursor,
    );
    let max_out = crate::router_env_flags::router_rs_cursor_hook_outbound_context_max_bytes();
    if let Some(Value::String(s)) = output.get_mut("additional_context") {
        let next = truncate_cursor_hook_outbound_context_preserving_gate(s.as_str(), max_out);
        *s = next;
    }

    let absurd_followup_threshold =
        crate::router_env_flags::router_rs_cursor_hook_outbound_context_max_bytes()
            .saturating_mul(4)
            .max(32 * 1024);
    if let Some(Value::String(s)) = output.get_mut("followup_message") {
        if s.len() > absurd_followup_threshold {
            *s = truncate_cursor_hook_followup_preserving_review_gate(s.as_str(), max_out);
        }
    }
}

/// Cursor outbound truncation: UTF-8 byte cap; prefix retained; **fixed suffix** so operators can
/// tell budget clipping from gate logic. (Variable names may say `_CHARS`; semantics are bytes.)
pub(crate) const CURSOR_HOOK_OUTBOUND_TRUNC_SUFFIX: &str = "...[~trunc]";

/// Cursor 出站 `additional_context` / 极端 `followup_message`：**UTF-8 字节预算**，前缀优先，末尾固定
/// [`CURSOR_HOOK_OUTBOUND_TRUNC_SUFFIX`]（与 Codex `truncate_codex_additional_context_bytes` 的 `...` 相比更可观测）。
fn truncate_cursor_hook_outbound_context(combined: &str, max_bytes: usize) -> String {
    if combined.len() <= max_bytes {
        return combined.to_string();
    }
    // `combined` may be borrowed; allocation only when truncating.
    let suf = CURSOR_HOOK_OUTBOUND_TRUNC_SUFFIX;
    let suf_len = suf.len();
    if max_bytes <= suf_len {
        let mut cut = max_bytes.min(combined.len());
        while cut > 0 && !combined.is_char_boundary(cut) {
            cut -= 1;
        }
        return combined[..cut].to_string();
    }
    let budget = max_bytes.saturating_sub(suf_len);
    let mut cut = budget.min(combined.len());
    while cut > 0 && !combined.is_char_boundary(cut) {
        cut -= 1;
    }
    if let Some(pos) = combined[..cut].rfind('\n') {
        if pos > 0 {
            cut = pos;
        }
    }
    while cut > 0 && !combined.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}{}", &combined[..cut], suf)
}

fn cursor_hook_outbound_line_is_protected(line: &str) -> bool {
    crate::hook_outbound_protect::hook_outbound_line_is_framework_protected(line)
}

fn truncate_cursor_hook_lines_preserving<F>(
    combined: &str,
    max_bytes: usize,
    is_protected: F,
) -> String
where
    F: Fn(&str) -> bool,
{
    if combined.len() <= max_bytes {
        return combined.to_string();
    }
    let mut protected: Vec<&str> = Vec::new();
    let mut rest: Vec<&str> = Vec::new();
    for line in combined.lines() {
        if is_protected(line) {
            protected.push(line);
        } else {
            rest.push(line);
        }
    }
    let protected_body = protected.join("\n");
    if protected_body.len() >= max_bytes {
        return truncate_cursor_hook_outbound_context(&protected_body, max_bytes);
    }
    let rest_body = rest.join("\n");
    if rest_body.is_empty() {
        return protected_body;
    }
    let sep_len = if protected_body.is_empty() { 0 } else { 1 };
    let rest_budget = max_bytes.saturating_sub(protected_body.len() + sep_len);
    let truncated_rest = truncate_cursor_hook_outbound_context(&rest_body, rest_budget);
    if protected_body.is_empty() {
        truncated_rest
    } else if truncated_rest.is_empty() {
        protected_body
    } else {
        let mut out = protected_body;
        out.push('\n');
        out.push_str(&truncated_rest);
        if out.len() > max_bytes {
            truncate_cursor_hook_outbound_context(&out, max_bytes)
        } else {
            out
        }
    }
}

/// Outbound truncation: keep REVIEW_GATE / continuity_suppressed lines; truncate filler.
pub(crate) fn truncate_cursor_hook_outbound_context_preserving_gate(
    combined: &str,
    max_bytes: usize,
) -> String {
    truncate_cursor_hook_lines_preserving(combined, max_bytes, cursor_hook_outbound_line_is_protected)
}

fn truncate_cursor_hook_followup_preserving_review_gate(
    combined: &str,
    max_bytes: usize,
) -> String {
    truncate_cursor_hook_lines_preserving(combined, max_bytes, |line| {
        line.trim_start().starts_with("router-rs REVIEW_GATE")
    })
}
