#!/usr/bin/env node

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const ROOT = path.resolve(__dirname, "..");
const SCRIPTS = path.join(ROOT, "scripts");
const ASSETS = path.join(ROOT, "assets");
const EXAMPLES = path.join(ROOT, "examples");
const TOOL_RUNNER = path.join(SCRIPTS, "pptx_tool.js");
const RUST_TOOL_BIN = path.resolve(ROOT, "..", "..", "rust_tools", "target", "debug", "pptx_tool_rs");
const RUST_TOOL_MANIFEST = path.resolve(ROOT, "..", "..", "rust_tools", "pptx_tool_rs", "Cargo.toml");

const NODE_DEPS = [
  "pptxgenjs",
  "skia-canvas",
  "linebreak",
  "fontkit",
  "prismjs",
  "mathjax-full",
  "js-yaml",
];

function rustToolEnv() {
  return {
    ...process.env,
    PPT_PPTX_RUST_TOOL_BIN: RUST_TOOL_BIN,
    PPT_PPTX_RUST_TOOL_MANIFEST: RUST_TOOL_MANIFEST,
  };
}

function rustToolCmd(...args) {
  return ["node", TOOL_RUNNER, ...args];
}

function run(cmd, cwd, label, env = process.env) {
  const proc = spawnSync(cmd[0], cmd.slice(1), {
    cwd,
    env,
    encoding: "utf8",
  });
  if (proc.status !== 0) {
    throw new Error(
      `${label} failed\ncmd: ${cmd.join(" ")}\nstdout:\n${proc.stdout || ""}\nstderr:\n${proc.stderr || ""}`,
    );
  }
  return proc;
}

function parseJsonOutput(label, text) {
  const trimmed = text.trim();
  const starts = [];
  for (let index = 0; index < trimmed.length; index += 1) {
    const ch = trimmed[index];
    if (ch === "{" || ch === "[") {
      starts.push(index);
    }
  }
  for (const start of starts) {
    try {
      return JSON.parse(trimmed.slice(start));
    } catch {}
  }
  throw new Error(`${label} did not return valid JSON\nstdout:\n${text}`);
}

function copyFile(src, dest) {
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  fs.copyFileSync(src, dest);
}

function copyToolRunner(dest) {
  copyFile(TOOL_RUNNER, path.join(dest, "scripts", "pptx_tool.js"));
}

function copyDir(src, dest) {
  fs.cpSync(src, dest, { recursive: true });
}

function npmBootstrap(dest) {
  run(["npm", "init", "-y"], dest, "npm init");
  run(["npm", "install", ...NODE_DEPS], dest, "npm install");
}

function countPngs(dir) {
  if (!fs.existsSync(dir)) {
    return 0;
  }
  return fs.readdirSync(dir).filter((name) => name.endsWith(".png")).length;
}

function officecliAvailable() {
  const proc = spawnSync("which", ["officecli"], { encoding: "utf8" });
  return proc.status === 0;
}

function officecliDoctor(workdir) {
  if (!officecliAvailable()) {
    return null;
  }
  const proc = run(
    rustToolCmd("office", "doctor", "deck.pptx", "--json"),
    workdir,
    "officecli doctor",
    rustToolEnv(),
  );
  return parseJsonOutput("officecli doctor", proc.stdout);
}

function enrichWithDoctor(result, workdir) {
  const doctor = officecliDoctor(workdir);
  if (doctor) {
    result.officecli_issue_count = doctor.issues.count;
    result.officecli_validation_ok = doctor.validation.ok;
  }
  return result;
}

function scenarioOutline(root) {
  const workdir = path.join(root, "outline");
  fs.mkdirSync(workdir, { recursive: true });

  copyFile(path.join(EXAMPLES, "outline_overload.yaml"), path.join(workdir, "outline.yaml"));
  copyFile(path.join(SCRIPTS, "outline_to_deck.js"), path.join(workdir, "outline_to_deck.js"));
  copyDir(path.join(ASSETS, "pptxgenjs_helpers"), path.join(workdir, "pptxgenjs_helpers"));
  copyToolRunner(workdir);
  npmBootstrap(workdir);

  const env = rustToolEnv();
  run(["node", "outline_to_deck.js", "outline.yaml", "-o", "deck.js"], workdir, "outline_to_deck", env);
  run(["node", "deck.js"], workdir, "generated deck.js", env);
  run(rustToolCmd("render", "deck.pptx", "--output_dir", "rendered"), workdir, "render", env);
  run(rustToolCmd("slides-test", "deck.pptx"), workdir, "slides_test", env);
  run(
    rustToolCmd("detect-fonts", "deck.pptx", "--include-missing", "--include-substituted"),
    workdir,
    "detect_font",
    env,
  );
  run(rustToolCmd("extract-structure", "deck.pptx", "-o", "structure.json"), workdir, "extract_structure", env);
  const hybrid = run(
    rustToolCmd("qa", "deck.pptx", "--rendered-dir", "rendered", "--json"),
    workdir,
    "hybrid qa",
    env,
  );
  const hybridPayload = parseJsonOutput("hybrid qa", hybrid.stdout);

  return enrichWithDoctor(
    {
      name: "outline_flow",
      workdir,
      deck_exists: fs.existsSync(path.join(workdir, "deck.pptx")),
      rendered_pngs: countPngs(path.join(workdir, "rendered")),
      structure_json: fs.existsSync(path.join(workdir, "structure.json")),
      hybrid_render_pngs: hybridPayload.render.png_count,
    },
    workdir,
  );
}

function scenarioTemplate(root) {
  const workdir = path.join(root, "template");
  fs.mkdirSync(path.join(workdir, "assets"), { recursive: true });

  copyFile(path.join(ASSETS, "deck.template.js"), path.join(workdir, "deck.js"));
  copyDir(path.join(ASSETS, "pptxgenjs_helpers"), path.join(workdir, "pptxgenjs_helpers"));
  copyToolRunner(workdir);
  npmBootstrap(workdir);

  const env = rustToolEnv();
  run(["node", "deck.js"], workdir, "template deck.js", env);
  run(rustToolCmd("render", "deck.pptx", "--output_dir", "rendered"), workdir, "render", env);
  run(rustToolCmd("slides-test", "deck.pptx"), workdir, "slides_test", env);
  run(
    rustToolCmd("detect-fonts", "deck.pptx", "--include-missing", "--include-substituted"),
    workdir,
    "detect_font",
    env,
  );
  const hybrid = run(
    rustToolCmd("qa", "deck.pptx", "--rendered-dir", "rendered", "--json"),
    workdir,
    "hybrid qa",
    env,
  );
  const hybridPayload = parseJsonOutput("hybrid qa", hybrid.stdout);

  return enrichWithDoctor(
    {
      name: "template_flow",
      workdir,
      deck_exists: fs.existsSync(path.join(workdir, "deck.pptx")),
      rendered_pngs: countPngs(path.join(workdir, "rendered")),
      hybrid_render_pngs: hybridPayload.render.png_count,
    },
    workdir,
  );
}

function scenarioSampleDeck(root) {
  const workdir = path.join(root, "sample_deck");
  fs.mkdirSync(workdir, { recursive: true });

  copyFile(path.join(ROOT, "deck.js"), path.join(workdir, "deck.js"));
  copyDir(path.join(ASSETS, "pptxgenjs_helpers"), path.join(workdir, "pptxgenjs_helpers"));
  copyToolRunner(workdir);
  npmBootstrap(workdir);

  const env = rustToolEnv();
  run(["node", "deck.js"], workdir, "sample deck.js", env);
  run(rustToolCmd("render", "deck.pptx", "--output_dir", "rendered"), workdir, "render", env);
  const hybrid = run(
    rustToolCmd("qa", "deck.pptx", "--rendered-dir", "rendered", "--json"),
    workdir,
    "hybrid qa",
    env,
  );
  const hybridPayload = parseJsonOutput("hybrid qa", hybrid.stdout);

  return enrichWithDoctor(
    {
      name: "sample_deck_flow",
      workdir,
      deck_exists: fs.existsSync(path.join(workdir, "deck.pptx")),
      rendered_pngs: countPngs(path.join(workdir, "rendered")),
      hybrid_render_pngs: hybridPayload.render.png_count,
    },
    workdir,
  );
}

function parseArgs(argv) {
  return {
    keepWorkdir: argv.includes("--keep-workdir"),
    json: argv.includes("--json"),
    strictOfficecli: argv.includes("--strict-officecli"),
  };
}

function printTextSummary(root, results) {
  console.log(`PASS: ppt-pptx smoke test (${root})`);
  for (const item of results) {
    const extras = Object.entries(item)
      .filter(([key]) => key !== "name" && key !== "workdir")
      .map(([key, value]) => `${key}=${value}`)
      .join(", ");
    console.log(`- ${item.name}: ${extras}`);
  }
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "pptx-smoke-"));

  try {
    if (!fs.existsSync(RUST_TOOL_BIN)) {
      run(["cargo", "build", "--manifest-path", RUST_TOOL_MANIFEST], path.resolve(ROOT, "..", ".."), "cargo build");
    }
    const results = [
      scenarioOutline(tempRoot),
      scenarioTemplate(tempRoot),
      scenarioSampleDeck(tempRoot),
    ];
    if (
      args.strictOfficecli &&
      results.some(
        (item) => (item.officecli_issue_count || 0) > 0 || item.officecli_validation_ok === false,
      )
    ) {
      throw new Error("OfficeCLI strict audit found deck issues or validation failures");
    }

    const payload = {
      status: "pass",
      root: tempRoot,
      officecli_available: officecliAvailable(),
      results,
    };

    if (args.json) {
      console.log(JSON.stringify(payload, null, 2));
    } else {
      printTextSummary(tempRoot, results);
      if (args.keepWorkdir) {
        console.log(`Kept workspace: ${tempRoot}`);
      }
    }
    return 0;
  } catch (error) {
    if (args.json) {
      console.log(
        JSON.stringify(
          {
            status: "fail",
            root: tempRoot,
            error: error instanceof Error ? error.message : String(error),
          },
          null,
          2,
        ),
      );
    } else {
      console.error(`FAIL: ${error instanceof Error ? error.message : String(error)}`);
      console.error(`Workspace: ${tempRoot}`);
      if (args.keepWorkdir) {
        console.error(`Kept workspace: ${tempRoot}`);
      }
    }
    return 1;
  } finally {
    if (!args.keepWorkdir) {
      fs.rmSync(tempRoot, { recursive: true, force: true });
    }
  }
}

process.exit(main());
