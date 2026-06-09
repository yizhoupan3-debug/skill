//! MCP agent loop for host integrations (claude, antigravity, opencode, etc.).
//!
//! MCP 服务器（stdio transport），提供 tools / prompts / resources 三类端点，
//! 替代 Claude Code CLI 的 shell hook 协议（PreToolUse / UserPromptSubmit / PostToolUse / Stop）。
//!
//! 架构约束：MCP 不支持工具拦截，因此 PreToolUse guards（framework/settings path guard、
//! dangerous bash guard）在 Desktop 上不可用，依赖 CLAUDE.md 指令自律。
//! Stop / UserPromptSubmit 无 shell 硬拦；`closeout_gate` / `goal_state_manage complete` 在 MCP 工具层为 advisory（不阻断执行）。
//!
//! 与 CLI 共享 L2/L3 手动画板（evidence、goal state、路由、snapshot），出站为 MCP JSON-RPC。

#[macro_use]
mod cache;
mod dispatch;
mod host;
mod prompts_resources;
mod tools;
pub mod tools_web;
mod transport;

#[cfg(test)]
mod tests;

pub use transport::*;
pub use dispatch::handle_mcp_request;

#[cfg(test)]
pub use dispatch::handle_initialize;
#[cfg(test)]
pub use prompts_resources::{handle_prompts_list, handle_prompts_get, handle_resources_list, handle_resources_read};
#[cfg(test)]
pub use tools::{
    build_evidence_entry, handle_tools_call, handle_tools_list, tool_closeout, tool_closeout_gate,
    tool_framework_snapshot, tool_goal_state, tool_record_evidence, tool_rfv_loop,
    tool_session_checkpoint, tool_skill_route, tool_web_fetch,
};
