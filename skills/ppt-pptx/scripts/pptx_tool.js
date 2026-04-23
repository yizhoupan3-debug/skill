#!/usr/bin/env node

const { spawnSync } = require("node:child_process");
const { existsSync } = require("node:fs");
const path = require("node:path");

const args = process.argv.slice(2);

function resolveTarget() {
  if (process.env.PPT_PPTX_RUST_TOOL_BIN && existsSync(process.env.PPT_PPTX_RUST_TOOL_BIN)) {
    return { type: "binary", target: process.env.PPT_PPTX_RUST_TOOL_BIN };
  }

  const scriptDir = __dirname;
  const release = path.resolve(scriptDir, "../../rust_tools/target/release/pptx_tool_rs");
  if (existsSync(release)) {
    return { type: "binary", target: release };
  }

  const debug = path.resolve(scriptDir, "../../rust_tools/target/debug/pptx_tool_rs");
  if (existsSync(debug)) {
    return { type: "binary", target: debug };
  }

  const envManifest = process.env.PPT_PPTX_RUST_TOOL_MANIFEST;
  if (envManifest && existsSync(envManifest)) {
    return { type: "cargo", target: envManifest };
  }

  const manifest = path.resolve(scriptDir, "../../rust_tools/pptx_tool_rs/Cargo.toml");
  return { type: "cargo", target: manifest };
}

const command = resolveTarget();
if (command.type === "binary") {
  const result = spawnSync(command.target, args, { stdio: "inherit" });
  process.exit(result.status ?? 1);
}

const result = spawnSync(
  "cargo",
  ["run", "--manifest-path", command.target, "--", ...args],
  { stdio: "inherit" },
);
process.exit(result.status ?? 1);
