# Shared Host Entrypoint Contract

## Goal

Codex CLI, Codex App, Claude Code CLI, and Claude Desktop share one repo system
policy. The runtime still emits the Codex-native `codex_profile` artifact for
Codex configuration, but host entrypoint sync is no longer Codex-only. The
supported generated host entrypoints are:

- `codex-cli` reads `AGENTS.md`.
- `codex-app` reads `AGENTS.md`.
- `claude-code-cli` reads `.claude/CLAUDE.md`.
- `claude-desktop` reads `CLAUDE.md`.

Claude surfaces are generated thin policy entrypoints that point back to the
same repository-owned skill system. They do not create copied skill mirrors or a
host-private policy fork. Claude Desktop may also use the repo-managed
`browser-mcp` stdio server as an MCP client edge, but that MCP edge is additive
to the shared host entrypoint contract rather than a replacement for it.

## Default Artifact

- `codex_profile`: native Codex profile output for config paths, context files,
  MCP config, resume hints, and framework alias entrypoints.

## Default Surface Boundary

- The default artifact surface is `codex_profile`, generated host entrypoints,
  Claude hook/settings files, and the sync manifest.
- Host-facing entrypoints are generated projections for discovery and policy
  bootstrap only; `skills/` remains the only live skill source.
- Generated entrypoints and settings are not hand-authored truth. Regenerate
  them through the Rust host-entrypoint sync or host-integration paths.

## Hard Rules

1. `framework_profile` stays Codex-pinned and Codex-private fields stay in
   `codex_profile.codex_host_payload`.
2. `workspace_bootstrap.resources` is the only skills/memory default source.
3. `router-rs codex sync` materializes `AGENTS.md`, `CLAUDE.md`,
   `.claude/CLAUDE.md`, and `.codex/host_entrypoints_sync_manifest.json` for the
   supported host entrypoints.
4. New default artifacts must stay within the single profile plus generated
   host-entrypoint/settings boundary.
5. Claude Desktop and Claude Code integration must stay minimal: prefer
   generated pointers, MCP stdio, settings hooks, and aliases over copied
   mirrors, Docker/image layers, HTTP bridges, Node runtime requirements, or
   host-private policy forks.
