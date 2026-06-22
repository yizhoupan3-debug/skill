//! PaperProseHookHost enum — identifies paper prose/adversarial mode by host.
//! Moved here from host-projection to resolve crate dependency ordering (Phase 4 F1).

/// Identifies which paper prose/adversarial variant a host uses.
///
/// SYNC REQUIREMENT: must have the same variants as `HookObservationHost` in
/// `host_projection::hooks::HookObservationHost`. See `HookObservationHost` doc
/// comment for the full sync protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperProseHookHost {
    Cursor,
    Codex,
    Claude,
    OpenCode,
}

impl PaperProseHookHost {
    /// Per-host env var controlling prose hook injection.
    pub fn env_var(self) -> &'static str {
        match self {
            Self::Cursor => "ROUTER_RS_CURSOR_PAPER_PROSE_HOOK",
            Self::Codex => "ROUTER_RS_CODEX_PAPER_PROSE_HOOK",
            Self::Claude => "ROUTER_RS_CLAUDE_PAPER_PROSE_HOOK",
            Self::OpenCode => "ROUTER_RS_OPENCODE_PAPER_PROSE_HOOK",
        }
    }

    /// Per-host env var controlling adversarial review hook injection.
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
