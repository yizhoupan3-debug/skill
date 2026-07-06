# Skill Framework — Claude Code / Codex Agent Toolkit

**A complete Claude Code / Codex skill routing, tool integration, and hook lifecycle framework.**  
Works on macOS, Linux, and **Windows (Git Bash)**.

50+ skills out of the box: research workflow (literature → experiment → math-verify → paper writing), deep code review, deep-search, document processing (PDF/DOCX/PPTX/Excel), design prototyping, MCP server management and more.

## Features

- **NL Routing** — natural language automatically matches the best skill, no commands to memorize
- **6-event Hooks** — SessionStart/PreToolUse/PostToolUse/UserPromptSubmit/SubagentStartStop/Stop
- **Rust Core** — `router-rs-cli` drives routing + hook dispatch, zero Python dependency
- **Cross-host** — same config system for Claude Code (macOS/Linux/Windows), Claude Desktop, Codex
- **Goal Lifecycle** — auto-detect user intent, create/track/close Goal contracts
- **50+ Skills** — research/deep-search/code-review/math-verify/paper-workbench/pdf/docx/ppt/simplify/smoke and more

## Directory Structure

```
skill/
├── .claude/
│   ├── CLAUDE.md                  # Project-level CLAUDE.md (auto-loaded)
│   ├── settings.json              # Hooks + Permissions + Sandbox (gitignored, copy from setup/)
│   └── router-rs-hook.env         # Hook env vars (gitignored)
├── setup/
│   ├── CLAUDE_USER.md             # User-level CLAUDE.md template → copy to ~/.claude/CLAUDE.md
│   ├── framework.md               # Framework rules template → copy to ~/.claude/rules/framework.md
│   ├── settings.json              # Hook settings template → copy to .claude/settings.json
│   └── install.sh                 # One-click install script
├── AGENTS.md                      # Host protocol (routing rules + skills directory + quick-ref)
├── configs/framework/
│   ├── hook.sh                    # Unified hook dispatcher
│   ├── claude-router-rs-hook.sh   # Claude-host hook launcher
│   ├── RUNTIME_REGISTRY.json      # Host registry (claude/cursor/codex/opencode)
│   ├── MCP_TOOL_REGISTRY.json     # MCP tool registry
│   └── SKILL_TO_TOOL_MAP.json     # Skill→Tool map
├── skills/
│   ├── SKILL_ROUTING_RUNTIME.json # Master routing table (50+ skills registered)
│   ├── SKILL_ROUTING_LAYERS.md    # Routing layer documentation
│   ├── research/                  # Research skill group
│   ├── research-harness/          # Research workspace (Rust core)
│   ├── code-review-deep/          # Adversarial code review
│   ├── simplify/                  # Code simplification
│   ├── deep-search/               # Deep web search engine
│   ├── math-verify/               # Math verification
│   ├── paper-workbench/           # Paper full workflow
│   ├── pdf/ doc/ slides/ spreadsheets/  # Document processing
│   └── ... (50+ more)
├── core/
│   ├── router-rs/                 # Rust core (routing engine + hook dispatcher)
│   └── research-harness/          # Research workspace Rust crate
└── configs/framework/             # Framework configs + hook scripts
```

## Prerequisites

| Component | Requirements |
|-----------|-------------|
| **Claude Code / Codex** | Any platform: macOS / Linux / Windows (Git Bash) |
| **Rust** | Install via [rustup.rs](https://rustup.rs) — builds `router-rs-cli` |
| **Git** | Clone the repo |
| **jq** | JSON parsing for hook scripts — `winget install jq` / `brew install jq` / `apt install jq` |

## Quick Start

### 1. Clone

```bash
git clone --recurse-submodules https://github.com/yizhoupan3-debug/skill.git
cd skill
```

### 2. Run install script

```bash
bash setup/install.sh
```

This auto-installs: user-level CLAUDE.md → `~/.claude/`, framework rules, native skill symlinks, `router-rs-cli` binary, `.claude/router-rs-hook.env`, and copies `settings.json`.

### 3. Manual setup (if script skips steps)

#### 3a. User-level CLAUDE.md

```bash
cp setup/CLAUDE_USER.md ~/.claude/CLAUDE.md
```

#### 3b. Framework rules

```bash
mkdir -p ~/.claude/rules
cp setup/framework.md ~/.claude/rules/framework.md
```

#### 3c. Native skills

**macOS / Linux:**
```bash
for skill in simplify gitx update deepinterview smoke goalx initx; do
  ln -sf "$PWD/skills/$skill" "$HOME/.claude/skills/$skill"
done
```

**Windows (Git Bash):**
```bash
mkdir -p ~/.claude/skills
for skill in simplify gitx update deepinterview smoke goalx initx; do
  mkdir -p "$HOME/.claude/skills/$skill"
  cp -r "skills/$skill/"* "$HOME/.claude/skills/$skill/"
done
```

Also copy these two that don't follow the standard layout:

```bash
# Windows: copy hallmark and huashu-design
mkdir -p ~/.claude/skills/hallmark ~/.claude/skills/huashu-design
cp .claude/skills/hallmark/SKILL.md ~/.claude/skills/hallmark/
cp .claude/skills/huashu-design/SKILL.md ~/.claude/skills/huashu-design/
```

#### 3d. Copy settings.json (CRITICAL — enables hooks)

```bash
cp setup/settings.json .claude/settings.json
```

Then create `.claude/router-rs-hook.env`:

```
SKILL_FRAMEWORK_ROOT=/full/path/to/skill
ROUTER_RS_BIN=${HOME}/.local/bin/router-rs-cli
ROUTER_RS_CONTINUITY_POSTTOOL_EVIDENCE=0
```

#### 3e. Build router-rs-cli

```bash
cd core/router-rs
cargo build --release --bin router-rs-cli
mkdir -p ~/.local/bin
cp target/release/router-rs-cli ~/.local/bin/
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
```

### 4. Verify

Open Claude Code / Codex in the repo root and try:

```
What skills are available?
```

The routing engine will match against `skills/SKILL_ROUTING_RUNTIME.json` with 50+ registered skills. For a full-function check, run the framework doctor:

```bash
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework doctor --repo-root "$PWD"
```

## How It Works

```
User: "Literature review on X"
        │
        ▼
┌─ skill_route(query) ─────────────────────┐
│  1. NL match against SKILL_ROUTING_RUNTIME│
│  2. Returns selected_skill + tools        │
│  3. Falls back to ~/.claude/skills/<name> │
└────────────────────────────────────────────┘
        │
        ▼
┌─ Route to skills/research/SKILL.md ──────┐
│  Research skill internally distributes    │
│  to lanes: discovery / execution / paper  │
└────────────────────────────────────────────┘
```

### Hook Event Flow (enabled by settings.json)

```
UserPromptSubmit → router-rs goal_auto_detect
PreToolUse       → router-rs route_intercept
PostToolUse      → router-rs evidence_record
Stop             → router-rs closeout_gate + review_gate
SessionStart     → router-rs session_init
SubagentStart    → router-rs agent_health_monitor start
SubagentStop     → router-rs agent_health_monitor stop
```

### NL Routing Pipeline

```
prompt
  → skill_route(prompt) → SKILL_ROUTING_RUNTIME.json
  → search_tools(prompt, top_k) → MCP_TOOL_REGISTRY.json
  → selected_skill + matching tools returned to AI context
```

## Skill Quick Reference

| Skill | When to use |
|-------|-------------|
| `$research` | Unified research entry (literature/experiment/manuscript) |
| `$code-review-deep` | Adversarial deep code review |
| `$simplify` | Code simplification (reuse + quality) |
| `$deep-search` | Deep web search + fact verification |
| `$math-verify` | Math derivation / proof / formula verification |
| `$paper-workbench` | Full paper workflow (review/write/rebuttal) |
| `$pdf` / `$doc` / `$slides` / `$spreadsheets` | Document format handling |
| `$smoke` | Internal component diagnostics + external evaluation |
| `$good-question` | Research question sharpening |
| `$good-story` | Results to narrative |
| `$systematic-debugging` | Root-cause debugging |
| `$design-md` / `$hallmark` | Design systems / UI design |
| `$tikz-paper-figure` | Publication-grade TikZ figures |
| `$goalx` | Goal lifecycle management |
| `$initx` | Project-level harness installation |

Full list: see [AGENTS.md](AGENTS.md) Skill Directory section and `skills/SKILL_ROUTING_RUNTIME.json`.

## Host Support

| Host | Status | Notes |
|------|--------|-------|
| **Claude Code** | ✅ Full | macOS/Linux/Windows CLI |
| **Claude Desktop** | ✅ Full | GUI version (MCP-based) |
| **Codex (Windows)** | ✅ Supported | Fully functional under Git Bash |
| **Cursor** | ⚠️ Registered | Needs validation |
| **OpenCode** | ⚠️ Registered | Needs validation |

Each host's config paths, hook adapter, and settings format are defined in `configs/framework/RUNTIME_REGISTRY.json`.

## Windows-Specific Notes

1. **Git Bash is the recommended terminal** — supports bash scripts but not full symlinks
2. **Symlink fallback:** the install script auto-falls back to `cp -r` instead of `ln -sf`
3. **PATH setup:** add `%USERPROFILE%\.local\bin` and `%USERPROFILE%\.cargo\bin` to system PATH
4. **Claude Code on Windows:** installed at `%LOCALAPPDATA%\Claude Code`
5. **jq on Windows:** `winget install jq` or download `jq.exe` and place it in PATH
6. **Performance:** `cargo build` is slow on first run; subsequent builds reuse the cache

## FAQ

**Q: Hooks keep reporting "router-rs binary unavailable"?**  
A: Make sure `router-rs-cli` is built and on PATH, or set `ROUTER_RS_BIN` correctly in `.claude/router-rs-hook.env`.

**Q: `skill_route` can't find a matching skill?**  
A: Check the skill's `trigger_hints` in `skills/SKILL_ROUTING_RUNTIME.json` — routing uses NL matching, not exact keywords. Try more descriptive phrasing.

**Q: settings.json is gitignored — copy every clone?**  
A: Yes. `.claude/settings.json` contains local paths, permissions, and personal config. `setup/settings.json` is the template; `setup/install.sh` copies it automatically.

**Q: Do I need Rust just to use the skills?**  
A: No. Core skills (research/code-review/paper/doc handling) work without the router-rs binary. Hooks (goal auto-detect, closeout gate) need the binary for full effect.

**Q: Can I just copy `skills/` to use the routing?**  
A: Not recommended. The full routing requires `skills/SKILL_ROUTING_RUNTIME.json`, `AGENTS.md` (or .claude/CLAUDE.md), and the framework configs together.

## Daily Maintenance

```bash
# Full framework update (equals /update)
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework maint update-one-shot

# Skill refresh + tests
cargo run --release --manifest-path core/router-rs/Cargo.toml -- framework skills refresh --write
cargo test --test policy_contracts
```

## Adding a New Skill

1. Create `skills/<name>/SKILL.md` (frontmatter + `## When to use` / `## Do not use`)
2. Add entry to `skills/SKILL_ROUTING_RUNTIME.json`
3. Rebuild companion indexes: `cargo run --manifest-path core/router-rs/Cargo.toml -- framework skills refresh --write --write-companions`
4. Validate: `cargo test --test policy_contracts`

## License

MIT
