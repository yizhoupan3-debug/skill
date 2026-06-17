#!/usr/bin/env bash
# RETIRED 2026-06: claude-desktop host removed from closed set. Use install-claude.sh for claude-code.
echo "RETIRED: claude-desktop is no longer supported. Use install-claude.sh instead." >&2
exit 1

MODE="${1:-bypassPermissions}"
if [[ -z "${CLAUDE_DESKTOP_ACCOUNT_ID:-}" ]]; then
  echo "error: set CLAUDE_DESKTOP_ACCOUNT_ID (Desktop account UUID from Claude-3p config)" >&2
  exit 1
fi
ACCOUNT_ID="${CLAUDE_DESKTOP_ACCOUNT_ID}"
if [[ "$(uname)" == "Darwin" ]]; then
  APPDATA_DIR="$HOME/Library/Application Support"
elif [[ -n "${APPDATA:-}" ]]; then
  APPDATA_DIR="$APPDATA"
else
  APPDATA_DIR="${XDG_CONFIG_HOME:-$HOME/.config}"
fi

CONFIG_3P="${CLAUDE_3P_DESKTOP_CONFIG:-$APPDATA_DIR/Claude-3p/claude_desktop_config.json}"
LEVELDB_3P="$APPDATA_DIR/Claude-3p/Local Storage/leveldb"
LEVELDB_STD="$APPDATA_DIR/Claude/Local Storage/leveldb"

COWORK_FILES="${COWORK_USER_FILES:-$HOME/Claude}"

usage() {
  cat <<'EOF'
Usage: patch-claude-desktop-permission-mode.sh [mode]

Persist Desktop permission mode to reduce Allow/Deny prompts (Cowork + Code).
Does NOT disable AskUserQuestion (clarifying multiple-choice).

Modes: bypassPermissions (default) | acceptEdits | auto | plan | default

Steps:
  1. Patch Claude-3p/claude_desktop_config.json (cc-landing-draft-permission-mode)
  2. Patch Electron Local Storage LevelDB keys containing "permission-mode"
     (requires Claude Desktop fully quit — Cmd+Q)

Examples:
  ./scripts/patch-claude-desktop-permission-mode.sh
  ./scripts/patch-claude-desktop-permission-mode.sh acceptEdits

After: Cmd+Q quit Desktop → run script → reopen → new Cowork sessions should
use the selected mode. Existing Cowork tabs may need a new session.
EOF
}

case "${MODE:-}" in
  -h | --help)
    usage
    exit 0
    ;;
  bypassPermissions | acceptEdits | auto | plan | default)
    ;;
  *)
    echo "error: invalid mode: $MODE" >&2
    usage >&2
    exit 1
    ;;
esac

for cmd in jq npm; do
  command -v "$cmd" >/dev/null 2>&1 || { echo "error: $cmd not found; install it first" >&2; exit 1; }
done

echo "==> Setting permission mode: $MODE"
if [[ "$MODE" == "bypassPermissions" ]]; then
  echo "WARNING: bypassPermissions skips all Allow/Deny prompts." >&2
  echo "  Use only in trusted local dev environments." >&2
  echo ""
fi

patch_desktop_config() {
  local cfg="$1"
  if [[ ! -f "$cfg" ]]; then
    echo "skip: no config at $cfg"
    return 0
  fi
  export PATCH_CFG="$cfg" PATCH_MODE="$MODE" PATCH_ACCOUNT="$ACCOUNT_ID" PATCH_COWORK_FILES="$COWORK_FILES"
  # Build folder paths array in bash (jq can't access CLAUDE_DESKTOP_FOLDER_PATHS)
  folder_paths_json=$(printf '%s' "$COWORK_FILES" | jq -R .)
  if [[ -n "${CLAUDE_DESKTOP_FOLDER_PATHS:-}" ]]; then
    extra_paths=$(printf '%s' "$CLAUDE_DESKTOP_FOLDER_PATHS" | tr ':' '\n' | grep -v '^$' | jq -R . | jq -s .)
    folder_paths_json=$(echo "[$folder_paths_json]" "$extra_paths" | jq -s 'add')
  else
    folder_paths_json="[$folder_paths_json]"
  fi
  old_mode=$(jq -r '.preferences.epitaxyPrefs["cc-landing-draft-permission-mode"] // "null"' "$cfg")
  tmp="${cfg}.tmp"
  jq --arg mode "$MODE" --arg account "$ACCOUNT_ID" --argjson paths "$folder_paths_json" '
    .preferences //= {} |
    .preferences.bypassPermissionsModeEnabled = true |
    .preferences.bypassPermissionsGateByAccount //= {} |
    .preferences.bypassPermissionsGateByAccount[$account] = true |
    .preferences.bypassPermissionsOptInByAccount //= {} |
    .preferences.bypassPermissionsOptInByAccount[$account] = true |
    .preferences.epitaxyPrefs //= {} |
    .preferences.epitaxyPrefs["cc-landing-draft-permission-mode"] = $mode |
    .preferences.epitaxyPrefs[("epitaxy-folder-permission-mode." + $account)] =
      ((.preferences.epitaxyPrefs[("epitaxy-folder-permission-mode." + $account)] // {}) *
       ($paths | reduce .[] as $p ({}; . + {($p): $mode}))) |
    .preferences.epitaxyPrefs[("epitaxy-perm-mode-acks." + $account)] =
      [(.preferences.epitaxyPrefs[("epitaxy-folder-permission-mode." + $account)] // {} | keys[]) as $p | ($p + ":" + $mode)]
  ' "$cfg" > "$tmp" && mv "$tmp" "$cfg"
  new_folder_count=$(jq --arg account "$ACCOUNT_ID" \
    '.preferences.epitaxyPrefs[("epitaxy-folder-permission-mode." + $account)] | length' "$cfg")
  echo "patched: $cfg"
  echo "  cc-landing-draft-permission-mode: '$old_mode' -> '$MODE'"
  echo "  epitaxy folders ($new_folder_count): $MODE"
}

patch_leveldb() {
  local db_path="$1"
  if [[ ! -d "$db_path" ]]; then
    echo "skip: no LevelDB at $db_path"
    return 0
  fi
  if ! command -v node >/dev/null 2>&1; then
    echo "warn: node not found; skipping LevelDB patch for $db_path" >&2
    return 0
  fi
  local tmpdir="/tmp/claude-desktop-leveldb-patch-env"
  mkdir -p "$tmpdir"
  (
    cd "$tmpdir"
    if [[ ! -d "node_modules/classic-level" ]]; then
      npm init -y >/dev/null 2>&1
      npm install "classic-level@1.2.0" --ignore-scripts --no-audit --no-fund >/dev/null 2>&1
    fi
    local cfg_for_db="$CONFIG_3P"
    [[ -f "$cfg_for_db" ]] || cfg_for_db=""
    node - "$db_path" "$MODE" "$cfg_for_db" <<'NODE'
const { ClassicLevel } = require("classic-level");
const fs = require("fs");

const dbPath = process.argv[2];
const targetMode = process.argv[3];
const configPath = process.argv[4];
const VALID = ["bypassPermissions", "acceptEdits", "auto", "plan", "default"];
if (!VALID.includes(targetMode)) {
  console.error(`invalid mode: ${targetMode}`);
  process.exit(1);
}

function folderMapFromConfig() {
  if (!configPath || !fs.existsSync(configPath)) return null;
  try {
    const cfg = JSON.parse(fs.readFileSync(configPath, "utf8"));
    const ep = cfg?.preferences?.epitaxyPrefs ?? {};
    for (const [k, v] of Object.entries(ep)) {
      if (k.startsWith("epitaxy-folder-permission-mode.") && v && typeof v === "object") {
        return v;
      }
    }
  } catch {
    return null;
  }
  return null;
}

(async () => {
  let db;
  try {
    db = new ClassicLevel(dbPath, { keyEncoding: "buffer", valueEncoding: "buffer" });
    await db.open();
  } catch (err) {
    if (String(err.message || err).includes("lock")) {
      console.error(`locked: ${dbPath} — quit Claude Desktop (Cmd+Q) and re-run`);
      process.exit(2);
    }
    throw err;
  }
  let updated = 0;
  for await (const [key, value] of db.iterator()) {
    const keyStr = key.toString("utf8");
    if (!keyStr.includes("permission-mode")) continue;
    const valStr = value.toString("utf8");
    const prefix = valStr.startsWith("\x01") ? "\x01" : "";
    const jsonStr = prefix ? valStr.slice(1) : valStr;
    let parsed;
    try {
      parsed = JSON.parse(jsonStr);
    } catch {
      parsed = null;
    }

    // Folder map keys store { value: { "/path": "mode", ... } } — do not flatten to a string.
    if (keyStr.includes("folder-permission-mode")) {
      let folderMap = parsed?.value;
      if (!folderMap || typeof folderMap !== "object" || Array.isArray(folderMap)) {
        folderMap = folderMapFromConfig();
        if (!folderMap) {
          console.log(`  skip folder key (unexpected shape, no config fallback): ${keyStr.slice(-72)}`);
          continue;
        }
        console.log(`  repair folder map from config (${Object.keys(folderMap).length} paths)`);
      }
      let changed = false;
      for (const [p, m] of Object.entries(folderMap)) {
        if (m !== targetMode) {
          folderMap[p] = targetMode;
          changed = true;
        }
      }
      if (!changed) {
        console.log(`  ok folder map already ${targetMode}: ${keyStr.slice(-72)}`);
        continue;
      }
      const next = { ...(parsed && typeof parsed === "object" ? parsed : {}), value: folderMap, timestamp: Date.now() };
      await db.put(key, Buffer.from(prefix + JSON.stringify(next)));
      console.log(`  leveldb ${dbPath}: folder map -> ${targetMode} (${Object.keys(folderMap).length} paths)`);
      updated++;
      continue;
    }

    const current = parsed?.value ?? "(unreadable)";
    const next = { ...(parsed && typeof parsed === "object" ? parsed : {}), value: targetMode, timestamp: Date.now() };
    await db.put(key, Buffer.from(prefix + JSON.stringify(next)));
    console.log(`  leveldb ${dbPath}: ${current} -> ${targetMode} (${keyStr.slice(-60)})`);
    updated++;
  }
  if (updated === 0) {
    const fullKey = Buffer.from("_https://claude.ai\x00\x01LSS-cc-landing-draft-permission-mode");
    const newValue = Buffer.from("\x01" + JSON.stringify({ value: targetMode, tabId: "", timestamp: Date.now() }));
    await db.put(fullKey, newValue);
    console.log(`  leveldb ${dbPath}: (created) -> ${targetMode}`);
    updated = 1;
  }
  await db.close();
  console.log(`leveldb ok: ${updated} key(s) in ${dbPath}`);
})().catch((err) => {
  console.error(err.message || err);
  process.exit(1);
});
NODE
  )
}

echo "==> Desktop config (3P)"
patch_desktop_config "$CONFIG_3P"

STD_CONFIG="$APPDATA_DIR/Claude/claude_desktop_config.json"
if [[ -f "$STD_CONFIG" ]]; then
  echo "==> Desktop config (standard Claude/)"
  patch_desktop_config "$STD_CONFIG"
fi

echo "==> LevelDB (quit Desktop first if locked)"
set +e
patch_leveldb "$LEVELDB_3P"
rc3p=$?
patch_leveldb "$LEVELDB_STD"
rc_std=$?
set -e

echo ""
if [[ $rc3p -eq 2 || $rc_std -eq 2 ]]; then
  echo "LevelDB locked. Config JSON is patched; for full effect: Cmd+Q → re-run this script → reopen Desktop."
  exit 0
fi
echo "Done. Cmd+Q reopen Desktop; start a NEW Cowork session and verify fewer Allow/Deny prompts."
echo "AskUserQuestion (clarifying choices) is unchanged — by design."
