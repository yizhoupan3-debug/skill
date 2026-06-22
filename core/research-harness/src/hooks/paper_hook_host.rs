//! PaperProseHookHost enum — identifies paper prose/adversarial mode by host.
//! Defined locally in research-harness to avoid crate dependency cycle with
//! runtime-core-contracts → host-projection → research-harness.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperProseHookHost {
    Cursor,
    Codex,
    Claude,
    OpenCode,
}

impl PaperProseHookHost {
    pub fn env_var(self) -> &'static str {
        match self {
            Self::Cursor => "ROUTER_RS_CURSOR_PAPER_PROSE_HOOK",
            Self::Codex => "ROUTER_RS_CODEX_PAPER_PROSE_HOOK",
            Self::Claude => "ROUTER_RS_CLAUDE_PAPER_PROSE_HOOK",
            Self::OpenCode => "ROUTER_RS_OPENCODE_PAPER_PROSE_HOOK",
        }
    }

    pub fn adversarial_env_var(self) -> &'static str {
        match self {
            Self::Cursor => "ROUTER_RS_CURSOR_PAPER_ADVERSARIAL_HOOK",
            Self::Codex => "ROUTER_RS_CODEX_PAPER_ADVERSARIAL_HOOK",
            Self::Claude => "ROUTER_RS_CLAUDE_PAPER_ADVERSARIAL_HOOK",
            Self::OpenCode => "ROUTER_RS_OPENCODE_PAPER_ADVERSARIAL_HOOK",
        }
    }

    pub fn from_host_lifecycle_state_dir(_state_dir_leaf: &str) -> Self {
        Self::Codex
    }

    pub fn from_host_id(host_id: &str) -> Option<Self> {
        match host_id {
            "cursor" => Some(Self::Cursor),
            "codex" => Some(Self::Codex),
            "claude" => Some(Self::Claude),
            "opencode" => Some(Self::OpenCode),
            _ => None,
        }
    }
}
