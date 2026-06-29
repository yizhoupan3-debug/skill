//! Team orchestration: multi-agent team lifecycle, membership, and inter-agent message passing.
//!
//! Replaces the deprecated "workflow" (JS) model with a structured team-manager approach
//! where agents are team members with tracked lifecycle, shared artifact state, and
//! filesystem-based message passing.
//!
//! ## Concurrency
//!
//! All team-registry mutations use `with_team_registry`, which acquires a cross-process
//! `flock` before loading, passes an `&mut TeamRegistry` to the closure, then saves
//! under the same lock.  This eliminates the TOCTOU window between load and save.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

use core_errors::FrameworkError;

use crate::process::{register_agent_alive, unregister_agent};

pub const TEAM_SCHEMA_VERSION: &str = "v1";
pub const TEAM_ARTIFACTS_DIR: &str = "artifacts/teams";

// ── Path sanitization ──────────────────────────────────────────

/// Core sanitization with configurable dot allowance.
fn sanitize_path_segment_inner(raw: &str, allow_dots: bool) -> Result<String, FrameworkError> {
    let sanitized: String = raw
        .chars()
        .filter(|&c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || (allow_dots && c == '.'))
        .collect();
    let trimmed = if allow_dots {
        sanitized.trim_matches('.').to_string()
    } else {
        sanitized
    };
    if trimmed.is_empty() {
        return Err(FrameworkError::validation(format!(
            "invalid (empty after sanitization): {raw:?}"
        )));
    }
    Ok(trimmed)
}

/// Sanitize a user-supplied team_id for use as a single path segment.
/// Strips everything except alphanumeric, `-`, `_`, `.`.
pub fn sanitize_path_segment(raw: &str) -> Result<String, FrameworkError> {
    sanitize_path_segment_inner(raw, true)
}

/// Sanitize an agent_id or to_agent for use in a file name.
/// No dots (prevents `..` traversal), only alphanumeric + `-` + `_`.
pub fn sanitize_segment_strict(raw: &str) -> Result<String, FrameworkError> {
    sanitize_path_segment_inner(raw, false)
}

// ── Data structures ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMember {
    pub agent_id: String,
    pub role: String,
    pub host_id: String,
    pub status: String,
    pub joined_at: String,
    pub completed_at: Option<String>,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub task_contract: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamDescriptor {
    pub team_id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub status: String,
    pub members: Vec<TeamMember>,
    pub supervisor_agent_id: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamRegistry {
    pub schema_version: String,
    pub teams: Vec<TeamDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterAgentMessage {
    pub message_id: String,
    pub from_agent: String,
    pub to_agent: Option<String>,
    pub team_id: String,
    pub kind: String,
    pub payload: Value,
    pub sent_at: String,
    pub read: bool,
}

// ── Internal I/O (no lock — caller must hold a with_team_registry guard) ─

fn team_registry_path(repo_root: &Path) -> PathBuf {
    repo_root.join(TEAM_ARTIFACTS_DIR).join("registry.json")
}

fn team_dir_safe(repo_root: &Path, safe_team: &str) -> PathBuf {
    repo_root.join(TEAM_ARTIFACTS_DIR).join(safe_team)
}

fn team_messages_dir_safe(repo_root: &Path, safe_team: &str) -> PathBuf {
    team_dir_safe(repo_root, safe_team).join("messages")
}

fn load_team_registry_raw(path: &Path) -> Result<TeamRegistry, FrameworkError> {
    if !path.is_file() {
        return Ok(TeamRegistry {
            schema_version: TEAM_SCHEMA_VERSION.to_string(),
            teams: Vec::new(),
        });
    }
    let raw = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

fn save_team_registry_raw(path: &Path, registry: &TeamRegistry) -> Result<(), FrameworkError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension("json.tmp");
    let payload = serde_json::to_string_pretty(registry)?;
    fs::write(&tmp_path, &payload)?;
    fs::rename(&tmp_path, path)?;
    core_state_utils::atomic_write::fsync_parent_dir(path)
        .unwrap_or_else(|e| tracing::warn!("fsync team registry parent dir failed: {e}"));
    Ok(())
}

/// Execute `f` with an exclusive lock on the team registry.
/// Lock is held for the entire load → modify → save cycle.
fn with_team_registry<F, T>(repo_root: &Path, f: F) -> Result<T, FrameworkError>
where
    F: FnOnce(&mut TeamRegistry) -> Result<T, FrameworkError>,
{
    let path = team_registry_path(repo_root);
    let _lock = rt_storage::runtime_storage::acquire_runtime_path_lock(&path)
        .map_err(|e| FrameworkError::lock(e))?;
    let mut registry = load_team_registry_raw(&path)?;
    let result = f(&mut registry)?;
    save_team_registry_raw(&path, &registry)?;
    Ok(result)
}

/// Read-only access: acquires the lock so the caller sees a consistent snapshot.
/// The registry is a borrow, so the lock lives for the duration of the call.
pub fn with_team_registry_ro<F, T>(repo_root: &Path, f: F) -> Result<T, FrameworkError>
where
    F: FnOnce(&TeamRegistry) -> Result<T, FrameworkError>,
{
    let path = team_registry_path(repo_root);
    let _lock = rt_storage::runtime_storage::acquire_runtime_path_lock(&path)
        .map_err(|e| FrameworkError::lock(e))?;
    let registry = load_team_registry_raw(&path)?;
    f(&registry)
}

// ── Team lifecycle ────────────────────────────────────────────────

/// Create a new team with the given ID and name.
pub fn create_team(
    repo_root: &Path,
    team_id: &str,
    name: &str,
    supervisor_agent_id: Option<&str>,
    now: &str,
) -> Result<TeamDescriptor, FrameworkError> {
    let safe_team = sanitize_path_segment(team_id)?;

    with_team_registry(repo_root, |registry| {
        if registry.teams.iter().any(|t| t.team_id == team_id) {
            return Err(FrameworkError::registry(format!(
                "team already exists: {team_id}"
            )));
        }

        let member_meta_path = team_dir_safe(repo_root, &safe_team).join("members");
        fs::create_dir_all(&member_meta_path)?;
        fs::create_dir_all(team_messages_dir_safe(repo_root, &safe_team))?;

        let team = TeamDescriptor {
            team_id: team_id.to_string(),
            name: name.to_string(),
            created_at: now.to_string(),
            updated_at: now.to_string(),
            status: "active".to_string(),
            members: Vec::new(),
            supervisor_agent_id: supervisor_agent_id.map(String::from),
            metadata: serde_json::json!({
                "phase": "created",
                "created_by": supervisor_agent_id.unwrap_or("system"),
            }),
        };

        // Save team.json metadata (outside the main registry but under lock)
        let meta_path = team_dir_safe(repo_root, &safe_team).join("team.json");
        let raw = serde_json::to_string_pretty(&team)?;
        fs::write(&meta_path, &raw)?;

        registry.teams.push(team.clone());
        Ok(team)
    })
}

fn find_team_mut<'a>(
    registry: &'a mut TeamRegistry,
    team_id: &str,
) -> Option<&'a mut TeamDescriptor> {
    registry.teams.iter_mut().find(|t| t.team_id == team_id)
}

/// Add a member (agent) to an existing team.
pub fn add_team_member(
    repo_root: &Path,
    team_id: &str,
    agent_id: &str,
    role: &str,
    host_id: &str,
    now: &str,
) -> Result<TeamMember, FrameworkError> {
    let safe_team = sanitize_path_segment(team_id)?;
    let safe_agent = sanitize_segment_strict(agent_id)?;

    with_team_registry(repo_root, |registry| {
        let team = find_team_mut(registry, team_id)
            .ok_or_else(|| FrameworkError::not_found(format!("team not found: {team_id}")))?;

        if team.members.iter().any(|m| m.agent_id == agent_id) {
            return Err(FrameworkError::registry(format!(
                "agent {agent_id} already in team {team_id}"
            )));
        }

        // Register in agent health (under the team lock)
        register_agent_alive(repo_root, agent_id, host_id, "team_member", now)?;

        let member = TeamMember {
            agent_id: agent_id.to_string(),
            role: role.to_string(),
            host_id: host_id.to_string(),
            status: "running".to_string(),
            joined_at: now.to_string(),
            completed_at: None,
            messages_sent: 0,
            messages_received: 0,
            task_contract: serde_json::json!({
                "team_id": team_id,
                "role": role,
                "joined_at": now,
            }),
        };

        // Write individual member file
        let member_path = team_dir_safe(repo_root, &safe_team)
            .join("members")
            .join(format!("{safe_agent}.json"));
        let raw = serde_json::to_string_pretty(&member)?;
        fs::write(&member_path, &raw)?;

        team.members.push(member.clone());
        team.updated_at = now.to_string();
        Ok(member)
    })
}

/// Remove a member from a team.
pub fn remove_team_member(
    repo_root: &Path,
    team_id: &str,
    agent_id: &str,
    terminal_status: &str,
    error: Option<&str>,
    now: &str,
) -> Result<(), FrameworkError> {
    with_team_registry(repo_root, |registry| {
        let team = find_team_mut(registry, team_id)
            .ok_or_else(|| FrameworkError::not_found(format!("team not found: {team_id}")))?;

        unregister_agent(repo_root, agent_id, terminal_status, error, now)?;

        if let Some(member) = team.members.iter_mut().find(|m| m.agent_id == agent_id) {
            member.status = terminal_status.to_string();
            member.completed_at = Some(now.to_string());
        }

        team.updated_at = now.to_string();
        Ok(())
    })
}

/// Mark a team as completed.
pub fn complete_team(
    repo_root: &Path,
    team_id: &str,
    now: &str,
) -> Result<TeamDescriptor, FrameworkError> {
    let safe_team = sanitize_path_segment(team_id)?;

    with_team_registry(repo_root, |registry| {
        let team = find_team_mut(registry, team_id)
            .ok_or_else(|| FrameworkError::not_found(format!("team not found: {team_id}")))?;

        // Re-entrancy guard: skip if already completed
        if team.status == "completed" {
            return Ok(team.clone());
        }

        // Terminate all running members
        let running_ids: Vec<String> = team
            .members
            .iter()
            .filter(|m| m.status == "running")
            .map(|m| m.agent_id.clone())
            .collect();

        for agent_id in running_ids {
            if let Some(member) = team.members.iter_mut().find(|m| m.agent_id == agent_id) {
                member.status = "interrupted".to_string();
                member.completed_at = Some(now.to_string());
            }
            unregister_agent(repo_root, &agent_id, "interrupted", None, now)?;
        }

        team.status = "completed".to_string();
        team.updated_at = now.to_string();

        // Update team.json metadata
        let meta_path = team_dir_safe(repo_root, &safe_team).join("team.json");
        let raw = serde_json::to_string_pretty(&team)?;
        fs::write(&meta_path, &raw)?;

        Ok(team.clone())
    })
}

// ── Inter-agent message passing ──────────────────────────────────

/// Send a message from one agent to another (or broadcast).
pub fn send_message(
    repo_root: &Path,
    team_id: &str,
    from_agent: &str,
    to_agent: Option<&str>,
    kind: &str,
    payload: Value,
    now: &str,
) -> Result<InterAgentMessage, FrameworkError> {
    let safe_team = sanitize_path_segment(team_id)?;
    let safe_from = sanitize_segment_strict(from_agent)?;
    let safe_target = to_agent
        .map(sanitize_segment_strict)
        .transpose()?
        .unwrap_or_else(|| "broadcast".to_string());

    // Validate team, update counters, AND write message file under the same
    // lock so the entire validation-and-write sequence is atomic.
    let msg_id = format!("{now}-{safe_from}");
    let safe_msg_id: String = msg_id
        .chars()
        .filter(|&c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        .collect();

    let msg = InterAgentMessage {
        message_id: safe_msg_id.clone(),
        from_agent: from_agent.to_string(),
        to_agent: to_agent.map(String::from),
        team_id: team_id.to_string(),
        kind: kind.to_string(),
        payload,
        sent_at: now.to_string(),
        read: false,
    };

    let msg_path = team_messages_dir_safe(repo_root, &safe_team)
        .join(&safe_target)
        .join(format!("{safe_msg_id}.json"));

    with_team_registry(repo_root, |registry| {
        let team = find_team_mut(registry, team_id)
            .ok_or_else(|| FrameworkError::not_found(format!("team not found: {team_id}")))?;

        if let Some(sender) = team.members.iter_mut().find(|m| m.agent_id == from_agent) {
            sender.messages_sent += 1;
        }
        if let Some(recipient) =
            to_agent.and_then(|id| team.members.iter_mut().find(|m| m.agent_id == id))
        {
            recipient.messages_received += 1;
        }
        team.updated_at = now.to_string();

        // Write message file inside the lock
        if let Some(parent) = msg_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let raw = serde_json::to_string_pretty(&msg)?;
        fs::write(&msg_path, &raw)?;

        Ok(msg)
    })
}

/// Read all messages for a given agent in a team.
/// After reading, marks messages as read by rewriting the file with `read: true`.
pub fn read_my_messages(
    repo_root: &Path,
    team_id: &str,
    agent_id: &str,
) -> Result<Vec<InterAgentMessage>, FrameworkError> {
    // Validate team exists first (under read lock)
    let safe_team = sanitize_path_segment(team_id)?;
    let safe_agent = sanitize_segment_strict(agent_id)?;

    with_team_registry_ro(repo_root, |registry| {
        if registry.teams.iter().all(|t| t.team_id != team_id) {
            return Err(FrameworkError::not_found(format!(
                "team not found: {team_id}"
            )));
        }
        Ok(())
    })?;

    let inbox = team_messages_dir_safe(repo_root, &safe_team).join(&safe_agent);
    let broadcast_dir = team_messages_dir_safe(repo_root, &safe_team).join("broadcast");

    let mut messages = Vec::new();

    // Read agent's own inbox
    if inbox.is_dir() {
        let mut entries: Vec<_> = fs::read_dir(&inbox)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
            .collect();
        entries.sort_by_key(|e| e.path().file_name().map(|n| n.to_os_string()));

        for entry in &entries {
            let raw = fs::read_to_string(entry.path())?;
            if let Ok(mut msg) = serde_json::from_str::<InterAgentMessage>(&raw) {
                // Mark as read
                if !msg.read {
                    msg.read = true;
                    if let Ok(updated) = serde_json::to_string_pretty(&msg) {
                        let _ = fs::write(entry.path(), &updated);
                    }
                }
                messages.push(msg);
            }
        }
    }

    // Read broadcast messages
    if broadcast_dir.is_dir() && broadcast_dir != inbox {
        let entries: Vec<_> = fs::read_dir(&broadcast_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
            .collect();

        for entry in entries {
            let raw = fs::read_to_string(entry.path())?;
            if let Ok(mut msg) = serde_json::from_str::<InterAgentMessage>(&raw)
                && !messages.iter().any(|m| m.message_id == msg.message_id)
            {
                // Mark broadcast as read too
                if !msg.read {
                    msg.read = true;
                    if let Ok(updated) = serde_json::to_string_pretty(&msg) {
                        let _ = fs::write(entry.path(), &updated);
                    }
                }
                messages.push(msg);
            }
        }
    }

    messages.sort_by(|a, b| a.sent_at.cmp(&b.sent_at));
    Ok(messages)
}

/// Get agent health (running member IDs) for a team.
pub fn team_alive_members(repo_root: &Path, team_id: &str) -> Result<Vec<String>, FrameworkError> {
    with_team_registry_ro(repo_root, |registry| {
        let team = registry
            .teams
            .iter()
            .find(|t| t.team_id == team_id)
            .ok_or_else(|| FrameworkError::not_found(format!("team not found: {team_id}")))?;
        Ok(team
            .members
            .iter()
            .filter(|m| m.status == "running")
            .map(|m| m.agent_id.clone())
            .collect())
    })
}

/// List teams, optionally filtered by `team_id`.
pub fn team_list(
    repo_root: &Path,
    team_id_filter: Option<&str>,
) -> Result<Vec<TeamDescriptor>, FrameworkError> {
    with_team_registry_ro(repo_root, |registry| {
        let teams: Vec<TeamDescriptor> = if let Some(filter) = team_id_filter {
            registry
                .teams
                .iter()
                .filter(|t| t.team_id == filter)
                .cloned()
                .collect()
        } else {
            registry.teams.clone()
        };
        Ok(teams)
    })
}

/// Clean up stale teams (completed beyond `retention_seconds`).
pub fn reap_stale_teams(repo_root: &Path, retention_seconds: i64) -> Result<usize, FrameworkError> {
    if retention_seconds <= 0 {
        return Ok(0);
    }
    with_team_registry(repo_root, |registry| {
        let before = registry.teams.len();
        let deadline = framework_core::time::now_iso();

        registry.teams.retain(|t| {
            if t.status != "completed" {
                return true;
            }
            if let Ok(updated) = chrono::DateTime::parse_from_rfc3339(&t.updated_at)
                && let Ok(dead) = chrono::DateTime::parse_from_rfc3339(&deadline) {
                    return dead.signed_duration_since(updated).num_seconds() < retention_seconds;
                }
            // Parse failure: retain the team to be safe rather than silently deleting.
            // If both timestamps are valid, the retain check above handles the comparison.
            // If either is unparseable, we keep the team — it's better to leave a stale
            // entry than to silently lose data. The next reap cycle will retry.
            tracing::warn!(
                "[team_manager] reap: cannot parse timestamp for team '{}' (updated_at={}, deadline={}) — retaining to avoid data loss; next reap cycle will retry",
                t.team_id, t.updated_at, deadline,
            );
            true
        });

        let reaped = before - registry.teams.len();
        // with_team_registry saves automatically
        Ok(reaped)
    })
}
