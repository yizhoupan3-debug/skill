#!/usr/bin/env bash
# sync-project.sh — 将 skill 框架配置同步到单个项目目录
#
# 用法:
#   ./scripts/sync-project.sh <project-dir>          # 创建缺失文件
#   ./scripts/sync-project.sh --force <project-dir>  # 强制更新到最新版本
#
# 被 sync-framework-global.sh 自动调用（项目级注册表驱动）。
# /initx 安装后自动调用 --force 确保项目配置为最新。

set -euo pipefail

FORCE=0
if [ "${1:-}" = "--force" ]; then
  FORCE=1
  shift
fi

PROJECT_DIR="$(cd "$1" && pwd)"
FRAMEWORK_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "==> Syncing project: $(basename "$PROJECT_DIR")"
[ "$FORCE" = 1 ] && echo "  (--force mode: overwriting existing configs)"

# ---------- helpers ----------
ensure_claude_dir() { mkdir -p "$PROJECT_DIR/.claude"; }

PROJECT_NAME="$(basename "$PROJECT_DIR")"

# ---------- 1. .mcp.json — router-rs-framework 连接 ----------
NEEDS_MCP=0
if [ -f "$PROJECT_DIR/.mcp.json" ]; then
  if [ "$FORCE" = 1 ]; then
    NEEDS_MCP=1
    echo "  -> .mcp.json exists, force-updating..."
  else
    echo "  -> .mcp.json already exists (skipped; use --force to update)"
  fi
else
  NEEDS_MCP=1
  echo "  -> Creating .mcp.json..."
fi

if [ "$NEEDS_MCP" = 1 ]; then
  WRAPPER_DIR="$PROJECT_DIR/scripts"
  WRAPPER_PATH="$WRAPPER_DIR/router-rs-framework.sh"

  if [ ! -f "$WRAPPER_PATH" ] || [ "$FORCE" = 1 ]; then
    mkdir -p "$WRAPPER_DIR"
    cat > "$WRAPPER_PATH" << 'WRAPPEREOF'
#!/usr/bin/env bash
# router-rs-framework MCP server launcher wrapper
set -euo pipefail
SKILL_DIR="${SKILL_FRAMEWORK_ROOT:-${FRAMEWORK_ROOT:-/Users/joe/Developer/skill}}"
cd "$SKILL_DIR" || { echo "FATAL: cannot cd to $SKILL_DIR" >&2; exit 1; }
ROUTER_RS_BIN="${ROUTER_RS_BIN:-/tmp/skill-cargo-target/release/router-rs-cli}"
exec "$ROUTER_RS_BIN" host agent "$@"
WRAPPEREOF
    chmod +x "$WRAPPER_PATH"
    echo "    -> Wrote $WRAPPER_PATH"
  fi

  cat > "$PROJECT_DIR/.mcp.json" << MCPEOF
{
  "mcpServers": {
    "router-rs-framework": {
      "command": "$WRAPPER_PATH",
      "args": [
        "--repo-root",
        "$FRAMEWORK_ROOT",
        "claude"
      ],
      "env": {
        "FRAMEWORK_ROOT": "$FRAMEWORK_ROOT",
        "PROJECT_ROOT": "$PROJECT_DIR",
        "SKILL_FRAMEWORK_ROOT": "$FRAMEWORK_ROOT"
      },
      "description": "Framework snapshot, skill routing, goal/closeout gating",
      "type": "stdio"
    }
  }
}
MCPEOF
  echo "    -> Wrote .mcp.json"
fi

# ---------- 2. .claude/settings.json — 权限 ----------
ensure_claude_dir

NEEDS_SETTINGS=0
if [ -f "$PROJECT_DIR/.claude/settings.json" ]; then
  if [ "$FORCE" = 1 ]; then
    NEEDS_SETTINGS=1
    echo "  -> settings.json exists, force-updating..."
  else
    echo "  -> .claude/settings.json already exists (skipped; use --force to update)"
  fi
else
  NEEDS_SETTINGS=1
  echo "  -> Creating .claude/settings.json..."
fi

if [ "$NEEDS_SETTINGS" = 1 ]; then
  cat > "$PROJECT_DIR/.claude/settings.json" << SETTINGSEOF
{
  "permissions": {
    "allow": [
      "Bash(*)",
      "mcp__router-rs-framework"
    ]
  },
  "sandbox": {
    "allowUnsandboxedCommands": true,
    "autoAllowBashIfSandboxed": true,
    "enabled": false,
    "excludedCommands": [
      "curl *",
      "wget *"
    ],
    "network": {
      "allowLocalBinding": true,
      "allowedDomains": [
        "github.com", "*.githubusercontent.com", "gitlab.com",
        "*.npmjs.org", "registry.yarnpkg.com",
        "pypi.org", "*.pythonhosted.org",
        "arxiv.org", "*.arxiv.org",
        "doi.org", "*.crossref.org",
        "scholar.google.com",
        "api.github.com", "raw.githubusercontent.com",
        "*.wikipedia.org", "stackoverflow.com",
        "*.stackoverflow.com",
        "docs.rs", "crates.io",
        "127.0.0.1", "localhost"
      ]
    }
  }
}
SETTINGSEOF
  echo "    -> Wrote .claude/settings.json"
fi

# ---------- 3. CLAUDE.md — 注入框架引用 ----------
if [ -f "$PROJECT_DIR/CLAUDE.md" ]; then
  if grep -qE "Skill Framework|跨宿主协议" "$PROJECT_DIR/CLAUDE.md" 2>/dev/null; then
    echo "  -> CLAUDE.md framework reference already exists (ok)"
  else
    echo "  -> Injecting framework reference into CLAUDE.md..."
    # 在第一个 ## 节之前插入跨宿主协议段落
    # 找到第一个以 ## 开头的行（第一个章节标题）
    HEADER_LINE=$(grep -n "^##" "$PROJECT_DIR/CLAUDE.md" | head -1 | cut -d: -f1)
    if [ -n "$HEADER_LINE" ]; then
      # 在第一个节标题之前插入
      sed -i '' "${HEADER_LINE}i\\
\\
## 跨宿主协议\\
\\
本仓库使用 Skill Framework 跨宿主协议，见 \`$FRAMEWORK_ROOT/AGENTS.md\`。\\
\\
**Skill Routing**：优先使用 \`skill_route(query)\` 路由到最佳匹配 skill，\\
然后参考 \`recommended_tools\` + \`skill_read(slug)\` 获取完整指引。\\
用于科研场景（文献调研、实验设计、手稿审改、数学推导等）。\\
\\
\\
" "$PROJECT_DIR/CLAUDE.md"
      echo "    -> Injected framework reference before line $HEADER_LINE"
    else
      # 没有章节标题，追加到文件末尾
      cat >> "$PROJECT_DIR/CLAUDE.md" << CLAUDEEOF

## 跨宿主协议

本仓库使用 Skill Framework 跨宿主协议，见 \`$FRAMEWORK_ROOT/AGENTS.md\`。

**Skill Routing**：优先使用 \`skill_route(query)\` 路由到最佳匹配 skill，
然后参考 \`recommended_tools\` + \`skill_read(slug)\` 获取完整指引。
用于科研场景（文献调研、实验设计、手稿审改、数学推导等）。

CLAUDEEOF
      echo "    -> Appended framework reference to end of CLAUDE.md"
    fi
  fi
else
  echo "  ⚠️  No CLAUDE.md found (no framework reference injected)"
fi

# ---------- 4. PROJECT_REGISTRY.json — 注册/更新 ----------
python3 << PYREG
import json, os, re

registry_path = os.path.join("$FRAMEWORK_ROOT", "configs/framework/PROJECT_REGISTRY.json")

if not os.path.exists(registry_path):
    print("  -> PROJECT_REGISTRY.json not found, skipping registry update")
else:
    with open(registry_path) as f:
        registry = json.load(f)

    proj_path = "$PROJECT_DIR"
    proj_id = os.path.basename(proj_path.rstrip("/"))
    proj_name = proj_id
    role = "research-execution"

    found = None
    for p in registry["projects"]:
        if p["path"] == proj_path or p["id"] == proj_id:
            found = p
            break

    claude_md_path = os.path.join(proj_path, "CLAUDE.md")
    has_framework_ref = "missing"
    if os.path.exists(claude_md_path):
        content = open(claude_md_path).read()
        if re.search(r"Skill Framework|跨宿主协议", content):
            has_framework_ref = "present"

    # Detect force mode
    forced = "$FORCE" == "1"

    new_status = {
        "mcp_json": "present" if os.path.exists(os.path.join(proj_path, ".mcp.json")) else "missing",
        "settings_json": "present" if os.path.exists(os.path.join(proj_path, ".claude", "settings.json")) else "missing",
        "claude_md_framework_ref": has_framework_ref,
        "last_sync": "2026-07-05T12:00:00+08:00"
    }

    if found:
        found["status"] = new_status
        print(f"  -> Updated PROJECT_REGISTRY: [{found['id']}]")
    else:
        entry = {
            "id": proj_id,
            "name": proj_name,
            "path": proj_path,
            "status": new_status,
            "framework_role": role
        }
        registry["projects"].append(entry)
        print(f"  -> Registered NEW project: [{proj_id}] -> {proj_path}")

    with open(registry_path, 'w') as f:
        json.dump(registry, f, indent=2, ensure_ascii=False)
        f.write('\n')
PYREG

echo "==> Done: $(basename "$PROJECT_DIR") synced."
