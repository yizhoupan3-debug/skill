---
name: plugin-creator
description: Create and scaffold plugin directories for Codex with a required `.codex-plugin/plugin.json`, optional plugin folders/files, and baseline placeholders you can edit before publishing or testing. Use when Codex needs to create a new local plugin, add optional plugin structure, or generate or update repo-root `.agents/plugins/marketplace.json` entries for plugin ordering and availability metadata.
routing_layer: L0
routing_owner: owner
routing_gate: none
routing_priority: P1
session_start: preferred
short_description: Scaffold a local Codex plugin and optional marketplace entry
trigger_hints:
  - create plugin
  - scaffold plugin
  - plugin marketplace
  - local plugin
  - codex plugin
metadata:
  platforms: [codex]
---

# Plugin Creator

## Quick Start

1. Create the plugin folder and required manifest:

```bash
PLUGIN_NAME="<plugin-name>"
PLUGIN_DIR="plugins/$PLUGIN_NAME"
mkdir -p "$PLUGIN_DIR/.codex-plugin"
$EDITOR "$PLUGIN_DIR/.codex-plugin/plugin.json"
```

Plugin names are normalized to lower-case hyphen-case and must be <= 64 chars. The generated folder and plugin.json `name` are always the same.

2. Fill `<plugin-path>/.codex-plugin/plugin.json` from the canonical sample in `references/plugin-json-spec.md`, then replace `[TODO: ...]` placeholders.

3. Add or update the repo marketplace entry when the plugin should appear in Codex UI ordering:

```bash
# marketplace.json always lives at <repo-root>/.agents/plugins/marketplace.json
$EDITOR .agents/plugins/marketplace.json
```

For a home-local plugin, treat `<home>` as the root and edit:

```bash
~/.agents/plugins/marketplace.json
```

4. Generate/adjust optional companion folders as needed:

```bash
mkdir -p "<parent-plugin-directory>/<plugin-name>"/{skills,hooks,scripts,assets}
touch "<parent-plugin-directory>/<plugin-name>/.mcp.json"
touch "<parent-plugin-directory>/<plugin-name>/.app.json"
```

`<parent-plugin-directory>` is the directory where the plugin folder `<plugin-name>` will be created (for example `~/code/plugins`).

## What this skill creates

- If the user has not made the plugin location explicit, ask whether they want a repo-local plugin or a home-local plugin before generating marketplace entries.
- Creates plugin root at `/<parent-plugin-directory>/<plugin-name>/`.
- Always creates `/<parent-plugin-directory>/<plugin-name>/.codex-plugin/plugin.json`.
- Fills the manifest with the full schema shape, placeholder values, and the complete `interface` section.
- Creates or updates `<repo-root>/.agents/plugins/marketplace.json` when `--with-marketplace` is set.
  - If the marketplace file does not exist yet, seed top-level `name` plus `interface.displayName` placeholders before adding the first plugin entry.
- `<plugin-name>` is normalized using skill-creator naming rules:
  - `My Plugin` → `my-plugin`
  - `My--Plugin` → `my-plugin`
  - underscores, spaces, and punctuation are converted to `-`
  - result is lower-case hyphen-delimited with consecutive hyphens collapsed
- Supports optional creation of:
  - `skills/`
  - `hooks/`
  - `scripts/`
  - `assets/`
  - `.mcp.json`
  - `.app.json`

## Marketplace workflow

- `marketplace.json` always lives at `<repo-root>/.agents/plugins/marketplace.json`.
- For a home-local plugin, use the same convention with `<home>` as the root:
  `~/.agents/plugins/marketplace.json` plus `./plugins/<plugin-name>`.
- Marketplace root metadata supports top-level `name` plus optional `interface.displayName`.
- Treat plugin order in `plugins[]` as render order in Codex. Append new entries unless a user explicitly asks to reorder the list.
- `displayName` belongs inside the marketplace `interface` object, not individual `plugins[]` entries.
- Each generated marketplace entry must include all of:
  - `policy.installation`
  - `policy.authentication`
  - `category`
- Default new entries to:
  - `policy.installation: "AVAILABLE"`
  - `policy.authentication: "ON_INSTALL"`
- Override defaults only when the user explicitly specifies another allowed value.
- Allowed `policy.installation` values:
  - `NOT_AVAILABLE`
  - `AVAILABLE`
  - `INSTALLED_BY_DEFAULT`
- Allowed `policy.authentication` values:
  - `ON_INSTALL`
  - `ON_USE`
- Treat `policy.products` as an override. Omit it unless the user explicitly requests product gating.
- The generated plugin entry shape is:

```json
{
  "name": "plugin-name",
  "source": {
    "source": "local",
    "path": "./plugins/plugin-name"
  },
  "policy": {
    "installation": "AVAILABLE",
    "authentication": "ON_INSTALL"
  },
  "category": "Productivity"
}
```

- Use `--force` only when intentionally replacing an existing marketplace entry for the same plugin name.
- If `<repo-root>/.agents/plugins/marketplace.json` does not exist yet, create it with top-level `"name"`, an `"interface"` object containing `"displayName"`, and a `plugins` array, then add the new entry.

- For a brand-new marketplace file, the root object should look like:

```json
{
  "name": "[TODO: marketplace-name]",
  "interface": {
    "displayName": "[TODO: Marketplace Display Name]"
  },
  "plugins": [
    {
      "name": "plugin-name",
      "source": {
        "source": "local",
        "path": "./plugins/plugin-name"
      },
      "policy": {
        "installation": "AVAILABLE",
        "authentication": "ON_INSTALL"
      },
      "category": "Productivity"
    }
  ]
}
```

## Required behavior

- Outer folder name and `plugin.json` `"name"` are always the same normalized plugin name.
- Do not remove required structure; keep `.codex-plugin/plugin.json` present.
- Keep manifest values as placeholders until a human or follow-up step explicitly fills them.
- If creating files inside an existing plugin path, use `--force` only when overwrite is intentional.
- Preserve any existing marketplace `interface.displayName`.
- When generating marketplace entries, always write `policy.installation`, `policy.authentication`, and `category` even if their values are defaults.
- Add `policy.products` only when the user explicitly asks for that override.
- Keep marketplace `source.path` relative to repo root as `./plugins/<plugin-name>`.

## Reference to exact spec sample

For the exact canonical sample JSON for both plugin manifests and marketplace entries, use:

- `references/plugin-json-spec.md`

## Validation

After editing `SKILL.md`, run:

```bash
cargo test --test policy_contracts
```
