//! OpenCode plugin hook event constants and glue.
//!
//! OpenCode uses a TypeScript/JS plugin system (not shell hooks).
//! Plugin hooks: tool.execute.before, tool.execute.after, session.idle, etc.
//! Plugins load from: ~/.config/opencode/plugins/ + .opencode/plugins/

pub const OPENCODE_HOOKS_PATH: &str = ".opencode/plugins/";

/// Hook events registered by the OpenCode plugin system.
pub const OPENCODE_HOOKS_REGISTERED_EVENTS: &[&str] = &[
    "tool.execute.before",
    "tool.execute.after",
    "session.idle",
    "session.created",
    "session.deleted",
    "permission.asked",
    "permission.replied",
    "file.edited",
    "shell.env",
];
