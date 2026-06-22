//! Compile-time Codex agent policy embedding (`AGENTS.md`).

pub fn build_codex_agent_policy() -> String {
    include_str!("../../../../../../AGENTS.md").to_string()
}
