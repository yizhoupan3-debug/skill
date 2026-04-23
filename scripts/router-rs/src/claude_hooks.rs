use chrono::Local;
use regex::Regex;
use rusqlite::{params, types::ValueRef, Connection};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::framework_runtime::{
    build_framework_recap_projection, build_framework_runtime_snapshot_envelope,
};

pub(crate) const CLAUDE_HOOK_SCHEMA_VERSION: &str = "router-rs-claude-hook-response-v1";
const CLAUDE_HOOK_AUDIT_SCHEMA_VERSION: &str = "router-rs-claude-hook-audit-response-v1";
const CLAUDE_HOOK_PROJECTION_SCHEMA_VERSION: &str = "router-rs-claude-hook-projection-v1";
pub(crate) const CLAUDE_HOOK_AUTHORITY: &str = "rust-claude-hook";
const CLAUDE_HOOK_AUDIT_AUTHORITY: &str = "rust-claude-hook-audit";
const MEMORY_STATE_SCHEMA_VERSION: &str = "memory-state-v1";
const MEMORY_STORE_SCHEMA_VERSION: &str = "1";
const CLAUDE_MEMORY_PATH: &str = ".codex/memory/CLAUDE_MEMORY.md";
const MEMORY_AUTO_FILENAME: &str = "MEMORY_AUTO.md";
const MEMORY_DB_FILENAME: &str = "memory.sqlite3";
const MEMORY_STATE_FILENAME: &str = "state.json";
const SQLITE_DUMP_FILENAME: &str = "sqlite_legacy_dump.json";
const STABLE_DOCUMENTS: [&str; 5] = [
    "MEMORY.md",
    "preferences.md",
    "decisions.md",
    "lessons.md",
    "runbooks.md",
];
const GENERATED_PATHS: [&str; 3] = [
    ".claude/settings.json",
    ".claude/hooks/README.md",
    ".claude/hooks/run.sh",
];
const CLAUDE_PRE_TOOL_USE_RULES: [&str; 13] = [
    "/AGENT.md",
    "/AGENTS.md",
    "/CLAUDE.md",
    "/GEMINI.md",
    "/.gemini/settings.json",
    "/.claude/settings.json",
    "/.claude/agents/README.md",
    "/.claude/hooks/README.md",
    "/.claude/hooks/*.sh",
    "/.claude/commands/**",
    "/.codex/hooks.json",
    "/.codex/host_entrypoints_sync_manifest.json",
    "/.codex/memory/CLAUDE_MEMORY.md",
];
const CLAUDE_PRE_TOOL_USE_BASH_RULES: [&str; 12] = [
    "*AGENT.md*",
    "*AGENTS.md*",
    "*CLAUDE.md*",
    "*GEMINI.md*",
    "*.gemini/settings.json*",
    "*.claude/settings.json*",
    "*.claude/agents/README.md*",
    "*.claude/hooks/*",
    "*.claude/commands/*",
    "*.codex/hooks.json*",
    "*.codex/host_entrypoints_sync_manifest.json*",
    "*.codex/memory/CLAUDE_MEMORY.md*",
];
const CLAUDE_QUALITY_PRE_TOOL_USE_RULES: [&str; 5] = [
    "/framework_runtime/src/**",
    "/scripts/router-rs/src/**",
    "/scripts/materialize_cli_host_entrypoints.py",
    "/tests/**",
    "/.claude/hooks/**",
];
const CLAUDE_STOP_FAILURE_MATCHER: &str =
    "invalid_request|server_error|max_output_tokens|rate_limit|authentication_failed|billing_error|unknown";
const PROTECTED_GENERATED_PATHS: [&str; 10] = [
    "AGENT.md",
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
    ".gemini/settings.json",
    ".claude/settings.json",
    ".claude/agents/README.md",
    ".codex/hooks.json",
    ".codex/host_entrypoints_sync_manifest.json",
    ".codex/memory/CLAUDE_MEMORY.md",
];
const PROTECTED_GENERATED_PREFIXES: [&str; 2] = [".claude/hooks/", ".claude/commands/"];
const PROTECTED_BASH_PATH_HINTS: [&str; 12] = [
    "AGENT.md",
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
    ".gemini/settings.json",
    ".claude/settings.json",
    ".claude/agents/README.md",
    ".codex/hooks.json",
    ".claude/hooks/",
    ".claude/commands/",
    ".codex/host_entrypoints_sync_manifest.json",
    ".codex/memory/CLAUDE_MEMORY.md",
];
const USER_PROMPT_STRONG_ACTION_TERMS: [&str; 21] = [
    "修复",
    "实现",
    "优化",
    "重构",
    "加速",
    "删掉",
    "去掉",
    "移除",
    "清理",
    "替换",
    "落实",
    "补齐",
    "增强",
    "改代码",
    "fix",
    "implement",
    "optimize",
    "refactor",
    "speed up",
    "remove",
    "rewrite",
];
const USER_PROMPT_WEAK_ACTION_TERMS: [&str; 8] =
    ["改", "写", "调", "修", "做", "update", "change", "improve"];
const USER_PROMPT_CODE_TARGET_TERMS: [&str; 22] = [
    "代码",
    "runtime",
    "hook",
    "hooks",
    "agent.md",
    "agents.md",
    "claude.md",
    "gemini.md",
    "脚本",
    "router",
    "路由",
    "内存",
    "性能",
    "热区",
    "热路径",
    "保底",
    "补丁",
    "fallback",
    "shim",
    "wrapper",
    "patch",
    "兼容层",
];
const USER_PROMPT_NON_CODE_EDIT_TERMS: [&str; 11] = [
    "结论",
    "措辞",
    "翻译",
    "润色",
    "摘要",
    "邮件",
    "口语化",
    "人话",
    "readme",
    "文档",
    "说明",
];
const USER_PROMPT_PERF_TERMS: [&str; 8] = [
    "加速",
    "性能",
    "内存",
    "热区",
    "热路径",
    "performance",
    "memory",
    "latency",
];
const USER_PROMPT_COMPAT_TERMS: [&str; 7] = [
    "保底", "补丁", "兼容", "fallback", "shim", "wrapper", "patch",
];
const USER_PROMPT_HOOK_TERMS: [&str; 6] = [
    "hook",
    "hooks",
    "agent.md",
    "claude.md",
    "pretooluse",
    "userpromptsubmit",
];
const USER_PROMPT_MEMORY_PRIORITY_CONTEXT: &str =
    "记忆真源：这个仓库优先使用 repo-local shared memory `./.codex/memory/`，不要把 Codex global memories 当成当前项目真相。";
const USER_PROMPT_CONTINUITY_CONTEXT: &str =
    "任务真源：`artifacts/current/<task_id>/` + `active_task.json` + `.supervisor_state.json`。";
const USER_PROMPT_PERF_CONTEXT: &str =
    "顺手看热路径上的重复 I/O、重复序列化、无谓 clone、临时对象和多余包装层。";
const USER_PROMPT_COMPAT_CONTEXT: &str =
    "如果旧 compat/fallback/过渡逻辑已经没有真实必要，优先删掉而不是继续包一层。";
const USER_PROMPT_HOOK_CONTEXT: &str =
    "Hook 额外检查：让 hook 增加自动化，而不是只做阻拦；优先短上下文、窄触发、低开销，并尽量用 matcher/if 避免无谓触发。";
const FALLBACK_SHARED_PROJECT_MCP_SERVERS: [&str; 3] =
    ["browser-mcp", "framework-mcp", "openaiDeveloperDocs"];
const USER_PROMPT_EXECUTION_INTENT_PREFIX: &str = "执行意图：";
const USER_PROMPT_STATE_COMPACT_PREFIX: &str = "当前状态：";
const USER_PROMPT_STATE_BUDGET_CHARS: usize = 120;
const USER_PROMPT_COMPLEX_STATE_BUDGET_CHARS: usize = 220;
const USER_PROMPT_CONTEXT_MAX_CHARS: usize = 420;
const USER_PROMPT_COMPLEX_CONTEXT_MAX_CHARS: usize = 1100;
const QUALITY_RUST_CONTEXT: &str =
    "Rust 额外检查：盯住热循环里的分配、clone、String/Vec 复制和 serde_json 往返。";
const QUALITY_PYTHON_CONTEXT: &str =
    "Python 额外检查：盯住重复解析、重复读文件、wrapper-on-wrapper 和兼容别名链。";
const QUALITY_TEST_CONTEXT: &str = "测试额外检查：锁真实契约和回归点，不给补丁式旧行为续命。";
const QUALITY_RUNTIME_PREFIXES: [&str; 2] = [
    "framework_runtime/src/framework_runtime/",
    "scripts/router-rs/src/",
];
const QUALITY_HOOK_PREFIXES: [&str; 1] = [".claude/hooks/"];
const QUALITY_TARGET_SUFFIXES: [&str; 3] = [".py", ".rs", ".sh"];
const PATCH_ARTIFACT_SUFFIXES: [&str; 4] = [".patch", ".diff", ".rej", ".orig"];
const ASYNC_AUDIT_PREFIXES: [&str; 5] = [
    "framework_runtime/src/framework_runtime/",
    "scripts/router-rs/src/",
    "scripts/materialize_cli_host_entrypoints.py",
    "tests/",
    ".claude/hooks/",
];
const CLAUDE_HOOK_SNAPSHOT_ROOT_DIRNAME: &str = "claude_hook_audit_snapshots";
const SNAPSHOT_MAX_BYTES: u64 = 200_000;
const COMPAT_SMELL_PATTERN: &str = r"\b(?:compat|compatibility|legacy|fallback|shim|patch|workaround|temporary|deprecated)\b|兼容|保底|补丁";
const SHARED_CONTINUITY_PATHS: [&str; 5] = [
    "SESSION_SUMMARY.md",
    "NEXT_ACTIONS.json",
    "EVIDENCE_INDEX.json",
    "TRACE_METADATA.json",
    ".supervisor_state.json",
];
const TERMINAL_STORY_STATES: [&str; 6] = [
    "completed",
    "finalized",
    "closed",
    "cancelled",
    "abandoned",
    "failed",
];
const TERMINAL_PHASES: [&str; 7] = [
    "completed",
    "finalized",
    "closed",
    "cancelled",
    "abandoned",
    "failed",
    "done",
];
const TERMINAL_VERIFICATION_STATUSES: [&str; 6] = [
    "completed",
    "passed",
    "verified",
    "cancelled",
    "abandoned",
    "failed",
];

const CLAUDE_SETTINGS_SCHEMA_URL: &str = "https://json.schemastore.org/claude-code-settings.json";
const CLAUDE_PROJECT_ALLOW_PERMISSIONS: [&str; 25] = [
    "Bash(ls)",
    "Bash(pwd)",
    "Bash(rg *)",
    "Bash(cat *)",
    "Bash(sed -n *)",
    "Bash(git status)",
    "Bash(git diff)",
    "Bash(git show *)",
    "Bash(git rev-parse *)",
    "Bash(git ls-files *)",
    "Bash(python3 scripts/check_skills.py --verify-sync)",
    "Bash(python3 scripts/materialize_cli_host_entrypoints.py)",
    "Bash(python3 -m pytest *)",
    "Bash(python3 -m compileall *)",
    "Bash(cargo test *)",
    "Bash(cargo run --manifest-path ./scripts/router-rs/Cargo.toml --release -- *)",
    "Bash(./scripts/router-rs/target/release/router-rs *)",
    "Bash(./scripts/router-rs/target/debug/router-rs *)",
    "Bash(*scripts/router-rs/target/release/router-rs *)",
    "Bash(*scripts/router-rs/target/debug/router-rs *)",
    "Bash(cargo run --manifest-path *scripts/router-rs/Cargo.toml --release -- *)",
    "Bash(python3 scripts/runtime_background_cli.py *)",
    "Bash(cmp -s TRACE_METADATA.json artifacts/current/TRACE_METADATA.json)",
    "Bash(./tools/browser-mcp/scripts/start_browser_mcp.sh *)",
    "Bash(bash ./tools/browser-mcp/scripts/start_browser_mcp.sh *)",
];

pub fn build_claude_hook_manifest() -> Value {
    let pre_tool_command = "sh \"$CLAUDE_PROJECT_DIR\"/.claude/hooks/run.sh pre-tool-use";
    let quality_command = "sh \"$CLAUDE_PROJECT_DIR\"/.claude/hooks/run.sh pre-tool-use-quality";
    let post_tool_command = "sh \"$CLAUDE_PROJECT_DIR\"/.claude/hooks/run.sh post-tool-audit";

    let pre_tool_hooks = build_tool_path_hooks(&CLAUDE_PRE_TOOL_USE_RULES, pre_tool_command, None);
    let pre_tool_bash_hooks = CLAUDE_PRE_TOOL_USE_BASH_RULES
        .iter()
        .map(|rule| {
            json!({
                "type": "command",
                "if": format!("Bash({rule})"),
                "command": pre_tool_command,
            })
        })
        .collect::<Vec<_>>();
    let quality_pre_tool_hooks =
        build_tool_path_hooks(&CLAUDE_QUALITY_PRE_TOOL_USE_RULES, quality_command, None);
    let post_tool_hooks = build_tool_path_hooks(
        &CLAUDE_QUALITY_PRE_TOOL_USE_RULES,
        post_tool_command,
        Some(json!({"async": true, "timeout": 8})),
    );

    json!({
        "schema_version": "router-rs-claude-hook-manifest-v1",
        "authority": CLAUDE_HOOK_AUTHORITY,
        "protected_paths": {
            "edit_write": CLAUDE_PRE_TOOL_USE_RULES,
            "bash": CLAUDE_PRE_TOOL_USE_BASH_RULES,
            "quality": CLAUDE_QUALITY_PRE_TOOL_USE_RULES,
            "generated_surfaces": GENERATED_PATHS,
        },
        "settings_hooks": {
            "PreToolUse": [
                {
                    "matcher": "Edit|MultiEdit|Write",
                    "hooks": pre_tool_hooks,
                },
                {
                    "matcher": "Bash",
                    "hooks": pre_tool_bash_hooks,
                },
                {
                    "matcher": "Edit|MultiEdit|Write",
                    "hooks": quality_pre_tool_hooks,
                }
            ],
            "PostToolUse": [
                {
                    "matcher": "Edit|MultiEdit|Write",
                    "hooks": post_tool_hooks,
                }
            ],
            "UserPromptSubmit": [
                {
                    "hooks": [
                        {
                            "type": "command",
                            "command": "sh \"$CLAUDE_PROJECT_DIR\"/.claude/hooks/run.sh user-prompt-submit",
                        }
                    ]
                }
            ],
            "SessionEnd": [
                {
                    "hooks": [
                        {
                            "type": "command",
                            "command": "sh \"$CLAUDE_PROJECT_DIR\"/.claude/hooks/run.sh session-end",
                        }
                    ]
                }
            ],
            "ConfigChange": [
                {
                    "matcher": "project_settings",
                    "hooks": [
                        {
                            "type": "command",
                            "command": "sh \"$CLAUDE_PROJECT_DIR\"/.claude/hooks/run.sh config-change",
                        }
                    ]
                }
            ],
            "StopFailure": [
                {
                    "matcher": CLAUDE_STOP_FAILURE_MATCHER,
                    "hooks": [
                        {
                            "type": "command",
                            "command": "sh \"$CLAUDE_PROJECT_DIR\"/.claude/hooks/run.sh stop-failure",
                        }
                    ]
                }
            ]
        }
    })
}

pub fn build_claude_project_settings(repo_root: &Path) -> Value {
    let allowed_mcp_servers = load_runtime_registry_shared_project_mcp_servers(repo_root)
        .into_iter()
        .map(|server_name| json!({"serverName": server_name}))
        .collect::<Vec<_>>();
    json!({
        "$schema": CLAUDE_SETTINGS_SCHEMA_URL,
        "permissions": {
            "allow": CLAUDE_PROJECT_ALLOW_PERMISSIONS,
        },
        "allowedMcpServers": allowed_mcp_servers,
        "hooks": build_claude_hook_manifest()["settings_hooks"].clone(),
    })
}

pub fn build_codex_hook_manifest() -> Value {
    json!({
        "hooks": {
            "PreToolUse": [
                build_codex_command_hook("pre-tool-use", "Edit"),
                build_codex_command_hook("pre-tool-use", "MultiEdit"),
                build_codex_command_hook("pre-tool-use", "Write"),
                build_codex_command_hook("pre-tool-use", "Bash"),
            ],
            "PermissionRequest": [
                build_codex_command_hook("permission-request", "Bash"),
            ],
        }
    })
}

pub fn build_claude_hook_projection() -> Value {
    json!({
        "schema_version": CLAUDE_HOOK_PROJECTION_SCHEMA_VERSION,
        "authority": CLAUDE_HOOK_AUTHORITY,
        "agent_policy": build_agent_policy(),
        "root_agents_proxy": build_root_agents_proxy(),
        "root_claude_proxy": build_root_claude_proxy(),
        "root_gemini_proxy": build_root_gemini_proxy(),
        "hooks_readme": build_claude_hooks_readme(),
        "hook_runner": build_claude_hook_runner(),
        "codex_hooks": build_codex_hook_manifest(),
    })
}

fn build_agent_policy() -> String {
    include_str!("../../../AGENT.md").to_string()
}

fn build_root_agents_proxy() -> String {
    include_str!("../../../AGENTS.md").to_string()
}

fn build_root_claude_proxy() -> String {
    include_str!("../../../CLAUDE.md").to_string()
}

fn build_root_gemini_proxy() -> String {
    include_str!("../../../GEMINI.md").to_string()
}

fn build_claude_hooks_readme() -> String {
    include_str!("../../../.claude/hooks/README.md").to_string()
}

fn build_claude_hook_runner() -> String {
    include_str!("../../../.claude/hooks/run.sh").to_string()
}

fn build_codex_command_hook(event: &str, matcher: &str) -> Value {
    json!({
        "matcher": matcher,
        "hooks": [
            {
                "type": "command",
                "command": build_codex_hook_bridge_command(event),
                "timeout": 8,
            }
        ]
    })
}

fn build_codex_hook_bridge_command(event: &str) -> String {
    let mut command = String::new();
    command
        .push_str("CODEX_PROJECT_ROOT=\"$(git rev-parse --show-toplevel 2>/dev/null || pwd)\"; ");
    command.push_str(
        "ROUTER_RS_RELEASE_BIN=\"$CODEX_PROJECT_ROOT/scripts/router-rs/target/release/router-rs\"; ",
    );
    command.push_str(
        "ROUTER_RS_DEBUG_BIN=\"$CODEX_PROJECT_ROOT/scripts/router-rs/target/debug/router-rs\"; ",
    );
    command.push_str("ROUTER_RS_CRATE_ROOT=\"$CODEX_PROJECT_ROOT/scripts/router-rs\"; ");
    command.push_str("router_rs_is_fresh() { ");
    command.push_str("bin_path=\"$1\"; ");
    command.push_str("[ -x \"$bin_path\" ] || return 1; ");
    command.push_str("[ \"$ROUTER_RS_CRATE_ROOT/Cargo.toml\" -nt \"$bin_path\" ] && return 1; ");
    command.push_str(
        "find \"$ROUTER_RS_CRATE_ROOT/src\" -type f -newer \"$bin_path\" | grep -q . && return 1; ",
    );
    command.push_str("return 0; ");
    command.push_str("}; ");
    command.push_str("run_router_rs() { ");
    command.push_str(
        "if router_rs_is_fresh \"$ROUTER_RS_RELEASE_BIN\"; then \"$ROUTER_RS_RELEASE_BIN\" \"$@\"; return; fi; ",
    );
    command.push_str(
        "if router_rs_is_fresh \"$ROUTER_RS_DEBUG_BIN\"; then \"$ROUTER_RS_DEBUG_BIN\" \"$@\"; return; fi; ",
    );
    command.push_str(
        "if [ -x \"$ROUTER_RS_RELEASE_BIN\" ]; then \"$ROUTER_RS_RELEASE_BIN\" \"$@\"; return; fi; ",
    );
    command.push_str(
        "if [ -x \"$ROUTER_RS_DEBUG_BIN\" ]; then \"$ROUTER_RS_DEBUG_BIN\" \"$@\"; return; fi; ",
    );
    command.push_str(
        "echo \"Missing required router-rs binary: $ROUTER_RS_RELEASE_BIN or $ROUTER_RS_DEBUG_BIN\" >&2; ",
    );
    command.push_str("exit 1; ");
    command.push_str("}; ");
    command.push_str(&format!(
        "run_router_rs --codex-hook-command {event} --repo-root \"$CODEX_PROJECT_ROOT\""
    ));
    command
}

fn load_runtime_registry_shared_project_mcp_servers_from_path(registry_path: &Path) -> Vec<String> {
    let Ok(raw) = fs::read_to_string(registry_path) else {
        return Vec::new();
    };
    let Ok(payload) = serde_json::from_str::<Value>(&raw) else {
        return Vec::new();
    };
    payload
        .get("shared_project_mcp_servers")
        .and_then(Value::as_array)
        .map(|servers| {
            servers
                .iter()
                .filter_map(Value::as_str)
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn framework_runtime_registry_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("configs")
        .join("framework")
        .join("RUNTIME_REGISTRY.json")
}

fn load_runtime_registry_shared_project_mcp_servers(repo_root: &Path) -> Vec<String> {
    let registry_path = repo_root
        .join("configs")
        .join("framework")
        .join("RUNTIME_REGISTRY.json");
    let servers = load_runtime_registry_shared_project_mcp_servers_from_path(&registry_path);
    if !servers.is_empty() {
        return servers;
    }
    let fallback_servers = load_runtime_registry_shared_project_mcp_servers_from_path(
        &framework_runtime_registry_path(),
    );
    if !fallback_servers.is_empty() {
        return fallback_servers;
    }
    FALLBACK_SHARED_PROJECT_MCP_SERVERS
        .iter()
        .map(|server| (*server).to_string())
        .collect()
}

fn build_tool_path_hooks(rules: &[&str], command: &str, extras: Option<Value>) -> Vec<Value> {
    let extra_object = extras.and_then(|value| value.as_object().cloned());
    let mut hooks = Vec::new();
    for rule in rules {
        for tool_name in ["Edit", "MultiEdit", "Write"] {
            let mut hook = Map::new();
            hook.insert("type".to_string(), Value::String("command".to_string()));
            hook.insert(
                "if".to_string(),
                Value::String(format!("{tool_name}({rule})")),
            );
            hook.insert("command".to_string(), Value::String(command.to_string()));
            if let Some(extras) = &extra_object {
                for (key, value) in extras {
                    hook.insert(key.clone(), value.clone());
                }
            }
            hooks.push(Value::Object(hook));
        }
    }
    hooks
}

pub fn run_claude_lifecycle_hook(
    command: &str,
    repo_root: &Path,
    max_lines: usize,
) -> Result<Value, String> {
    let canonical = canonical_lifecycle_command(command)?;
    let contract = lifecycle_contract(canonical);
    let mut response = json!({
        "schema_version": CLAUDE_HOOK_SCHEMA_VERSION,
        "authority": CLAUDE_HOOK_AUTHORITY,
        "wrapper_command": command,
        "canonical_command": canonical,
        "command": canonical,
        "repo_root": repo_root.display().to_string(),
        "contract": contract,
    });

    if contract
        .get("consolidates_shared_memory")
        .and_then(Value::as_bool)
        == Some(true)
    {
        response["consolidation"] = consolidate_shared_memory(repo_root)?;
    }

    response["projection"] = sync_claude_memory_projection(repo_root, max_lines)?;
    Ok(response)
}

pub fn run_claude_audit_hook(command: &str, repo_root: &Path) -> Result<Value, String> {
    let canonical = canonical_audit_command(command)?;
    let payload = read_stdin_payload()?;
    match canonical {
        "user-prompt-submit" => run_user_prompt_submit(repo_root, &payload),
        "pre-tool-use" => run_pre_tool_use(repo_root, &payload),
        "pre-tool-use-quality" => run_pre_tool_use_quality(repo_root, &payload),
        "post-tool-audit" => run_post_tool_audit(repo_root, &payload),
        "config-change" => run_config_change(repo_root, &payload),
        "stop-failure" => run_stop_failure(repo_root, &payload),
        _ => Err(format!("Unsupported Claude audit command: {command}")),
    }
}

pub fn run_codex_audit_hook(command: &str, repo_root: &Path) -> Result<Option<Value>, String> {
    let canonical = canonical_codex_audit_command(command)?;
    let payload = read_stdin_payload()?;
    match canonical {
        "pre-tool-use" => run_codex_pre_tool_use(repo_root, &payload),
        "permission-request" => {
            bridge_codex_permission_request(run_pre_tool_use(repo_root, &payload)?)
        }
        // Older project-local Codex hook installs may still invoke this event.
        // The current Codex runtime surfaces `systemMessage` visibly and does
        // not yet honor `suppressOutput`, so keep the compatibility path silent.
        "user-prompt-submit" => Ok(None),
        _ => Err(format!("Unsupported Codex audit command: {command}")),
    }
}

fn run_codex_pre_tool_use(repo_root: &Path, payload: &Value) -> Result<Option<Value>, String> {
    let base = run_pre_tool_use(repo_root, payload)?;
    if base.get("decision").and_then(Value::as_str) == Some("deny") {
        return bridge_codex_pre_tool_use(base);
    }
    if let Some(block) = codex_pre_tool_use_quality_block(repo_root, payload)? {
        return Ok(Some(block));
    }
    Ok(None)
}

fn canonical_lifecycle_command(command: &str) -> Result<&'static str, String> {
    match command {
        "refresh-projection" => Ok("refresh-projection"),
        "session-start" => Ok("session-start"),
        "session-stop" => Ok("session-stop"),
        "pre-compact" => Ok("pre-compact"),
        "subagent-stop" => Ok("subagent-stop"),
        "session-end" => Ok("session-end"),
        _ => Err(format!("Unsupported Claude lifecycle command: {command}")),
    }
}

fn canonical_audit_command(command: &str) -> Result<&'static str, String> {
    match command {
        "user-prompt-submit" => Ok("user-prompt-submit"),
        "pre-tool-use" => Ok("pre-tool-use"),
        "pre-tool-use-quality" => Ok("pre-tool-use-quality"),
        "post-tool-audit" => Ok("post-tool-audit"),
        "config-change" => Ok("config-change"),
        "stop-failure" => Ok("stop-failure"),
        _ => Err(format!("Unsupported Claude audit command: {command}")),
    }
}

fn canonical_codex_audit_command(command: &str) -> Result<&'static str, String> {
    match command {
        "pre-tool-use" => Ok("pre-tool-use"),
        "permission-request" => Ok("permission-request"),
        "user-prompt-submit" => Ok("user-prompt-submit"),
        _ => Err(format!("Unsupported Codex audit command: {command}")),
    }
}

fn bridge_codex_pre_tool_use(payload: Value) -> Result<Option<Value>, String> {
    if payload.get("decision").and_then(Value::as_str) != Some("deny") {
        return Ok(None);
    }
    let reason = codex_pre_tool_use_reason(
        payload
            .get("hookSpecificOutput")
            .and_then(Value::as_object)
            .and_then(|hook| hook.get("permissionDecisionReason"))
            .and_then(Value::as_str)
            .or_else(|| payload.get("message").and_then(Value::as_str))
            .unwrap_or("Request blocked by repo-local policy."),
    );
    Ok(Some(json!({
        "decision": "block",
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        },
    })))
}

fn bridge_codex_permission_request(payload: Value) -> Result<Option<Value>, String> {
    if payload.get("decision").and_then(Value::as_str) != Some("deny") {
        return Ok(None);
    }
    let reason = codex_pre_tool_use_reason(
        payload
            .get("hookSpecificOutput")
            .and_then(Value::as_object)
            .and_then(|hook| hook.get("permissionDecisionReason"))
            .and_then(Value::as_str)
            .or_else(|| payload.get("message").and_then(Value::as_str))
            .unwrap_or("Request blocked by repo-local policy."),
    );
    Ok(Some(json!({
        "hookSpecificOutput": {
            "hookEventName": "PermissionRequest",
            "decision": {
                "behavior": "deny",
                "message": reason,
            },
        },
    })))
}

fn codex_pre_tool_use_reason(reason: &str) -> String {
    reason.replace("[claude-pre-tool-use]", "[codex-pre-tool-use]")
}

fn codex_block_payload(reason: String) -> Value {
    json!({
        "decision": "block",
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": codex_pre_tool_use_reason(&reason),
        },
    })
}

fn is_patch_artifact_path(path: &str) -> bool {
    let lowered = path.to_lowercase();
    PATCH_ARTIFACT_SUFFIXES
        .iter()
        .any(|suffix| lowered.ends_with(suffix))
}

fn codex_patch_artifact_message(path: &str) -> String {
    format!(
        "[claude-pre-tool-use] blocked patch artifact write {path}; implement the change in tracked source files instead of emitting diff/patch byproducts."
    )
}

fn codex_pre_tool_use_quality_block(
    repo_root: &Path,
    payload: &Value,
) -> Result<Option<Value>, String> {
    let tool_name = payload
        .get("tool_name")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if !matches!(tool_name, "Edit" | "MultiEdit" | "Write") {
        return Ok(None);
    }

    let mut rel_paths = iter_payload_paths(payload)
        .into_iter()
        .map(|path| relative_candidate_path(&path, repo_root))
        .collect::<Vec<_>>();
    rel_paths.sort();
    rel_paths.dedup();

    for path in &rel_paths {
        if is_patch_artifact_path(path) {
            return Ok(Some(codex_block_payload(codex_patch_artifact_message(
                path,
            ))));
        }
    }

    for path in rel_paths {
        if quality_target_context(&path).is_none() {
            continue;
        }
        let (delta_text, source_mode) = extract_audit_delta(repo_root, &path, payload)?;
        if delta_text.trim().is_empty() {
            continue;
        }
        if let Some(reason) = codex_quality_block_reason(&path, &delta_text, &source_mode) {
            return Ok(Some(codex_block_payload(reason)));
        }
    }

    Ok(None)
}

fn codex_quality_block_reason(path: &str, text: &str, source_mode: &str) -> Option<String> {
    let compat_hits = compat_smell_count(text);
    let lowered_path = path.to_lowercase();
    let source_label = format!("增量来源={source_mode}");

    if path.ends_with(".rs") {
        let clone_hits = text.matches(".clone(").count() + text.matches(".clone()").count();
        let serde_hits = text.matches("serde_json::").count();
        let string_hits =
            text.matches(".to_string()").count() + text.matches(".to_owned()").count();
        if compat_hits >= 1 || clone_hits >= 3 || serde_hits >= 3 || string_hits >= 4 {
            return Some(format!(
                "[claude-pre-tool-use] blocked patchy Rust edit in {path} ({source_label}, compat={compat_hits}, clone={clone_hits}, serde={serde_hits}, string_copy={string_hits}); fold the fix into the real path instead of adding fallback/shim glue or extra hot-path copies."
            ));
        }
    } else if path.ends_with(".py") {
        let json_hits = text.matches("json.loads(").count() + text.matches("json.dumps(").count();
        let io_hits = text.matches(".read_text(").count()
            + text.matches(".read_bytes(").count()
            + text.matches(".write_text(").count();
        let wrapper_hits = text.matches("def ").count();
        if compat_hits >= 1
            || json_hits >= 3
            || io_hits >= 3
            || (lowered_path.contains("hook") && wrapper_hits >= 3)
        {
            return Some(format!(
                "[claude-pre-tool-use] blocked patchy Python edit in {path} ({source_label}, compat={compat_hits}, json_roundtrip={json_hits}, file_io={io_hits}, helper_defs={wrapper_hits}); collapse the change into the main path instead of stacking wrappers, patch branches, or repeated parse/write loops."
            ));
        }
    } else if path.ends_with(".sh") {
        let deny_hits = text.matches("permissionDecision").count();
        if compat_hits >= 1 || (lowered_path.contains("hook") && deny_hits >= 1) {
            return Some(format!(
                "[claude-pre-tool-use] blocked patchy hook shell edit in {path} ({source_label}, compat={compat_hits}, deny_rules={deny_hits}); keep the hook short and Rust-owned instead of growing another shell-side guard layer."
            ));
        }
    }

    None
}

fn lifecycle_contract(command: &str) -> Value {
    match command {
        "refresh-projection" => json!({
            "writes": ["project-local Claude memory projection"],
            "forbidden_writes": SHARED_CONTINUITY_PATHS,
            "consolidates_shared_memory": false,
            "summary": "Refresh the imported Claude projection without touching shared continuity artifacts."
        }),
        "session-start" => json!({
            "writes": ["project-local Claude memory projection"],
            "forbidden_writes": SHARED_CONTINUITY_PATHS,
            "consolidates_shared_memory": false,
            "summary": "Refresh the imported Claude projection at session start."
        }),
        "session-stop" => json!({
            "writes": ["project-local Claude memory projection"],
            "forbidden_writes": SHARED_CONTINUITY_PATHS,
            "consolidates_shared_memory": false,
            "summary": "Perform a lightweight post-turn projection refresh only."
        }),
        "pre-compact" => json!({
            "writes": ["project-local Claude memory projection"],
            "forbidden_writes": SHARED_CONTINUITY_PATHS,
            "consolidates_shared_memory": false,
            "summary": "Refresh the projection before compaction without running consolidation."
        }),
        "subagent-stop" => json!({
            "writes": ["project-local Claude memory projection"],
            "forbidden_writes": SHARED_CONTINUITY_PATHS,
            "consolidates_shared_memory": false,
            "summary": "Refresh the projection after subagent completion without taking over subagent orchestration."
        }),
        "session-end" => json!({
            "writes": [
                "project-local shared memory bundle",
                "project-local Claude memory projection",
                "terminal-session continuity repair when resume_allowed is stale"
            ],
            "forbidden_writes": [
                "SESSION_SUMMARY.md",
                "NEXT_ACTIONS.json",
                "EVIDENCE_INDEX.json",
                "TRACE_METADATA.json"
            ],
            "conditional_writes": [".supervisor_state.json"],
            "consolidates_shared_memory": true,
            "summary": "Consolidate the project-local memory bundle, refresh the imported Claude projection, and only repair terminal resume state when needed."
        }),
        _ => Value::Null,
    }
}

fn sync_claude_memory_projection(repo_root: &Path, max_lines: usize) -> Result<Value, String> {
    let target = repo_root.join(CLAUDE_MEMORY_PATH);
    let content = build_claude_memory_projection(repo_root, max_lines)?;
    let changed = write_text_if_changed(&target, &content)?;
    Ok(json!({
        "status": if changed { "updated" } else { "unchanged" },
        "target_path": target.display().to_string(),
        "changed": changed,
    }))
}

fn build_claude_memory_projection(repo_root: &Path, max_lines: usize) -> Result<String, String> {
    build_framework_recap_projection(repo_root, max_lines)
}

fn consolidate_shared_memory(repo_root: &Path) -> Result<Value, String> {
    repair_terminal_resume_allowed(repo_root)?;
    let runtime_snapshot = build_framework_runtime_snapshot_envelope(repo_root)?
        .get("runtime_snapshot")
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| "framework runtime snapshot is missing runtime_snapshot".to_string())?;
    let workspace = repo_root
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("workspace")
        .to_string();
    let resolved_dir = repo_root.join(".codex/memory");
    fs::create_dir_all(&resolved_dir)
        .map_err(|err| format!("create memory directory failed: {err}"))?;
    let archive = archive_legacy_memory_bundle(&workspace, &resolved_dir)?;
    let documents = load_stable_documents(repo_root, &resolved_dir);
    let changed_files = write_documents(&documents, &resolved_dir)?;
    let mut changed_files_with_state = changed_files;
    if let Some(state_path) = write_memory_state(&runtime_snapshot, &resolved_dir)? {
        changed_files_with_state.push(state_path);
    }
    let sqlite_result = persist_memory_bundle(&workspace, &documents, &resolved_dir)?;
    Ok(json!({
        "memory_root": resolved_dir.display().to_string(),
        "changed_files": changed_files_with_state,
        "archive": archive,
        "sqlite_result": sqlite_result,
    }))
}

fn repair_terminal_resume_allowed(repo_root: &Path) -> Result<(), String> {
    let state_path = repo_root.join(".supervisor_state.json");
    let mut supervisor_state = read_json_if_exists(&state_path);
    let needs_repair = supervisor_state
        .as_object()
        .and_then(|state| {
            state
                .get("continuity")
                .and_then(Value::as_object)
                .map(|continuity| (state, continuity))
        })
        .map(|(state, continuity)| {
            continuity.get("resume_allowed").and_then(Value::as_bool) == Some(true)
                && (is_terminal_token(state.get("active_phase"), &TERMINAL_PHASES)
                    || is_terminal_token(
                        state
                            .get("verification")
                            .and_then(Value::as_object)
                            .and_then(|verification| verification.get("verification_status")),
                        &TERMINAL_VERIFICATION_STATUSES,
                    )
                    || is_terminal_token(continuity.get("story_state"), &TERMINAL_STORY_STATES))
        })
        .unwrap_or(false);
    if !needs_repair {
        return Ok(());
    }
    if let Some(state) = supervisor_state.as_object_mut() {
        if let Some(continuity) = state.get_mut("continuity").and_then(Value::as_object_mut) {
            continuity.insert("resume_allowed".to_string(), Value::Bool(false));
        }
    }
    write_json_if_changed(&state_path, &supervisor_state)?;
    Ok(())
}

fn load_stable_documents(repo_root: &Path, resolved_dir: &Path) -> Vec<(String, String)> {
    vec![
        (
            "MEMORY.md".to_string(),
            read_text_if_exists(&resolved_dir.join("MEMORY.md"))
                .if_empty_then(default_memory_md(repo_root)),
        ),
        (
            "preferences.md".to_string(),
            read_text_if_exists(&resolved_dir.join("preferences.md"))
                .if_empty_then("# preferences\n".to_string()),
        ),
        (
            "decisions.md".to_string(),
            read_text_if_exists(&resolved_dir.join("decisions.md"))
                .if_empty_then("# decisions\n".to_string()),
        ),
        (
            "lessons.md".to_string(),
            read_text_if_exists(&resolved_dir.join("lessons.md"))
                .if_empty_then("# lessons\n".to_string()),
        ),
        (
            "runbooks.md".to_string(),
            read_text_if_exists(&resolved_dir.join("runbooks.md"))
                .if_empty_then(default_runbooks()),
        ),
    ]
}

fn default_memory_md(repo_root: &Path) -> String {
    format!(
        "# 项目长期记忆\n\n_本文件沉淀跨会话稳定的项目事实、决策与约定。当前任务态以 continuity artifacts 为准；历史/debug 归档到 `memory/archive/`。_\n\n## 项目身份\n\n- **仓库**: `{}`\n- **闭环事实源**: `artifacts/current/<task_id>/` + `artifacts/current/active_task.json` + `.supervisor_state.json`\n- **默认召回策略**: 稳定层优先，仅在 query 明确命中 active task 且 freshness gate 通过时追加当前任务态\n- **Artifact 分层**: `artifacts/bootstrap/` / `artifacts/ops/memory_automation/` / `artifacts/evidence/` / `artifacts/scratch/`\n",
        repo_root.display()
    )
}

fn default_runbooks() -> String {
    "# runbooks\n\n## 标准操作\n\n- 统一维护入口：python3 scripts/run_memory_automation.py --workspace <workspace>\n- 需要迁移旧 artifact 布局时显式执行：python3 scripts/run_memory_automation.py --workspace <workspace> --apply-artifact-migrations\n- 合并稳定记忆：python3 scripts/consolidate_memory.py --workspace <workspace>\n- 召回上下文：python3 scripts/retrieve_memory.py --workspace <workspace> --mode stable|active|history|debug --topic <关键词>\n- 生命周期收口：./scripts/router-rs/target/release/router-rs --claude-hook-command session-end --repo-root <repo_root> --claude-hook-max-lines 4\n- 诊断快照与存储审计查看 `artifacts/ops/memory_automation/<run_id>/`，不再从 MEMORY_AUTO 或 sessions 读取。\n"
        .to_string()
}

fn archive_legacy_memory_bundle(workspace: &str, resolved_dir: &Path) -> Result<Value, String> {
    let archive_root = resolved_dir
        .join("archive")
        .join(format!("pre-cutover-{}", current_local_date()));
    let mut archived_paths = Vec::new();

    let legacy_path = resolved_dir.join(MEMORY_AUTO_FILENAME);
    if legacy_path.exists() {
        archived_paths.push(move_to_archive(
            &legacy_path,
            &archive_root.join(MEMORY_AUTO_FILENAME),
        )?);
    }
    let sessions_dir = resolved_dir.join("sessions");
    if sessions_dir.exists() {
        archived_paths.push(move_to_archive(
            &sessions_dir,
            &archive_root.join("sessions"),
        )?);
    }

    let db_path = resolved_dir.join(MEMORY_DB_FILENAME);
    let conn = open_memory_store(&db_path)?;
    let legacy_rows = export_rows(
        &conn,
        "SELECT * FROM session_notes WHERE workspace = ? ORDER BY updated_at DESC, session_key DESC, position ASC",
        &[workspace],
    )?;
    let evidence_rows = export_rows(
        &conn,
        "SELECT * FROM evidence_records WHERE workspace = ? ORDER BY updated_at DESC",
        &[workspace],
    )?;
    let memory_items = export_rows(
        &conn,
        "SELECT * FROM memory_items WHERE workspace = ? AND source NOT IN (?, ?, ?, ?, ?) ORDER BY updated_at DESC",
        &[
            workspace,
            STABLE_DOCUMENTS[0],
            STABLE_DOCUMENTS[1],
            STABLE_DOCUMENTS[2],
            STABLE_DOCUMENTS[3],
            STABLE_DOCUMENTS[4],
        ],
    )?;
    let legacy_row_count = legacy_rows.len() + evidence_rows.len();
    let legacy_memory_item_count = memory_items.len();
    if legacy_row_count > 0 || legacy_memory_item_count > 0 {
        fs::create_dir_all(&archive_root)
            .map_err(|err| format!("create archive directory failed: {err}"))?;
        let dump_path = archive_root.join(SQLITE_DUMP_FILENAME);
        let dump_payload = json!({
            "schema_version": "memory-legacy-dump-v1",
            "exported_at": current_local_timestamp(),
            "workspace": workspace,
            "memory_items": memory_items,
            "session_notes": legacy_rows,
            "evidence_records": evidence_rows,
        });
        write_json_if_changed(&dump_path, &dump_payload)?;
        archived_paths.push(dump_path.display().to_string());
        conn.execute(
            "DELETE FROM session_notes WHERE workspace = ?",
            params![workspace],
        )
        .map_err(|err| format!("delete legacy session notes failed: {err}"))?;
        conn.execute(
            "DELETE FROM evidence_records WHERE workspace = ?",
            params![workspace],
        )
        .map_err(|err| format!("delete legacy evidence failed: {err}"))?;
        conn.execute(
            "DELETE FROM memory_items WHERE workspace = ? AND source NOT IN (?, ?, ?, ?, ?)",
            params![
                workspace,
                STABLE_DOCUMENTS[0],
                STABLE_DOCUMENTS[1],
                STABLE_DOCUMENTS[2],
                STABLE_DOCUMENTS[3],
                STABLE_DOCUMENTS[4]
            ],
        )
        .map_err(|err| format!("delete non-authoritative memory items failed: {err}"))?;
    }

    Ok(json!({
        "archive_root": archive_root.display().to_string(),
        "archived_paths": archived_paths,
        "legacy_row_count": legacy_row_count,
        "legacy_memory_item_count": legacy_memory_item_count,
    }))
}

fn persist_memory_bundle(
    workspace: &str,
    documents: &[(String, String)],
    resolved_dir: &Path,
) -> Result<Value, String> {
    let db_path = resolved_dir.join(MEMORY_DB_FILENAME);
    let conn = open_memory_store(&db_path)?;
    let sources = documents
        .iter()
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    let expected_items = documents
        .iter()
        .map(|(_, text)| extract_memory_segments(text).len())
        .sum::<usize>();
    let bundle_hash = build_authoritative_bundle_hash(documents)?;
    let bundle_hash_key = workspace_schema_meta_key(workspace, "authoritative_bundle_hash");
    let existing_bundle_hash = read_schema_meta_value(&conn, &bundle_hash_key)?;
    let existing_items_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM memory_items WHERE workspace = ?",
            params![workspace],
            |row| row.get(0),
        )
        .map_err(|err| format!("count memory items failed: {err}"))?;
    let bundle_matches = existing_bundle_hash.as_deref() == Some(bundle_hash.as_str())
        && existing_items_count == expected_items as i64;

    let mut persisted_items = 0usize;
    if !bundle_matches {
        delete_memory_items_not_in_sources(&conn, workspace, &sources)?;
        delete_memory_items_by_sources(&conn, workspace, &sources)?;

        for (file_name, text) in documents {
            let category = memory_category_for_file(file_name);
            let segments = extract_memory_segments(text);
            for (index, (headings, summary)) in segments.iter().enumerate() {
                let heading_context = headings.join(" / ");
                let item_id = memory_item_id(workspace, category, index + 1, summary, file_name);
                let metadata = json!({
                    "document": file_name,
                    "headings": headings,
                });
                let keywords = json!([summary, file_name, headings]).to_string();
                let now = current_local_timestamp();
                conn.execute(
                    "INSERT INTO memory_items (item_id, workspace, category, source, confidence, status, summary, notes, evidence_json, metadata_json, keywords_json, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(item_id) DO UPDATE SET workspace=excluded.workspace, category=excluded.category, source=excluded.source, confidence=excluded.confidence, status=excluded.status, summary=excluded.summary, notes=excluded.notes, evidence_json=excluded.evidence_json, metadata_json=excluded.metadata_json, keywords_json=excluded.keywords_json, updated_at=excluded.updated_at",
                    params![
                        item_id,
                        workspace,
                        category,
                        file_name,
                        0.8f64,
                        "active",
                        summary,
                        heading_context,
                        "[]",
                        metadata.to_string(),
                        keywords,
                        now,
                        current_local_timestamp(),
                    ],
                )
                .map_err(|err| format!("upsert memory item failed: {err}"))?;
                persisted_items += 1;
            }
        }
        write_schema_meta_value(&conn, &bundle_hash_key, &bundle_hash)?;
    }

    let memory_items_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM memory_items WHERE workspace = ?",
            params![workspace],
            |row| row.get(0),
        )
        .map_err(|err| format!("count memory items failed: {err}"))?;

    Ok(json!({
        "db_path": db_path.display().to_string(),
        "memory_items": memory_items_count,
        "persisted_items": persisted_items,
        "legacy_tables_authoritative": false,
    }))
}

fn write_documents(
    documents: &[(String, String)],
    resolved_dir: &Path,
) -> Result<Vec<String>, String> {
    fs::create_dir_all(resolved_dir)
        .map_err(|err| format!("create memory directory failed: {err}"))?;
    let mut changed_files = Vec::new();
    for (file_name, text) in documents {
        let path = resolved_dir.join(file_name);
        if write_text_if_changed(&path, text)? {
            changed_files.push(path.canonicalize().unwrap_or(path).display().to_string());
        }
    }
    Ok(changed_files)
}

fn write_memory_state(
    runtime_snapshot: &Map<String, Value>,
    resolved_dir: &Path,
) -> Result<Option<String>, String> {
    let path = resolved_dir.join(MEMORY_STATE_FILENAME);
    let existing = read_json_if_exists(&path);
    let payload = build_memory_state(runtime_snapshot, Some(&existing))?;
    if write_json_if_changed(&path, &payload)? {
        let resolved = path.canonicalize().unwrap_or(path);
        return Ok(Some(resolved.display().to_string()));
    }
    Ok(None)
}

fn build_memory_state(
    runtime_snapshot: &Map<String, Value>,
    existing_state: Option<&Value>,
) -> Result<Value, String> {
    let continuity = runtime_snapshot
        .get("continuity")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let paths = runtime_snapshot
        .get("paths")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let source_updated_at = continuity
        .get("continuity")
        .and_then(Value::as_object)
        .and_then(|inner| inner.get("last_updated_at"))
        .and_then(Value::as_str)
        .map(|value| value.to_string());
    let source_hash = build_runtime_source_hash(&paths, runtime_snapshot.get("active_task_id"))?;
    let reused_source_updated_at = existing_state
        .and_then(Value::as_object)
        .filter(|existing| {
            existing.get("content_hash").and_then(Value::as_str) == Some(source_hash.as_str())
        })
        .and_then(|existing| existing.get("source_updated_at"))
        .and_then(Value::as_str)
        .map(|value| value.to_string());
    let source_updated_at = source_updated_at
        .or(reused_source_updated_at)
        .or_else(|| {
            runtime_snapshot
                .get("collected_at")
                .and_then(Value::as_str)
                .map(|value| value.to_string())
        })
        .unwrap_or_default();
    let mut payload = json!({
        "schema_version": MEMORY_STATE_SCHEMA_VERSION,
        "source_task_id": runtime_snapshot.get("active_task_id").cloned().unwrap_or(Value::Null),
        "source_task": continuity.get("task").cloned().unwrap_or(Value::Null),
        "source_phase": continuity.get("phase").cloned().unwrap_or(Value::Null),
        "source_status": continuity.get("status").cloned().unwrap_or(Value::Null),
        "continuity_state": continuity.get("state").cloned().unwrap_or(Value::Null),
        "artifact_root": runtime_snapshot.get("current_root").cloned().unwrap_or(Value::Null),
        "source_updated_at": source_updated_at,
        "content_hash": source_hash,
        "last_consolidated_at": current_local_timestamp(),
    });
    if let Some(existing) = existing_state {
        if memory_state_matches_ignoring_timestamp(existing, &payload) {
            if let Some(previous_timestamp) =
                existing.get("last_consolidated_at").and_then(Value::as_str)
            {
                payload["last_consolidated_at"] = Value::String(previous_timestamp.to_string());
            }
        }
    }
    Ok(payload)
}

fn build_runtime_source_hash(
    paths: &Map<String, Value>,
    active_task_id: Option<&Value>,
) -> Result<String, String> {
    let payload = json!({
        "active_task_id": active_task_id.cloned().unwrap_or(Value::Null),
        "session_summary_text": read_text_if_exists(Path::new(&value_text(paths.get("session_summary")))),
        "next_actions": read_json_if_exists(Path::new(&value_text(paths.get("next_actions")))),
        "evidence_index": read_json_if_exists(Path::new(&value_text(paths.get("evidence_index")))),
        "trace_metadata": read_json_if_exists(Path::new(&value_text(paths.get("trace_metadata")))),
        "supervisor_state": read_json_if_exists(Path::new(&value_text(paths.get("supervisor_state")))),
    });
    let encoded =
        serde_json::to_vec(&payload).map_err(|err| format!("encode hash payload failed: {err}"))?;
    let mut hasher = Sha256::new();
    hasher.update(encoded);
    Ok(format!("{:x}", hasher.finalize()))
}

fn build_authoritative_bundle_hash(documents: &[(String, String)]) -> Result<String, String> {
    let encoded = serde_json::to_vec(documents)
        .map_err(|err| format!("encode authoritative bundle hash failed: {err}"))?;
    let mut hasher = Sha256::new();
    hasher.update(encoded);
    Ok(format!("{:x}", hasher.finalize()))
}

fn workspace_schema_meta_key(workspace: &str, suffix: &str) -> String {
    format!("workspace:{}:{suffix}", safe_slug(workspace))
}

fn read_schema_meta_value(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    let mut statement = conn
        .prepare("SELECT value FROM schema_meta WHERE key = ?")
        .map_err(|err| format!("prepare schema meta lookup failed: {err}"))?;
    match statement.query_row(params![key], |row| row.get::<_, String>(0)) {
        Ok(value) => Ok(Some(value)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(err) => Err(format!("read schema meta value failed: {err}")),
    }
}

fn write_schema_meta_value(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO schema_meta(key, value, updated_at) VALUES (?, ?, ?) ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at",
        params![key, value, current_local_timestamp()],
    )
    .map_err(|err| format!("write schema meta value failed: {err}"))?;
    Ok(())
}

fn memory_state_matches_ignoring_timestamp(existing: &Value, candidate: &Value) -> bool {
    let Some(existing_object) = existing.as_object() else {
        return false;
    };
    let Some(candidate_object) = candidate.as_object() else {
        return false;
    };
    let mut existing_cmp = existing_object.clone();
    let mut candidate_cmp = candidate_object.clone();
    existing_cmp.remove("last_consolidated_at");
    candidate_cmp.remove("last_consolidated_at");
    existing_cmp == candidate_cmp
}

fn run_config_change(repo_root: &Path, payload: &Value) -> Result<Value, String> {
    let scope = payload
        .get("source")
        .or_else(|| payload.get("scope"))
        .or_else(|| payload.get("matcher"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let mut rel_paths = HashSet::new();
    for path in iter_candidate_paths(payload) {
        rel_paths.insert(relative_candidate_path(&path, repo_root));
    }
    let mentions_continuity = payload_mentions_continuity(payload);
    let mut notices = Vec::new();
    if mentions_continuity {
        let message = "[claude-config-change] payload referenced shared continuity artifacts; leaving them untouched and keeping audit host-private.";
        eprintln!("{message}");
        notices.push(message.to_string());
    }
    if scope == "project_settings" {
        let hits = rel_paths
            .iter()
            .filter(|path| GENERATED_PATHS.contains(&path.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if hits.is_empty() {
            let message =
                "[claude-config-change] project settings changed outside generated Claude host surfaces; no action taken.";
            eprintln!("{message}");
            notices.push(message.to_string());
        } else {
            let message = format!(
                "[claude-config-change] detected edits on generated Claude host surfaces: {}; regenerate via scripts/materialize_cli_host_entrypoints.py instead of hand-editing outputs.",
                hits.join(", ")
            );
            eprintln!("{message}");
            notices.push(message);
        }
    }
    Ok(json!({
        "schema_version": CLAUDE_HOOK_AUDIT_SCHEMA_VERSION,
        "authority": CLAUDE_HOOK_AUDIT_AUTHORITY,
        "command": "config-change",
        "repo_root": repo_root.display().to_string(),
        "scope": scope,
        "notices": notices,
    }))
}

fn run_user_prompt_submit(repo_root: &Path, payload: &Value) -> Result<Value, String> {
    let prompt_text = extract_user_prompt_text(payload);
    let context_payload = build_user_prompt_context_payload(repo_root, &prompt_text)?;
    let context = context_payload
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| "user prompt context payload missing text".to_string())?;
    let telemetry = context_payload
        .get("telemetry")
        .cloned()
        .unwrap_or(Value::Null);
    Ok(json!({
        "schema_version": CLAUDE_HOOK_AUDIT_SCHEMA_VERSION,
        "authority": CLAUDE_HOOK_AUDIT_AUTHORITY,
        "command": "user-prompt-submit",
        "hookSpecificOutput": {
            "hookEventName": "UserPromptSubmit",
            "additionalContext": context,
        },
        "contextTelemetry": telemetry,
    }))
}

fn run_pre_tool_use(repo_root: &Path, payload: &Value) -> Result<Value, String> {
    let tool_name = payload
        .get("tool_name")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let mut rel_paths = HashSet::new();
    for path in iter_payload_paths(payload) {
        rel_paths.insert(relative_candidate_path(&path, repo_root));
    }
    for path in rel_paths.iter().cloned().collect::<Vec<_>>() {
        if classify_protected_generated_path(&path).is_some() {
            let message = pre_tool_use_message(&path);
            return Ok(json!({
                "schema_version": CLAUDE_HOOK_AUDIT_SCHEMA_VERSION,
                "authority": CLAUDE_HOOK_AUDIT_AUTHORITY,
                "command": "pre-tool-use",
                "tool_name": tool_name,
                "decision": "deny",
                "path": path,
                "message": message,
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "deny",
                    "permissionDecisionReason": message,
                },
            }));
        }
    }
    if let Some(path) = bash_generated_write_target(payload) {
        let message = pre_tool_use_message(&path);
        return Ok(json!({
            "schema_version": CLAUDE_HOOK_AUDIT_SCHEMA_VERSION,
            "authority": CLAUDE_HOOK_AUDIT_AUTHORITY,
            "command": "pre-tool-use",
            "tool_name": tool_name,
            "decision": "deny",
            "path": path,
            "message": message,
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": message,
            },
        }));
    }
    Ok(json!({
        "schema_version": CLAUDE_HOOK_AUDIT_SCHEMA_VERSION,
        "authority": CLAUDE_HOOK_AUDIT_AUTHORITY,
        "command": "pre-tool-use",
        "tool_name": tool_name,
        "decision": "allow",
    }))
}

fn run_pre_tool_use_quality(repo_root: &Path, payload: &Value) -> Result<Value, String> {
    let tool_name = payload
        .get("tool_name")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if !matches!(tool_name, "Edit" | "MultiEdit" | "Write") {
        return Ok(json!({
            "schema_version": CLAUDE_HOOK_AUDIT_SCHEMA_VERSION,
            "authority": CLAUDE_HOOK_AUDIT_AUTHORITY,
            "command": "pre-tool-use-quality",
            "tool_name": tool_name,
        }));
    }

    let mut rel_paths = iter_payload_paths(payload)
        .into_iter()
        .map(|path| relative_candidate_path(&path, repo_root))
        .collect::<Vec<_>>();
    rel_paths.sort();
    rel_paths.dedup();

    for path in &rel_paths {
        if is_async_audit_target(path) && is_quality_target_path(path) {
            store_pre_edit_snapshot(repo_root, path)?;
        }
        if let Some(context) = quality_target_context(path) {
            return Ok(json!({
                "schema_version": CLAUDE_HOOK_AUDIT_SCHEMA_VERSION,
                "authority": CLAUDE_HOOK_AUDIT_AUTHORITY,
                "command": "pre-tool-use-quality",
                "tool_name": tool_name,
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "allow",
                    "permissionDecisionReason": "Apply repo implementation-quality defaults.",
                    "additionalContext": context,
                },
            }));
        }
    }

    Ok(json!({
        "schema_version": CLAUDE_HOOK_AUDIT_SCHEMA_VERSION,
        "authority": CLAUDE_HOOK_AUDIT_AUTHORITY,
        "command": "pre-tool-use-quality",
        "tool_name": tool_name,
    }))
}

fn run_post_tool_audit(repo_root: &Path, payload: &Value) -> Result<Value, String> {
    let tool_name = payload
        .get("tool_name")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if !matches!(tool_name, "Edit" | "MultiEdit" | "Write") {
        return Ok(json!({
            "schema_version": CLAUDE_HOOK_AUDIT_SCHEMA_VERSION,
            "authority": CLAUDE_HOOK_AUDIT_AUTHORITY,
            "command": "post-tool-audit",
            "tool_name": tool_name,
        }));
    }

    let mut rel_paths = iter_payload_paths(payload)
        .into_iter()
        .map(|path| relative_candidate_path(&path, repo_root))
        .collect::<Vec<_>>();
    rel_paths.sort();
    rel_paths.dedup();

    for path in rel_paths {
        if !is_async_audit_target(&path) || !is_quality_target_path(&path) {
            continue;
        }
        let (delta_text, source_mode) = extract_audit_delta(repo_root, &path, payload)?;
        if delta_text.trim().is_empty() {
            continue;
        }
        let Some(context) = build_async_audit_context(&path, &delta_text, &source_mode) else {
            continue;
        };
        return Ok(json!({
            "schema_version": CLAUDE_HOOK_AUDIT_SCHEMA_VERSION,
            "authority": CLAUDE_HOOK_AUDIT_AUTHORITY,
            "command": "post-tool-audit",
            "tool_name": tool_name,
            "hookSpecificOutput": {
                "hookEventName": "PostToolUse",
                "additionalContext": context,
            },
            "additionalContext": context,
        }));
    }

    Ok(json!({
        "schema_version": CLAUDE_HOOK_AUDIT_SCHEMA_VERSION,
        "authority": CLAUDE_HOOK_AUDIT_AUTHORITY,
        "command": "post-tool-audit",
        "tool_name": tool_name,
    }))
}

fn run_stop_failure(_repo_root: &Path, payload: &Value) -> Result<Value, String> {
    let failure_type = payload
        .get("error")
        .or_else(|| payload.get("failure_type"))
        .or_else(|| payload.get("matcher"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let continuity_note = if payload_mentions_continuity(payload) {
        " Shared continuity remains untouched."
    } else {
        ""
    };
    let message = format!(
        "[claude-stop-failure] Claude stop failure classified as {failure_type}; inspect /hooks, generated host files, and host-private projection drift before retrying.{continuity_note}"
    );
    eprintln!("{message}");
    Ok(json!({
        "schema_version": CLAUDE_HOOK_AUDIT_SCHEMA_VERSION,
        "authority": CLAUDE_HOOK_AUDIT_AUTHORITY,
        "command": "stop-failure",
        "failure_type": failure_type,
        "message": message,
    }))
}

fn read_stdin_payload() -> Result<Value, String> {
    let mut raw = String::new();
    io::stdin()
        .read_to_string(&mut raw)
        .map_err(|err| format!("read stdin payload failed: {err}"))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str::<Value>(trimmed).or_else(|_| Ok(json!({ "raw": trimmed })))
}

fn iter_candidate_paths(payload: &Value) -> Vec<String> {
    let mut candidates = Vec::new();
    for key in [
        "file_path",
        "changed_path",
        "path",
        "config_path",
        "target_path",
    ] {
        if let Some(text) = payload.get(key).and_then(Value::as_str) {
            let normalized = text.replace('\\', "/");
            if !normalized.is_empty() {
                candidates.push(normalized);
            }
        }
    }
    if let Some(items) = payload.get("changed_files").and_then(Value::as_array) {
        for item in items {
            if let Some(text) = item.as_str() {
                let normalized = text.replace('\\', "/");
                if !normalized.is_empty() {
                    candidates.push(normalized);
                }
            }
        }
    }
    candidates
}

fn iter_payload_paths(payload: &Value) -> Vec<String> {
    let mut candidates = iter_candidate_paths(payload);
    if let Some(tool_input) = payload.get("tool_input") {
        candidates.extend(iter_candidate_paths(tool_input));
    }
    candidates
}

fn relative_candidate_path(path: &str, repo_root: &Path) -> String {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        if let Ok(rel) = candidate
            .canonicalize()
            .unwrap_or(candidate.clone())
            .strip_prefix(
                repo_root
                    .canonicalize()
                    .unwrap_or_else(|_| repo_root.to_path_buf()),
            )
        {
            return rel.to_string_lossy().replace('\\', "/");
        }
    }
    path.replace('\\', "/")
}

fn payload_mentions_continuity(payload: &Value) -> bool {
    let serialized = serde_json::to_string(payload).unwrap_or_default();
    SHARED_CONTINUITY_PATHS
        .iter()
        .any(|needle| serialized.contains(needle))
}

fn extract_user_prompt_text(payload: &Value) -> String {
    let mut values = Vec::new();
    for key in ["prompt", "user_prompt", "text", "message"] {
        if let Some(text) = payload.get(key).and_then(Value::as_str) {
            values.push(text.trim().to_string());
        }
    }
    if let Some(text) = payload.get("input").and_then(Value::as_str) {
        values.push(text.trim().to_string());
    }
    if let Some(nested) = payload.get("payload").and_then(Value::as_object) {
        for key in ["prompt", "user_prompt", "text", "message"] {
            if let Some(text) = nested.get(key).and_then(Value::as_str) {
                values.push(text.trim().to_string());
            }
        }
    }
    values
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn count_contains(text: &str, tokens: &[&str]) -> usize {
    tokens.iter().filter(|token| text.contains(**token)).count()
}

fn prompt_path_mentions(prompt_text: &str, code_only: bool) -> Vec<String> {
    let suffixes = if code_only {
        "(?:py|rs|sh)"
    } else {
        "(?:py|rs|sh|json)"
    };
    let pattern = format!(r"(^|[^A-Za-z0-9_])([\w./-]+\.{suffixes})([^A-Za-z0-9_]|$)");
    let Ok(regex) = Regex::new(&pattern) else {
        return Vec::new();
    };
    let mut results = Vec::new();
    for captures in regex.captures_iter(prompt_text) {
        if let Some(path) = captures
            .get(2)
            .map(|value| value.as_str().replace('\\', "/"))
        {
            results.push(path);
        }
    }
    results
}

fn markdown_path_mentions(prompt_text: &str) -> Vec<String> {
    let Ok(regex) = Regex::new(r"(^|[^A-Za-z0-9_])([\w./-]+\.md)([^A-Za-z0-9_]|$)") else {
        return Vec::new();
    };
    let mut results = Vec::new();
    for captures in regex.captures_iter(prompt_text) {
        if let Some(path) = captures
            .get(2)
            .map(|value| value.as_str().replace('\\', "/"))
        {
            results.push(path);
        }
    }
    results
}

fn looks_like_coding_request(prompt_text: &str) -> bool {
    if prompt_text.trim().is_empty() {
        return false;
    }
    let lowered = prompt_text.to_lowercase();
    if prompt_text.contains("/autopilot")
        || prompt_text.contains("/deepinterview")
        || prompt_text.contains("/team")
        || lowered.contains("autopilot")
        || lowered.contains("deepinterview")
        || lowered.contains("deep-interview")
        || lowered.contains("team mode")
    {
        return true;
    }
    let markdown_mentions = markdown_path_mentions(prompt_text);
    let markdown_execution_mentions = markdown_mentions
        .iter()
        .filter(|path| {
            matches!(
                path.as_str(),
                "AGENT.md"
                    | "AGENTS.md"
                    | "CLAUDE.md"
                    | "GEMINI.md"
                    | ".claude/hooks/README.md"
                    | ".claude/agents/README.md"
            )
        })
        .count();
    if !markdown_mentions.is_empty()
        && prompt_path_mentions(prompt_text, false).is_empty()
        && markdown_execution_mentions == 0
    {
        return false;
    }
    let lowered = prompt_text.to_lowercase();
    let strong_actions = count_contains(&lowered, &USER_PROMPT_STRONG_ACTION_TERMS);
    let weak_actions = count_contains(&lowered, &USER_PROMPT_WEAK_ACTION_TERMS);
    let code_targets = count_contains(&lowered, &USER_PROMPT_CODE_TARGET_TERMS);
    let path_mentions = prompt_path_mentions(prompt_text, false).len();
    let non_code_edits = count_contains(&lowered, &USER_PROMPT_NON_CODE_EDIT_TERMS);
    let action_score = strong_actions * 2 + weak_actions;
    let target_score = code_targets + path_mentions * 2 + markdown_execution_mentions * 2;
    if action_score == 0 || target_score == 0 {
        return false;
    }
    if non_code_edits >= 2 && strong_actions == 0 && target_score < 2 {
        return false;
    }
    action_score + target_score >= non_code_edits + 3
}

fn join_unique_context(parts: &[&str]) -> String {
    let mut seen = HashSet::new();
    let mut ordered = Vec::new();
    for part in parts {
        if part.is_empty() || !seen.insert(*part) {
            continue;
        }
        ordered.push(*part);
    }
    ordered.join(" ")
}

fn user_prompt_memory_projection(repo_root: &Path, max_lines: usize) -> Result<String, String> {
    let projection = build_claude_memory_projection(repo_root, max_lines)?;
    let lines = projection
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty()
                && trimmed != "# Claude Startup Projection"
                && !trimmed.starts_with("_Generated from shared runtime artifacts")
        })
        .collect::<Vec<_>>();
    Ok(lines.join("\n"))
}

fn compact_user_prompt_projection(
    repo_root: &Path,
    state_budget_chars: usize,
) -> Result<String, String> {
    let projection = user_prompt_memory_projection(repo_root, 1)?;
    let summary = projection
        .lines()
        .map(str::trim)
        .find_map(|line| {
            if line.is_empty() || line.starts_with("##") {
                return None;
            }
            let normalized = line.trim_start_matches("- ").trim();
            if normalized.starts_with("current:")
                || normalized.starts_with("recent task:")
                || normalized.starts_with("next:")
                || normalized.starts_with("continuity:")
                || normalized.starts_with("last_known:")
            {
                return Some(normalized.to_string());
            }
            None
        })
        .or_else(|| {
            projection
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty() && !line.starts_with("##") && !line.starts_with('-'))
                .map(|line| line.to_string())
        })
        .unwrap_or_default();
    if summary.is_empty() {
        return Ok(String::new());
    }
    let shortened = if summary.chars().count() > state_budget_chars {
        let truncated = summary
            .chars()
            .take(state_budget_chars.saturating_sub(1))
            .collect::<String>();
        format!("{truncated}…")
    } else {
        summary
    };
    Ok(format!("{USER_PROMPT_STATE_COMPACT_PREFIX}{shortened}"))
}

fn complex_coding_request(prompt_text: &str) -> bool {
    let lowered = prompt_text.to_lowercase();
    if prompt_text.contains("/autopilot")
        || prompt_text.contains("/deepinterview")
        || prompt_text.contains("/team")
        || lowered.contains("autopilot")
        || lowered.contains("deepinterview")
        || lowered.contains("deep-interview")
        || lowered.contains("team mode")
    {
        return true;
    }
    if (lowered.contains("root cause") || lowered.contains("根因"))
        && (lowered.contains("resume") || lowered.contains("续跑") || lowered.contains("恢复"))
    {
        return true;
    }
    let path_mentions = prompt_path_mentions(prompt_text, false).len();
    let strong_actions = count_contains(&lowered, &USER_PROMPT_STRONG_ACTION_TERMS);
    let code_targets = count_contains(&lowered, &USER_PROMPT_CODE_TARGET_TERMS);
    path_mentions >= 2 || (strong_actions >= 3 && code_targets >= 4)
}

fn execution_intent_summary(prompt_text: &str) -> Option<String> {
    if !looks_like_coding_request(prompt_text) || !complex_coding_request(prompt_text) {
        return None;
    }
    let lowered = prompt_text.to_lowercase();
    let mut parts = Vec::new();
    if prompt_text.contains("/deepinterview")
        || lowered.contains("deepinterview")
        || lowered.contains("deep-interview")
    {
        parts.push("先澄清最弱维度，再决定是否 handoff");
    }
    if prompt_text.contains("/team")
        || lowered.contains("team mode")
        || lowered.contains("多 agent")
        || lowered.contains("worker")
    {
        parts.push("shared continuity 只允许 supervisor 持有");
    }
    if prompt_text.contains("/autopilot") || lowered.contains("autopilot") {
        parts.push("优先续跑当前执行链，不把中断当完成");
    }
    if lowered.contains("root cause") || lowered.contains("根因") {
        parts.push("根因未明时先定位，不机械重试");
    }
    if lowered.contains("resume") || lowered.contains("续跑") || lowered.contains("恢复") {
        parts.push("先核对恢复锚点，再继续执行");
    }
    if parts.is_empty() {
        parts.push("保留执行语义，优先验证与恢复锚点");
    }
    Some(format!(
        "{USER_PROMPT_EXECUTION_INTENT_PREFIX}{}",
        parts.join("；")
    ))
}

fn user_prompt_context_budget(prompt_text: &str) -> (usize, usize) {
    if looks_like_coding_request(prompt_text) && complex_coding_request(prompt_text) {
        (
            USER_PROMPT_COMPLEX_CONTEXT_MAX_CHARS,
            USER_PROMPT_COMPLEX_STATE_BUDGET_CHARS,
        )
    } else {
        (
            USER_PROMPT_CONTEXT_MAX_CHARS,
            USER_PROMPT_STATE_BUDGET_CHARS,
        )
    }
}

fn shrink_user_prompt_context(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    while !lines.is_empty() && lines.join("\n\n").chars().count() > max_chars {
        lines.pop();
    }
    if lines.is_empty() {
        return text.chars().take(max_chars).collect();
    }
    lines.join("\n\n")
}

fn build_user_prompt_context_payload(repo_root: &Path, prompt_text: &str) -> Result<Value, String> {
    let (context_budget_chars, state_budget_chars) = user_prompt_context_budget(prompt_text);
    let mut sections = vec![
        USER_PROMPT_MEMORY_PRIORITY_CONTEXT.to_string(),
        USER_PROMPT_CONTINUITY_CONTEXT.to_string(),
    ];
    let mut lanes = vec!["memory-truth".to_string(), "continuity-truth".to_string()];
    let projection = compact_user_prompt_projection(repo_root, state_budget_chars)?;
    if !projection.trim().is_empty() {
        sections.push(projection);
        lanes.push("state-compact".to_string());
    }
    if let Some(intent) = execution_intent_summary(prompt_text) {
        sections.push(intent);
        lanes.push("execution-intent".to_string());
    }
    if looks_like_coding_request(prompt_text) {
        let lowered = prompt_text.to_lowercase();
        let mut parts = Vec::new();
        if count_contains(&lowered, &USER_PROMPT_PERF_TERMS) > 0 {
            parts.push(USER_PROMPT_PERF_CONTEXT);
            lanes.push("perf".to_string());
        }
        if count_contains(&lowered, &USER_PROMPT_COMPAT_TERMS) > 0 {
            parts.push(USER_PROMPT_COMPAT_CONTEXT);
            lanes.push("compat".to_string());
        }
        let path_hints = prompt_path_mentions(prompt_text, true);
        let path_hint_has_hook = path_hints
            .iter()
            .any(|hint| hint.to_lowercase().contains("hook"));
        if count_contains(&lowered, &USER_PROMPT_HOOK_TERMS) > 0 || path_hint_has_hook {
            parts.push(USER_PROMPT_HOOK_CONTEXT);
            lanes.push("hook".to_string());
        }
        if !parts.is_empty() {
            sections.push(join_unique_context(&parts));
        }
    }
    let pre_shrink = sections.join("\n\n");
    let context = shrink_user_prompt_context(&pre_shrink, context_budget_chars);
    let trimmed = context.chars().count() < pre_shrink.chars().count();
    Ok(json!({
        "text": context,
        "telemetry": {
            "lanes": lanes,
            "char_count": pre_shrink.chars().count(),
            "final_char_count": pre_shrink.chars().count().min(context_budget_chars).min(context.chars().count()),
            "trimmed": trimmed,
            "budget_chars": context_budget_chars,
            "state_budget_chars": state_budget_chars,
        }
    }))
}

fn is_quality_target_path(path: &str) -> bool {
    QUALITY_TARGET_SUFFIXES
        .iter()
        .any(|suffix| path.ends_with(suffix))
}

fn quality_target_context(path: &str) -> Option<String> {
    if !is_quality_target_path(path) {
        return None;
    }
    let lowered_path = path.to_lowercase();
    let is_runtime = QUALITY_RUNTIME_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix));
    let is_hook = QUALITY_HOOK_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix))
        || lowered_path.contains("hook");
    let is_test = path.starts_with("tests/");
    let is_materializer = path == "scripts/materialize_cli_host_entrypoints.py";
    if !(is_runtime || is_hook || is_test || is_materializer) {
        return None;
    }

    let mut parts = Vec::new();
    if is_runtime {
        parts.push(USER_PROMPT_PERF_CONTEXT);
    }
    if path.ends_with(".rs") && is_runtime {
        parts.push(QUALITY_RUST_CONTEXT);
    }
    if path.ends_with(".py") && (is_runtime || is_hook || is_materializer) {
        parts.push(QUALITY_PYTHON_CONTEXT);
    }
    if is_hook || path.ends_with(".sh") || is_materializer {
        parts.push(USER_PROMPT_HOOK_CONTEXT);
    }
    if is_test {
        parts.push(QUALITY_TEST_CONTEXT);
    }
    if parts.is_empty() {
        return None;
    }
    Some(join_unique_context(&parts))
}

fn snapshot_root() -> PathBuf {
    env::temp_dir().join(CLAUDE_HOOK_SNAPSHOT_ROOT_DIRNAME)
}

fn read_text_if_small(path: &Path) -> Option<String> {
    let metadata = path.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > SNAPSHOT_MAX_BYTES {
        return None;
    }
    fs::read_to_string(path).ok()
}

fn snapshot_path(repo_root: &Path, path: &str) -> PathBuf {
    let key = Sha256::digest(format!("{}::{path}", repo_root.display()).as_bytes());
    let key_hex = format!("{key:x}");
    snapshot_root()
        .join(&key_hex[..2])
        .join(format!("{key_hex}.json"))
}

fn store_pre_edit_snapshot(repo_root: &Path, path: &str) -> Result<(), String> {
    let target = repo_root.join(path);
    let payload = if target.is_file() {
        let Some(text) = read_text_if_small(&target) else {
            return Ok(());
        };
        json!({ "exists": true, "text": text })
    } else {
        json!({ "exists": false, "text": "" })
    };
    let snapshot = snapshot_path(repo_root, path);
    if let Some(parent) = snapshot.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create snapshot parent failed: {err}"))?;
    }
    let text = serde_json::to_string(&payload)
        .map_err(|err| format!("serialize snapshot failed: {err}"))?;
    fs::write(&snapshot, text).map_err(|err| format!("write snapshot failed: {err}"))?;
    Ok(())
}

fn pop_pre_edit_snapshot(repo_root: &Path, path: &str) -> Option<(String, String)> {
    let snapshot = snapshot_path(repo_root, path);
    if !snapshot.is_file() {
        return None;
    }
    let text = fs::read_to_string(&snapshot).ok();
    let _ = fs::remove_file(&snapshot);
    let payload = serde_json::from_str::<Value>(&text?).ok()?;
    let before = payload
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mode = if payload.get("exists").and_then(Value::as_bool) == Some(true) {
        "pre_tool_snapshot_added_lines"
    } else {
        "pre_tool_snapshot_new_file"
    };
    Some((before, mode.to_string()))
}

fn compat_smell_count(text: &str) -> usize {
    Regex::new(COMPAT_SMELL_PATTERN)
        .ok()
        .map(|regex| regex.find_iter(text).count())
        .unwrap_or(0)
}

fn git_tracked_text(repo_root: &Path, path: &str) -> String {
    Command::new("git")
        .arg("show")
        .arg(format!("HEAD:{path}"))
        .current_dir(repo_root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .unwrap_or_default()
}

fn added_lines(before: &str, after: &str) -> String {
    let mut before_counts = HashMap::new();
    for line in before.lines() {
        *before_counts.entry(line.to_string()).or_insert(0usize) += 1;
    }
    let mut added = Vec::new();
    for line in after.lines() {
        if let Some(count) = before_counts.get_mut(line) {
            if *count > 0 {
                *count -= 1;
                continue;
            }
        }
        added.push(line);
    }
    added.join("\n").trim().to_string()
}

fn extract_multi_edit_delta(tool_input: &Map<String, Value>) -> String {
    tool_input
        .get("edits")
        .and_then(Value::as_array)
        .map(|edits| {
            edits
                .iter()
                .filter_map(Value::as_object)
                .filter_map(|edit| edit.get("new_string").and_then(Value::as_str))
                .filter(|text| !text.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n")
                .trim()
                .to_string()
        })
        .unwrap_or_default()
}

fn extract_audit_delta(
    repo_root: &Path,
    path: &str,
    payload: &Value,
) -> Result<(String, String), String> {
    let tool_name = payload
        .get("tool_name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let tool_input = payload
        .get("tool_input")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    if let Some((before_text, mode)) = pop_pre_edit_snapshot(repo_root, path) {
        let after_path = repo_root.join(path);
        let after_text = if after_path.exists() {
            let Some(text) = read_text_if_small(&after_path) else {
                return Ok(("".to_string(), "snapshot_skip_large_after".to_string()));
            };
            text
        } else {
            String::new()
        };
        let added = added_lines(&before_text, &after_text);
        let source_mode = if added.is_empty() {
            format!("{mode}_no_added_lines")
        } else {
            mode
        };
        return Ok((added, source_mode));
    }

    if tool_name == "Edit" {
        return Ok((
            tool_input
                .get("new_string")
                .and_then(Value::as_str)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_default(),
            if tool_input
                .get("new_string")
                .and_then(Value::as_str)
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
            {
                "edit_new_string".to_string()
            } else {
                "edit_empty".to_string()
            },
        ));
    }

    if tool_name == "MultiEdit" {
        let delta = extract_multi_edit_delta(&tool_input);
        return Ok((
            delta.clone(),
            if delta.is_empty() {
                "multi_edit_empty".to_string()
            } else {
                "multi_edit_new_strings".to_string()
            },
        ));
    }

    if tool_name == "Write" {
        let content = tool_input
            .get("content")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_default();
        if content.trim().is_empty() {
            return Ok(("".to_string(), "write_empty".to_string()));
        }
        let tracked = git_tracked_text(repo_root, path);
        if !tracked.is_empty() {
            let added = added_lines(&tracked, &content);
            return Ok((
                added.clone(),
                if added.is_empty() {
                    "write_no_added_lines".to_string()
                } else {
                    "write_git_added_lines".to_string()
                },
            ));
        }
        if !repo_root.join(path).exists() {
            return Ok((content, "write_new_file".to_string()));
        }
        return Ok((
            "".to_string(),
            "write_skip_existing_without_base".to_string(),
        ));
    }

    Ok(("".to_string(), "unsupported_tool".to_string()))
}

fn build_async_audit_context(path: &str, text: &str, source_mode: &str) -> Option<String> {
    let compat_hits = compat_smell_count(text);
    let lowered_path = path.to_lowercase();
    let source_label = format!("增量来源={source_mode}");
    let mut parts = Vec::new();

    if path.ends_with(".rs") {
        let clone_hits = text.matches(".clone(").count() + text.matches(".clone()").count();
        let serde_hits = text.matches("serde_json::").count();
        let string_hits =
            text.matches(".to_string()").count() + text.matches(".to_owned()").count();
        if compat_hits >= 1 || clone_hits >= 2 || serde_hits >= 2 || string_hits >= 3 {
            parts.push(format!(
                "`{path}` 的新增片段有实现复查信号：{source_label}, compat={compat_hits}, clone={clone_hits}, serde={serde_hits}, string_copy={string_hits}。"
            ));
            parts.push(
                "如果这轮还在继续，先看新增兼容分支或中转层能不能直接收掉，再压缩热路径里的 clone 和序列化往返。"
                    .to_string(),
            );
        }
    } else if path.ends_with(".py") {
        let json_hits = text.matches("json.loads(").count() + text.matches("json.dumps(").count();
        let io_hits = text.matches(".read_text(").count()
            + text.matches(".read_bytes(").count()
            + text.matches(".write_text(").count();
        let wrapper_hits = if lowered_path.contains("hook") {
            text.matches("def ").count()
        } else {
            0
        };
        if compat_hits >= 1 || json_hits >= 2 || io_hits >= 2 {
            parts.push(format!(
                "`{path}` 的新增片段有实现复查信号：{source_label}, compat={compat_hits}, json_roundtrip={json_hits}, file_io={io_hits}。"
            ));
            parts.push(
                "如果这轮还在继续，先看新增兼容/补丁分支能不能直接收掉，再减少重复解析、重复读写和 wrapper-on-wrapper。"
                    .to_string(),
            );
        } else if lowered_path.contains("hook") && compat_hits >= 1 {
            parts.push(format!(
                "`{path}` 的新增片段仍带有明显的 hook 过渡信号：{source_label}, compat={compat_hits}, helper_defs={wrapper_hits}。"
            ));
            parts.push(
                "确认这层是在增加自动化，而不是只多加一道阻拦或中转包装；优先把新增中转层收回到更短路径。"
                    .to_string(),
            );
        }
    } else if path.ends_with(".sh") {
        let deny_hits = text.matches("permissionDecision").count();
        if lowered_path.contains("hook") && (compat_hits >= 1 || deny_hits >= 1) {
            parts.push(format!(
                "`{path}` 的新增片段有 hook 复查信号：{source_label}, compat={compat_hits}, deny_rules={deny_hits}。"
            ));
            parts.push(
                "确认这层仍然是短路径、低开销、加自动化；优先删掉新增阻拦规则或合并重复判断。"
                    .to_string(),
            );
        }
    }

    if parts.is_empty() {
        return None;
    }
    Some(format!("异步实现复查：{}", parts.join(" ")))
}

fn is_async_audit_target(path: &str) -> bool {
    ASYNC_AUDIT_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix))
}

fn classify_protected_generated_path(path: &str) -> Option<&'static str> {
    if PROTECTED_GENERATED_PATHS.contains(&path) {
        return Some("generated_file");
    }
    if PROTECTED_GENERATED_PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix))
    {
        return Some("generated_file");
    }
    None
}

fn pre_tool_use_message(path: &str) -> String {
    if path == ".codex/memory/CLAUDE_MEMORY.md" {
        return format!(
            "[claude-pre-tool-use] blocked direct edits to imported Claude projection {path}; edit the memory source files or rerun the projection refresh instead."
        );
    }
    format!(
        "[claude-pre-tool-use] blocked direct edits to generated host surface {path}; edit scripts/materialize_cli_host_entrypoints.py and regenerate outputs instead."
    )
}

fn bash_generated_write_target(payload: &Value) -> Option<String> {
    let tool_name = payload.get("tool_name").and_then(Value::as_str)?;
    if tool_name != "Bash" {
        return None;
    }
    let command = payload
        .get("tool_input")
        .and_then(Value::as_object)
        .and_then(|tool_input| tool_input.get("command"))
        .or_else(|| payload.get("command"))
        .and_then(Value::as_str)?;
    for segment in split_bash_segments(command) {
        let looks_mutating = bash_command_looks_mutating(&segment);
        for hint in PROTECTED_BASH_PATH_HINTS {
            if !segment.contains(hint) {
                continue;
            }
            if looks_mutating || bash_segment_redirects_to_hint(&segment, hint) {
                return Some(if hint == ".claude/" {
                    ".claude/**".to_string()
                } else {
                    hint.to_string()
                });
            }
        }
    }
    None
}

fn split_bash_segments(command: &str) -> Vec<String> {
    Regex::new(r"\s*(?:&&|\|\||;|\|)\s*")
        .ok()
        .map(|regex| {
            regex
                .split(command)
                .filter_map(|segment| {
                    let trimmed = segment.trim();
                    (!trimmed.is_empty()).then(|| trimmed.to_string())
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![command.trim().to_string()])
}

fn bash_command_looks_mutating(command: &str) -> bool {
    [
        r"^\s*(mv|cp|install|touch|rm|unlink|truncate)\b",
        r"^\s*ln\b[^\n]*\s-[^\n]*[fs][^\n]*\b",
        r"^\s*git\s+(checkout\s+--|restore\b)",
        r"\bsed\s+-i\b",
        r"\bperl\s+-pi\b",
        r"\bpython3?\s+-c\b",
        r"\bnode\s+-e\b",
        r"\bruby\s+-e\b",
        r"\btee\b",
        r"\bdd\b",
    ]
    .iter()
    .any(|pattern| {
        Regex::new(pattern)
            .ok()
            .map(|regex| regex.is_match(command))
            .unwrap_or(false)
    })
}

fn bash_segment_redirects_to_hint(segment: &str, hint: &str) -> bool {
    let escaped = regex::escape(hint);
    [
        format!(r#"(>>?|>\|)\s*['"]?[^'"\n;&|]*{escaped}[^'"\n;&|]*['"]?"#),
        format!(r#"\btee\b(?:\s+-a)?\s+['"]?[^'"\n;&|]*{escaped}[^'"\n;&|]*['"]?"#),
        format!(r#"\bdd\b[^\n;&|]*\bof=['"]?[^'"\n;&|]*{escaped}[^'"\n;&|]*['"]?"#),
    ]
    .iter()
    .any(|pattern| {
        Regex::new(pattern)
            .ok()
            .map(|regex| regex.is_match(segment))
            .unwrap_or(false)
    })
}

fn open_memory_store(db_path: &Path) -> Result<Connection, String> {
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create sqlite parent failed: {err}"))?;
    }
    let conn =
        Connection::open(db_path).map_err(|err| format!("open sqlite store failed: {err}"))?;
    conn.execute_batch(
        "
        PRAGMA foreign_keys = ON;
        PRAGMA busy_timeout = 5000;
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        CREATE TABLE IF NOT EXISTS schema_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS memory_items (
            item_id TEXT PRIMARY KEY,
            workspace TEXT NOT NULL,
            category TEXT NOT NULL,
            source TEXT NOT NULL,
            confidence REAL NOT NULL DEFAULT 0.5,
            status TEXT NOT NULL DEFAULT 'active',
            summary TEXT NOT NULL,
            notes TEXT NOT NULL DEFAULT '',
            evidence_json TEXT NOT NULL DEFAULT '[]',
            metadata_json TEXT NOT NULL DEFAULT '{}',
            keywords_json TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_memory_items_workspace_updated
        ON memory_items(workspace, updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_memory_items_workspace_category_status
        ON memory_items(workspace, category, status, updated_at DESC);
        CREATE TABLE IF NOT EXISTS session_notes (
            note_id INTEGER PRIMARY KEY AUTOINCREMENT,
            workspace TEXT NOT NULL,
            session_key TEXT NOT NULL,
            position INTEGER NOT NULL,
            note TEXT NOT NULL,
            note_type TEXT NOT NULL DEFAULT 'append',
            metadata_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE (workspace, session_key, position)
        );
        CREATE INDEX IF NOT EXISTS idx_session_notes_workspace_session_position
        ON session_notes(workspace, session_key, position);
        CREATE TABLE IF NOT EXISTS evidence_records (
            evidence_id INTEGER PRIMARY KEY AUTOINCREMENT,
            workspace TEXT NOT NULL,
            kind TEXT NOT NULL,
            path TEXT NOT NULL,
            content TEXT NOT NULL DEFAULT '',
            artifact_id TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_evidence_records_workspace_updated
        ON evidence_records(workspace, updated_at DESC);
        ",
    )
    .map_err(|err| format!("ensure memory schema failed: {err}"))?;
    conn.execute(
        "INSERT INTO schema_meta(key, value, updated_at) VALUES ('schema_version', ?, ?) ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=excluded.updated_at",
        params![MEMORY_STORE_SCHEMA_VERSION, current_local_timestamp()],
    )
    .map_err(|err| format!("update schema version failed: {err}"))?;
    Ok(conn)
}

fn export_rows(conn: &Connection, query: &str, params_list: &[&str]) -> Result<Vec<Value>, String> {
    let mut statement = conn
        .prepare(query)
        .map_err(|err| format!("prepare sqlite export failed: {err}"))?;
    let rows = statement
        .query_map(rusqlite::params_from_iter(params_list.iter()), |row| {
            row_to_json(row)
        })
        .map_err(|err| format!("query sqlite export failed: {err}"))?;
    let mut values = Vec::new();
    for row in rows {
        values.push(Value::Object(
            row.map_err(|err| format!("read sqlite row failed: {err}"))?,
        ));
    }
    Ok(values)
}

fn row_to_json(row: &rusqlite::Row<'_>) -> rusqlite::Result<Map<String, Value>> {
    let row_ref = row.as_ref();
    let mut map = Map::new();
    for index in 0..row_ref.column_count() {
        let name = row_ref.column_name(index)?.to_string();
        let value = match row.get_ref(index)? {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(value) => Value::from(value),
            ValueRef::Real(value) => Value::from(value),
            ValueRef::Text(value) => Value::String(String::from_utf8_lossy(value).to_string()),
            ValueRef::Blob(value) => Value::String(String::from_utf8_lossy(value).to_string()),
        };
        map.insert(name, value);
    }
    Ok(map)
}

fn delete_memory_items_not_in_sources(
    conn: &Connection,
    workspace: &str,
    sources: &[String],
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM memory_items WHERE workspace = ? AND source NOT IN (?, ?, ?, ?, ?)",
        params![
            workspace,
            sources[0].as_str(),
            sources[1].as_str(),
            sources[2].as_str(),
            sources[3].as_str(),
            sources[4].as_str()
        ],
    )
    .map_err(|err| format!("delete memory items outside sources failed: {err}"))?;
    Ok(())
}

fn delete_memory_items_by_sources(
    conn: &Connection,
    workspace: &str,
    sources: &[String],
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM memory_items WHERE workspace = ? AND source IN (?, ?, ?, ?, ?)",
        params![
            workspace,
            sources[0].as_str(),
            sources[1].as_str(),
            sources[2].as_str(),
            sources[3].as_str(),
            sources[4].as_str()
        ],
    )
    .map_err(|err| format!("delete authoritative memory items before resync failed: {err}"))?;
    Ok(())
}

fn move_to_archive(source: &Path, destination: &Path) -> Result<String, String> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create archive parent failed: {err}"))?;
    }
    let target = if destination.exists() {
        let suffix = current_local_timestamp().replace(':', "").replace('+', "_");
        let stem = destination
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("archive");
        let ext = destination
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| format!(".{value}"))
            .unwrap_or_default();
        destination.with_file_name(format!("{stem}-{suffix}{ext}"))
    } else {
        destination.to_path_buf()
    };
    fs::rename(source, &target)
        .map_err(|err| format!("move {} failed: {err}", source.display()))?;
    Ok(target.display().to_string())
}

fn read_text_if_exists(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

fn read_json_if_exists(path: &Path) -> Value {
    let text = read_text_if_exists(path);
    serde_json::from_str(&text).unwrap_or(Value::Object(Map::new()))
}

fn write_text_if_changed(path: &Path, content: &str) -> Result<bool, String> {
    let existing = read_text_if_exists(path);
    if existing == content {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create parent directory failed: {err}"))?;
    }
    fs::write(path, content).map_err(|err| format!("write {} failed: {err}", path.display()))?;
    Ok(true)
}

fn write_json_if_changed(path: &Path, payload: &Value) -> Result<bool, String> {
    let text = serde_json::to_string_pretty(payload)
        .map_err(|err| format!("serialize {} failed: {err}", path.display()))?;
    write_text_if_changed(path, &(text + "\n"))
}

fn value_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.trim().to_string(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn is_terminal_token(value: Option<&Value>, terminal_values: &[&str]) -> bool {
    let token = value_text(value).to_lowercase();
    !token.is_empty() && terminal_values.contains(&token.as_str())
}

fn current_local_timestamp() -> String {
    Local::now().to_rfc3339()
}

fn current_local_date() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

fn safe_slug(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for ch in value.chars() {
        let normalized = ch.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() {
            slug.push(normalized);
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

fn memory_category_for_file(file_name: &str) -> &'static str {
    match file_name {
        "MEMORY.md" => "invariant",
        "preferences.md" => "preference",
        "decisions.md" => "decision",
        "lessons.md" => "lesson",
        "runbooks.md" => "runbook",
        _ => "general",
    }
}

fn memory_item_id(
    workspace: &str,
    category: &str,
    index: usize,
    summary: &str,
    fallback: &str,
) -> String {
    let summary_slug = {
        let slug = safe_slug(&summary.chars().take(80).collect::<String>());
        if slug.is_empty() {
            safe_slug(fallback)
        } else {
            slug
        }
    };
    format!("{}:{category}:{index}:{summary_slug}", safe_slug(workspace))
}

fn extract_memory_segments(raw: &str) -> Vec<(Vec<String>, String)> {
    let mut segments = Vec::new();
    let mut heading_stack: Vec<String> = Vec::new();
    let mut paragraph: Vec<String> = Vec::new();

    let flush_paragraph = |segments: &mut Vec<(Vec<String>, String)>,
                           heading_stack: &Vec<String>,
                           paragraph: &mut Vec<String>| {
        if paragraph.is_empty() {
            return;
        }
        let body = paragraph
            .iter()
            .map(|part| part.trim())
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        paragraph.clear();
        if body.is_empty() || (body.starts_with('_') && body.ends_with('_')) {
            return;
        }
        segments.push((heading_stack.clone(), body));
    };

    for raw_line in raw.lines() {
        let stripped = raw_line.trim();
        if stripped.is_empty() {
            flush_paragraph(&mut segments, &heading_stack, &mut paragraph);
            continue;
        }
        if let Some(captures) = Regex::new(r"^(#{1,6})\s+(.*)$")
            .ok()
            .and_then(|regex| regex.captures(stripped))
        {
            flush_paragraph(&mut segments, &heading_stack, &mut paragraph);
            let level = captures
                .get(1)
                .map(|value| value.as_str().len())
                .unwrap_or(1);
            let title = captures
                .get(2)
                .map(|value| value.as_str().trim().to_string())
                .unwrap_or_default();
            if level == 1 {
                heading_stack.clear();
                continue;
            }
            let depth = level.saturating_sub(2);
            heading_stack.truncate(depth);
            heading_stack.push(title);
            continue;
        }
        if let Some(captures) = Regex::new(r"^(?:[-*]|\d+[.)])\s+(.*)$")
            .ok()
            .and_then(|regex| regex.captures(stripped))
        {
            flush_paragraph(&mut segments, &heading_stack, &mut paragraph);
            let body = captures
                .get(1)
                .map(|value| value.as_str().trim().to_string())
                .unwrap_or_default();
            if !body.is_empty() {
                segments.push((heading_stack.clone(), body));
            }
            continue;
        }
        paragraph.push(stripped.to_string());
    }
    flush_paragraph(&mut segments, &heading_stack, &mut paragraph);
    segments
}

trait StringFallback {
    fn if_empty_then(self, fallback: String) -> String;
}

impl StringFallback for String {
    fn if_empty_then(self, fallback: String) -> String {
        if self.trim().is_empty() {
            fallback
        } else {
            self
        }
    }
}

#[cfg(test)]
pub(crate) fn temp_repo_root(label: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!(
        "router-rs-claude-hooks-{label}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos()
    ));
    fs::create_dir_all(base.join("artifacts/current/task-1")).expect("create temp repo");
    fs::create_dir_all(base.join(".codex/memory")).expect("create memory dir");
    fs::write(
        base.join(".codex/memory/MEMORY.md"),
        "# 项目长期记忆\n\n## Active Patterns\n\n- AP-1: Sync skills after skill edits\n\n## 稳定决策\n\n- SD-1: Shared CLI memory root lives under `./.codex/memory/`\n\n## Lessons\n\n- L-1: Do not let generated host files drift from runtime truth\n",
    )
    .expect("write shared memory");
    fs::write(
        base.join(".supervisor_state.json"),
        serde_json::to_string_pretty(&json!({
            "task_id": "task-1",
            "task_summary": "repair claude hook",
            "active_phase": "implementing",
            "verification": {"verification_status": "in_progress"},
            "continuity": {"story_state": "active", "resume_allowed": true},
            "next_actions": ["finish rust hook"],
            "execution_contract": {"scope": ["hooks"], "acceptance_criteria": ["smoke passes"]},
            "blockers": {"open_blockers": []},
            "controller": {"primary_owner": "claude-hook", "gate": "none"}
        }))
        .expect("serialize state"),
    )
    .expect("write state");
    fs::write(
        base.join("artifacts/current/active_task.json"),
        serde_json::to_string_pretty(&json!({"task_id": "task-1"})).expect("serialize pointer"),
    )
    .expect("write pointer");
    fs::write(
        base.join("artifacts/current/task-1/SESSION_SUMMARY.md"),
        "- task: repair claude hook\n- phase: implementing\n- status: in_progress\n",
    )
    .expect("write session summary");
    fs::write(
        base.join("artifacts/current/task-1/NEXT_ACTIONS.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": "next-actions-v2",
            "next_actions": ["finish rust hook"]
        }))
        .expect("serialize next actions"),
    )
    .expect("write next actions");
    fs::write(
        base.join("artifacts/current/task-1/EVIDENCE_INDEX.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": "evidence-index-v2",
            "artifacts": []
        }))
        .expect("serialize evidence"),
    )
    .expect("write evidence");
    fs::write(
        base.join("artifacts/current/task-1/TRACE_METADATA.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": "trace-metadata-v2",
            "task": "repair claude hook",
            "matched_skills": ["claude-hooks"]
        }))
        .expect("serialize trace"),
    )
    .expect("write trace");
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_project_settings_fall_back_to_framework_runtime_registry_defaults() {
        let repo_root = temp_repo_root("claude-project-settings-default-mcp");
        let settings = build_claude_project_settings(&repo_root);
        assert_eq!(
            settings.get("allowedMcpServers").and_then(Value::as_array),
            Some(&vec![
                json!({"serverName": "browser-mcp"}),
                json!({"serverName": "framework-mcp"}),
                json!({"serverName": "openaiDeveloperDocs"}),
            ])
        );
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn session_stop_writes_projection() {
        let repo_root = temp_repo_root("projection");
        let response = run_claude_lifecycle_hook("session-stop", &repo_root, 6).expect("hook ok");
        assert_eq!(
            response["canonical_command"],
            Value::String("session-stop".to_string())
        );
        assert_eq!(
            response["projection"]["target_path"],
            Value::String(repo_root.join(CLAUDE_MEMORY_PATH).display().to_string())
        );
        let projection =
            fs::read_to_string(repo_root.join(CLAUDE_MEMORY_PATH)).expect("projection");
        assert!(projection.contains("Claude Startup Projection"));
        assert!(projection.contains("## Startup Rules"));
        assert!(projection.contains("默认用中文；先给答案；默认只回一小段。"));
        assert!(projection.contains("## Task Snapshot"));
        assert!(projection.contains("repair claude hook"));
        assert!(projection.contains("AP-1: Sync skills after skill edits"));
        assert!(projection.contains("SD-1: Shared CLI memory root lives under `./.codex/memory/`"));
        assert!(projection.contains("artifacts/current/active_task.json"));
        assert!(!projection.contains("OpenAI/GPT"));
        assert!(projection.lines().count() <= 24);
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn refresh_projection_writes_projection() {
        let repo_root = temp_repo_root("refresh-projection");
        let response =
            run_claude_lifecycle_hook("refresh-projection", &repo_root, 6).expect("hook ok");
        assert_eq!(
            response["canonical_command"],
            Value::String("refresh-projection".to_string())
        );
        assert_eq!(
            response["contract"]["summary"],
            Value::String(
                "Refresh the imported Claude projection without touching shared continuity artifacts."
                    .to_string()
            )
        );
        assert_eq!(
            response["projection"]["target_path"],
            Value::String(repo_root.join(CLAUDE_MEMORY_PATH).display().to_string())
        );
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn legacy_lifecycle_aliases_are_rejected() {
        let repo_root = temp_repo_root("legacy-aliases");
        for alias in ["sync", "start-session", "stop-session", "end-session"] {
            let error =
                run_claude_lifecycle_hook(alias, &repo_root, 6).expect_err("alias should fail");
            assert!(
                error.contains(&format!("Unsupported Claude lifecycle command: {alias}")),
                "unexpected error for {alias}: {error}"
            );
        }
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn session_end_repairs_terminal_resume_allowed_before_consolidation() {
        let repo_root = temp_repo_root("session-end");
        fs::write(
            repo_root.join(".supervisor_state.json"),
            serde_json::to_string_pretty(&json!({
                "task_id": "task-1",
                "task_summary": "repair claude hook",
                "active_phase": "finalized",
                "verification": {"verification_status": "completed"},
                "continuity": {"story_state": "completed", "resume_allowed": true},
                "next_actions": ["finish rust hook"],
                "execution_contract": {"scope": ["hooks"], "acceptance_criteria": ["smoke passes"]},
                "blockers": {"open_blockers": []}
            }))
            .expect("serialize state"),
        )
        .expect("write repaired state seed");

        let response = run_claude_lifecycle_hook("session-end", &repo_root, 6).expect("hook ok");

        assert_eq!(
            response["canonical_command"],
            Value::String("session-end".to_string())
        );
        assert!(response.get("consolidation").is_some());
        let repaired_state = read_json_if_exists(&repo_root.join(".supervisor_state.json"));
        assert_eq!(
            repaired_state["continuity"]["resume_allowed"],
            Value::Bool(false)
        );
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn session_end_skips_rewriting_unchanged_memory_state_and_bundle() {
        let repo_root = temp_repo_root("session-end-idempotent");

        let first = run_claude_lifecycle_hook("session-end", &repo_root, 6).expect("first hook ok");
        let first_state = read_json_if_exists(&repo_root.join(".codex/memory/state.json"));
        let first_timestamp = first_state
            .get("last_consolidated_at")
            .and_then(Value::as_str)
            .expect("first timestamp")
            .to_string();
        assert!(
            first["consolidation"]["sqlite_result"]["persisted_items"]
                .as_u64()
                .unwrap_or(0)
                > 0
        );

        let second =
            run_claude_lifecycle_hook("session-end", &repo_root, 6).expect("second hook ok");
        let second_state = read_json_if_exists(&repo_root.join(".codex/memory/state.json"));
        let second_timestamp = second_state
            .get("last_consolidated_at")
            .and_then(Value::as_str)
            .expect("second timestamp");
        assert_eq!(second_timestamp, first_timestamp);
        assert_eq!(
            second["consolidation"]["sqlite_result"]["persisted_items"],
            Value::from(0)
        );
        assert_eq!(
            second["consolidation"]["changed_files"],
            Value::Array(Vec::new())
        );

        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn config_change_audit_detects_generated_surfaces() {
        let repo_root = temp_repo_root("audit");
        let payload = json!({
            "source": "project_settings",
            "file_path": ".claude/settings.json"
        });
        let result = run_config_change(&repo_root, &payload).expect("audit ok");
        assert_eq!(
            result["command"],
            Value::String("config-change".to_string())
        );
        assert_eq!(
            result["scope"],
            Value::String("project_settings".to_string())
        );
        assert!(result["notices"]
            .as_array()
            .expect("notices")
            .iter()
            .any(|item| item
                .as_str()
                .unwrap_or("")
                .contains("generated Claude host surfaces")));
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn user_prompt_submit_emits_context_for_code_requests() {
        let repo_root = temp_repo_root("user-prompt-submit-code");
        let payload = json!({
            "hook_event_name": "UserPromptSubmit",
            "prompt": "继续优化 runtime，去掉补丁式保底并顺手看内存和速度"
        });
        let result = run_user_prompt_submit(&repo_root, &payload).expect("audit ok");
        assert_eq!(
            result["hookSpecificOutput"]["hookEventName"],
            Value::String("UserPromptSubmit".to_string())
        );
        let telemetry = result["contextTelemetry"].as_object().expect("telemetry");
        let lanes = telemetry["lanes"].as_array().expect("lanes");
        assert!(lanes
            .iter()
            .any(|item| item.as_str() == Some("memory-truth")));
        assert!(lanes
            .iter()
            .any(|item| item.as_str() == Some("continuity-truth")));
        assert!(lanes
            .iter()
            .any(|item| item.as_str() == Some("state-compact")));
        assert!(lanes.iter().any(|item| item.as_str() == Some("perf")));
        assert!(lanes.iter().any(|item| item.as_str() == Some("compat")));
        assert_eq!(
            telemetry["budget_chars"],
            Value::from(USER_PROMPT_CONTEXT_MAX_CHARS as u64)
        );
        assert_eq!(
            telemetry["state_budget_chars"],
            Value::from(USER_PROMPT_STATE_BUDGET_CHARS as u64)
        );
        assert_eq!(telemetry["trimmed"], Value::Bool(false));
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn user_prompt_submit_emits_repo_memory_for_non_code_wording() {
        let repo_root = temp_repo_root("user-prompt-submit-non-code");
        let payload = json!({
            "hook_event_name": "UserPromptSubmit",
            "prompt": "把这个结论改得更像人话一点，顺手润色一下措辞"
        });
        let result = run_user_prompt_submit(&repo_root, &payload).expect("audit ok");
        let telemetry = result["contextTelemetry"].as_object().expect("telemetry");
        let lanes = telemetry["lanes"].as_array().expect("lanes");
        assert!(lanes
            .iter()
            .any(|item| item.as_str() == Some("memory-truth")));
        assert!(lanes
            .iter()
            .any(|item| item.as_str() == Some("continuity-truth")));
        assert!(lanes
            .iter()
            .any(|item| item.as_str() == Some("state-compact")));
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn user_prompt_submit_keeps_doc_edits_on_repo_memory_without_extra_code_nudges() {
        let repo_root = temp_repo_root("user-prompt-submit-doc");
        for prompt in [
            "优化 .claude/hooks/README.md，把说明写得更清楚",
            "继续优化 AGENT.md，把 simplify 原则再收紧一点",
        ] {
            let payload = json!({
                "hook_event_name": "UserPromptSubmit",
                "prompt": prompt,
            });
            let result = run_user_prompt_submit(&repo_root, &payload).expect("audit ok");
            let context = result["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .unwrap_or("");
            assert!(
                context.contains("repo-local shared memory"),
                "missing repo memory for {prompt}"
            );
            assert!(
                context.contains("当前状态："),
                "missing compact state for {prompt}"
            );
            assert!(
                !context.contains("Task Snapshot"),
                "unexpected long state block for {prompt}"
            );
            assert!(
                !context.contains("实现要求"),
                "unexpected old code nudge for {prompt}"
            );
            assert!(
                !context.contains("收尾提醒："),
                "unexpected closeout reminder for {prompt}"
            );
            assert!(context.chars().count() <= USER_PROMPT_CONTEXT_MAX_CHARS);
        }
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn user_prompt_submit_compacts_completed_state_with_plain_next_step() {
        let repo_root = temp_repo_root("user-prompt-submit-completed-state");
        fs::write(
            repo_root.join(".supervisor_state.json"),
            serde_json::to_string_pretty(&json!({
                "task_id": "task-completed-1",
                "task_summary": "bounded rerun",
                "active_phase": "closeout",
                "verification": {"verification_status": "completed"},
                "continuity": {"story_state": "completed", "resume_allowed": false},
                "next_actions": []
            }))
            .expect("serialize state"),
        )
        .expect("write state");

        let payload = json!({
            "hook_event_name": "UserPromptSubmit",
            "prompt": "继续优化 AGENT.md 的收尾措辞"
        });
        let result = run_user_prompt_submit(&repo_root, &payload).expect("audit ok");
        let context = result["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap_or("");

        assert!(context.contains("当前状态：recent task: wrapped up"));
        assert!(!context.contains("no resumable active task"));
        assert!(!context.contains("resume: blocked"));
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn user_prompt_submit_uses_hook_path_mentions_to_raise_precision() {
        let repo_root = temp_repo_root("user-prompt-submit-hook");
        let payload = json!({
            "hook_event_name": "UserPromptSubmit",
            "prompt": "继续改 scripts/router-rs/src/claude_hooks.rs，把 hook 自动化做准一点，hook 触发要更窄"
        });
        let result = run_user_prompt_submit(&repo_root, &payload).expect("audit ok");
        let context = result["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap_or("");
        assert!(!context.is_empty());
        assert!(context.contains("Hook 额外检查"));
        assert!(!context.contains("实现要求"));
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn user_prompt_submit_expands_budget_for_complex_execution_alias_requests() {
        let repo_root = temp_repo_root("user-prompt-submit-complex-alias");
        let payload = json!({
            "hook_event_name": "UserPromptSubmit",
            "prompt": "继续 /autopilot 续跑当前任务，先核对恢复锚点，再处理 root cause，必要时按 /team 拆 worker"
        });
        let result = run_user_prompt_submit(&repo_root, &payload).expect("audit ok");
        let telemetry = result["contextTelemetry"].as_object().expect("telemetry");
        let lanes = telemetry["lanes"].as_array().expect("lanes");
        let context = result["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap_or("");
        assert!(lanes
            .iter()
            .any(|item| item.as_str() == Some("execution-intent")));
        assert_eq!(
            telemetry["budget_chars"],
            Value::from(USER_PROMPT_COMPLEX_CONTEXT_MAX_CHARS as u64)
        );
        assert_eq!(
            telemetry["state_budget_chars"],
            Value::from(USER_PROMPT_COMPLEX_STATE_BUDGET_CHARS as u64)
        );
        assert!(context.contains("执行意图："));
        assert!(context.contains("优先续跑当前执行链") || context.contains("先核对恢复锚点"));
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn claude_project_settings_fall_back_to_default_mcp_servers_when_repo_registry_is_missing() {
        let repo_root = temp_repo_root("claude-settings-mcp-missing");
        let settings = build_claude_project_settings(&repo_root);
        let servers = settings["allowedMcpServers"]
            .as_array()
            .expect("allowed mcp servers");
        let names = servers
            .iter()
            .filter_map(|row| row.get("serverName").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec!["browser-mcp", "framework-mcp", "openaiDeveloperDocs"]
        );
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn claude_project_settings_fall_back_to_default_mcp_servers_when_repo_registry_is_empty() {
        let repo_root = temp_repo_root("claude-settings-mcp-empty");
        let registry_path = repo_root.join("configs/framework/RUNTIME_REGISTRY.json");
        fs::create_dir_all(registry_path.parent().expect("registry parent"))
            .expect("create registry parent");
        fs::write(
            &registry_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": "framework-runtime-registry-v1",
                "shared_project_mcp_servers": []
            }))
            .expect("serialize registry"),
        )
        .expect("write registry");

        let settings = build_claude_project_settings(&repo_root);
        let servers = settings["allowedMcpServers"]
            .as_array()
            .expect("allowed mcp servers");
        let names = servers
            .iter()
            .filter_map(|row| row.get("serverName").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec!["browser-mcp", "framework-mcp", "openaiDeveloperDocs"]
        );
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn claude_project_settings_use_repo_registry_mcp_servers_when_nonempty() {
        let repo_root = temp_repo_root("claude-settings-mcp-repo");
        let registry_path = repo_root.join("configs/framework/RUNTIME_REGISTRY.json");
        fs::create_dir_all(registry_path.parent().expect("registry parent"))
            .expect("create registry parent");
        fs::write(
            &registry_path,
            serde_json::to_string_pretty(&json!({
                "schema_version": "framework-runtime-registry-v1",
                "shared_project_mcp_servers": ["framework-mcp"]
            }))
            .expect("serialize registry"),
        )
        .expect("write registry");

        let settings = build_claude_project_settings(&repo_root);
        let servers = settings["allowedMcpServers"]
            .as_array()
            .expect("allowed mcp servers");
        let names = servers
            .iter()
            .filter_map(|row| row.get("serverName").and_then(Value::as_str))
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["framework-mcp"]);
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn pre_tool_use_blocks_generated_host_surface_edits() {
        let repo_root = temp_repo_root("pre-tool");
        let payload = json!({
            "tool_name": "MultiEdit",
            "tool_input": {
                "file_path": ".claude/settings.json"
            }
        });
        let result = run_pre_tool_use(&repo_root, &payload).expect("guard ok");
        assert_eq!(result["decision"], Value::String("deny".to_string()));
        assert_eq!(
            result["path"],
            Value::String(".claude/settings.json".to_string())
        );
        assert_eq!(
            result["hookSpecificOutput"]["permissionDecision"],
            Value::String("deny".to_string())
        );
        assert!(result["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap_or("")
            .contains("generated host surface"));
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn pre_tool_use_allows_normal_workspace_edits() {
        let repo_root = temp_repo_root("pre-tool-allow");
        let payload = json!({
            "tool_name": "Edit",
            "tool_input": {
                "file_path": "notes/todo.md"
            }
        });
        let result = run_pre_tool_use(&repo_root, &payload).expect("guard ok");
        assert_eq!(result["decision"], Value::String("allow".to_string()));
        assert!(result.get("hookSpecificOutput").is_none());
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn pre_tool_use_blocks_targeted_bash_writes() {
        let repo_root = temp_repo_root("pre-tool-bash");
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {
                "command": "cp tmp .claude/settings.json"
            }
        });
        let result = run_pre_tool_use(&repo_root, &payload).expect("guard ok");
        assert_eq!(result["decision"], Value::String("deny".to_string()));
        assert_eq!(
            result["path"],
            Value::String(".claude/settings.json".to_string())
        );
        assert_eq!(
            result["hookSpecificOutput"]["permissionDecision"],
            Value::String("deny".to_string())
        );
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn pre_tool_use_blocks_shell_redirection_into_generated_files() {
        let repo_root = temp_repo_root("pre-tool-redirect");
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {
                "command": "printf '{}' > .claude/settings.json"
            }
        });
        let result = run_pre_tool_use(&repo_root, &payload).expect("guard ok");
        assert_eq!(result["decision"], Value::String("deny".to_string()));
        assert_eq!(
            result["path"],
            Value::String(".claude/settings.json".to_string())
        );
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn pre_tool_use_allows_read_only_generated_file_checks_after_unrelated_write() {
        let repo_root = temp_repo_root("pre-tool-bash-read");
        let payload = json!({
            "tool_name": "Bash",
            "tool_input": {
                "command": "cp tmp ./tmp.out && cat .claude/settings.json"
            }
        });
        let result = run_pre_tool_use(&repo_root, &payload).expect("guard ok");
        assert_eq!(result["decision"], Value::String("allow".to_string()));
        assert!(result.get("hookSpecificOutput").is_none());
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn codex_pre_tool_use_blocks_patch_artifact_writes() {
        let repo_root = temp_repo_root("codex-pre-tool-patch");
        let payload = json!({
            "tool_name": "Write",
            "tool_input": {
                "file_path": "tmp/fix.patch",
                "content": "diff --git a/x b/x\n"
            }
        });
        let result = run_codex_pre_tool_use(&repo_root, &payload).expect("codex hook ok");
        let payload = result.expect("codex block payload");
        assert_eq!(payload["decision"], Value::String("block".to_string()));
        assert_eq!(
            payload["hookSpecificOutput"]["permissionDecision"],
            Value::String("deny".to_string())
        );
        assert!(payload["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap_or("")
            .contains("patch artifact write"));
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn codex_pre_tool_use_blocks_patchy_runtime_edits() {
        let repo_root = temp_repo_root("codex-pre-tool-quality");
        let target = repo_root.join("scripts/router-rs/src/claude_hooks.rs");
        fs::create_dir_all(target.parent().expect("target parent")).expect("create target parent");
        fs::write(&target, "fn main() {}\n").expect("write target");

        let payload = json!({
            "tool_name": "Edit",
            "tool_input": {
                "file_path": target.display().to_string(),
                "new_string": "let a = foo.clone();\nlet b = bar.clone();\nlet c = baz.clone();\nlet g = serde_json::to_string(&x)?;\n// legacy fallback compatibility patch",
            }
        });
        let result = run_codex_pre_tool_use(&repo_root, &payload).expect("codex hook ok");
        let payload = result.expect("codex block payload");
        assert_eq!(payload["decision"], Value::String("block".to_string()));
        assert_eq!(
            payload["hookSpecificOutput"]["permissionDecision"],
            Value::String("deny".to_string())
        );
        assert!(payload["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap_or("")
            .contains("patchy Rust edit"));
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn codex_pre_tool_use_keeps_clean_runtime_edits_silent() {
        let repo_root = temp_repo_root("codex-pre-tool-clean");
        let target = repo_root.join("tests/test_cli_host_entrypoints.py");
        fs::create_dir_all(target.parent().expect("target parent")).expect("create target parent");
        fs::write(&target, "assert True\n").expect("write target");

        let payload = json!({
            "tool_name": "Edit",
            "tool_input": {
                "file_path": target.display().to_string(),
                "new_string": "assert render_statusline() == 'ok'\n",
            }
        });
        let result = run_codex_pre_tool_use(&repo_root, &payload).expect("codex hook ok");
        assert!(result.is_none());
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn pre_tool_use_quality_emits_context_for_runtime_targets() {
        let repo_root = temp_repo_root("pre-tool-quality");
        let target = repo_root.join("tests/test_cli_host_entrypoints.py");
        fs::create_dir_all(target.parent().expect("target parent")).expect("create target parent");
        fs::write(&target, "fn main() {}\n").expect("write target");

        let payload = json!({
            "tool_name": "Edit",
            "tool_input": {
                "file_path": target.display().to_string()
            }
        });
        let result = run_pre_tool_use_quality(&repo_root, &payload).expect("quality ok");
        assert_eq!(
            result["hookSpecificOutput"]["hookEventName"],
            Value::String("PreToolUse".to_string())
        );
        assert_eq!(
            result["hookSpecificOutput"]["permissionDecision"],
            Value::String("allow".to_string())
        );
        let context = result["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap_or("");
        assert!(context.contains("测试额外检查"));
        assert!(context.contains("补丁式旧行为"));
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn post_tool_audit_reports_patchy_rust_runtime_edits() {
        let repo_root = temp_repo_root("post-tool-rust");
        let target = repo_root.join("scripts/router-rs/src/claude_hooks.rs");
        fs::create_dir_all(target.parent().expect("target parent")).expect("create target parent");
        fs::write(&target, "fn main() {}\n").expect("write target");

        let payload = json!({
            "tool_name": "Edit",
            "tool_input": {
                "file_path": target.display().to_string(),
                "new_string": "let a = foo.clone();\nlet b = bar.clone();\nlet g = serde_json::to_string(&x)?;\n// legacy fallback compatibility patch",
            }
        });
        let result = run_post_tool_audit(&repo_root, &payload).expect("audit ok");
        assert_eq!(
            result["hookSpecificOutput"]["hookEventName"],
            Value::String("PostToolUse".to_string())
        );
        let context = result["additionalContext"].as_str().unwrap_or("");
        assert!(!context.is_empty());
        assert!(context.contains("增量来源=edit_new_string"));
        assert!(context.contains("clone="));
        assert!(
            context.contains("兼容")
                || context.contains("中转层")
                || context.contains("fallback")
                || context.contains("patch")
        );
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn post_tool_audit_uses_pre_tool_snapshot_for_true_delta_review() {
        let repo_root = temp_repo_root("post-tool-snapshot");
        let target = repo_root.join("tests/test_cli_host_entrypoints.py");
        fs::create_dir_all(target.parent().expect("target parent")).expect("create target parent");
        fs::write(
            &target,
            "legacy fallback compatibility patch\njson.dumps(x)\njson.loads(y)\njson.dumps(z)\njson.loads(w)\n",
        )
        .expect("write target");

        let pre_payload = json!({
            "tool_name": "Edit",
            "tool_input": {
                "file_path": target.display().to_string()
            }
        });
        let pre_result = run_pre_tool_use_quality(&repo_root, &pre_payload).expect("pre hook ok");
        assert_eq!(
            pre_result["hookSpecificOutput"]["permissionDecision"],
            Value::String("allow".to_string())
        );

        fs::write(
            &target,
            format!(
                "{}helper = build_context(payload)\n",
                fs::read_to_string(&target).expect("read target")
            ),
        )
        .expect("append target");

        let post_payload = json!({
            "tool_name": "Edit",
            "tool_input": {
                "file_path": target.display().to_string()
            }
        });
        let result = run_post_tool_audit(&repo_root, &post_payload).expect("audit ok");
        assert!(result.get("hookSpecificOutput").is_none());
        fs::remove_dir_all(repo_root).expect("cleanup repo");
    }

    #[test]
    fn stop_failure_audit_prefers_official_error_field() {
        let payload = json!({
            "error": "rate_limit",
            "error_details": "too many requests"
        });
        let result = run_stop_failure(Path::new("."), &payload).expect("audit ok");
        assert_eq!(
            result["failure_type"],
            Value::String("rate_limit".to_string())
        );
    }
}
