//! Compile-time Codex agent policy embedding (`AGENTS.md` + `AGENTS_CODEX.md`).

pub fn build_codex_agent_policy() -> String {
    format!(
        "{}\n\n---\n\n{}",
        include_str!("../../../../../AGENTS.md"),
        include_str!("../../../../../AGENTS_CODEX.md")
    )
}
