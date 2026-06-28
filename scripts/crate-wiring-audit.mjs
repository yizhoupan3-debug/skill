export const meta = {
  name: "crate-wiring-audit",
  description: "深度核查：每个 crate 是否被正确引用、每个 pub fn 是否有 call site、特征门控是否完整",
  phases: [
    { title: "Phase 1: Crate Dependency Graph" },
    { title: "Phase 2: Cross-Crate Call Sites" },
    { title: "Phase 3: Feature Gate Audit" },
    { title: "Phase 4: CLI Subcommand Wiring" },
    { title: "Phase 5: SKILL.md Crate References" },
    { title: "Phase 6: Synthesis" },
  ],
};

const REPO = "/Users/joe/Developer/skill";

phase("Phase 1: Crate Dependency Graph");
log("Checking every workspace member has at least one dependent...");

const p1 = await agent(`Read ${REPO}/Cargo.toml and find ALL workspace members (members list + dependencies section).

Then for each of these crates, grep across the entire workspace Cargo.toml files to find how many other crates depend on it:

1. core/core-errors
2. core/core-state
3. core/core-state-utils
4. core/core-state-types
5. core/eval-route
6. core/fr-utils
7. core/fr-contracts
8. core/fr-exec
9. core/framework-kernel
10. core/core-policy
11. core/routing-engine
12. core/router-rs
13. core/runtime-storage
14. core/runtime-core
15. core/runtime-core-contracts
16. core/trace-runtime
17. core/browser-mcp-dispatch
18. core/host-projection
19. core/framework-extra
20. core/research-harness
21. core/session-supervisor
22. core/hook-framework
23. core/codegraph-rs
24. core/runtime-infra
25. tools/browser-mcp
26. Any other members listed

Report:
- Crates with ZERO dependents (these are dead entries — only the binary crate should have 0)
- Crates with only one dependent (the binary crate) — borderline, note it
- Unused workspace dependencies (declared in [workspace.dependencies] but no crate actually uses them)

Do NOT count the crate's own test dependencies or dev-dependencies.
`, {
  label: "Crate dependency graph",
  schema: {
    type: "object",
    properties: {
      workspace_members: { type: "array", items: { type: "string" } },
      dead_crates: { type: "array", items: { type: "string" } },
      thin_crates: { type: "array", items: { type: "object", properties: { name: { type: "string" }, dependents: { type: "integer" } } } },
      unused_workspace_deps: { type: "array", items: { type: "string" } },
      issues: { type: "array", items: { type: "string" } },
    },
    required: ["workspace_members", "dead_crates", "issues"],
  },
});

phase("Phase 2: Cross-Crate Call Sites");
log("Checking every pub fn exported from core crates has external callers...");

const p2 = await agent(`Scan ALL pub functions in the following core crates and verify each has at least one call site outside its definition crate.

Cross-referencing: for each pub fn, grep across ALL .rs files in ${REPO}/core/ and ${REPO}/tools/ (excluding the definition file itself and its test modules) to find callers.

Key crates to scan (focus on the 'entry point' functions):

1. core/host-projection/src/lib.rs — every pub fn
2. core/runtime-core/src/lib.rs — every pub fn (especially init_hooks, combined_orchestrator_handler)
3. core/framework-kernel/src/ — every pub fn
4. core/routing-engine/src/ — every pub fn
5. core/core-state/src/ — every pub fn
6. core/session-supervisor/src/lib.rs — every pub fn
7. core/framework-extra/src/ — every pub fn
8. core/core-errors/src/ — every pub fn
9. core/research-harness/src/ — every pub fn (especially handle_research_tool, math_tool_dispatch, verification_tool_dispatch)

For each crate report:
- Total pub fns checked
- Dead pub fns (zero call sites outside definition crate)
- Test-only pub fns (only called from #[cfg(test)] blocks)
`, {
  label: "Cross-crate call site analysis",
  schema: {
    type: "object",
    properties: {
      crates_scanned: { type: "array", items: { type: "string" } },
      total_pub_fns: { type: "integer" },
      dead_pub_fns: { type: "array", items: { type: "object", properties: { fn_name: { type: "string" }, crate: { type: "string" } } } },
      test_only_fns: { type: "array", items: { type: "object", properties: { fn_name: { type: "string" }, crate: { type: "string" } } } },
      issues: { type: "array", items: { type: "string" } },
    },
    required: ["crates_scanned", "total_pub_fns", "dead_pub_fns", "issues"],
  },
});

phase("Phase 3: Feature Gate Audit");
log("Checking all #[cfg(feature = ...)] gates are matched by actual Cargo features...");

const p3 = await agent(`Scan ALL Rust source files in ${REPO}/core/ and ${REPO}/tools/ for #[cfg(feature = "..." )] and #[cfg(not(feature = "..."))] directives.

For each feature string found in the source, verify:
1. Is it declared in the Cargo.toml's [features] section of that crate?
2. For shared features used across crates (like "test-support", "codegraph"), are they enabled by the workspace's dependency resolution?

Also check Cargo.toml files for:
- Features declared but NEVER used in source code
- Features that are only used in the crate that declares them vs cross-crate features

Report all feature gate mismatches.
`, {
  label: "Feature gate audit",
  schema: {
    type: "object",
    properties: {
      features_in_source: { type: "integer" },
      features_declared: { type: "integer" },
      feature_mismatches: { type: "array", items: { type: "object", properties: { feature: { type: "string" }, issue: { type: "string" } } } },
      unused_features: { type: "array", items: { type: "string" } },
      issues: { type: "array", items: { type: "string" } },
    },
    required: ["features_in_source", "features_declared", "feature_mismatches", "issues"],
  },
});

phase("Phase 4: CLI Subcommand Wiring");
log("Checking router-rs CLI subcommands map to actual handlers...");

const p4 = await agent(`Read ${REPO}/core/router-rs/src/cli/ to trace the CLI subcommand tree.

Specifically:
1. Read router_command_dispatch.rs and identify ALL CLI subcommand variants
2. For each variant, check if the handler function actually exists and compiles
3. Check if there are handler functions NOT reachable from any subcommand (dead CLI code)

Also check:
- Does the CLI subcommand structure match what spawn_cli_tool in host-projection expects?
- The map_tool_to_cli_args function in mod.rs references "web", "research", "math" subcommands — do these exist in router-rs's CLI?
- Are there CLI subcommands that have no MCP_TOOL_REGISTRY entry (orphan CLI commands)?
`, {
  label: "CLI subcommand wiring audit",
  schema: {
    type: "object",
    properties: {
      cli_subcommands_found: { type: "integer" },
      cli_subcommands_with_handler: { type: "integer" },
      orphan_cli_handlers: { type: "array", items: { type: "string" } },
      cli_mcp_mismatches: { type: "array", items: { type: "object", properties: { cli_cmd: { type: "string" }, mcp_tool: { type: "string" } } } },
      issues: { type: "array", items: { type: "string" } },
    },
    required: ["cli_subcommands_found", "cli_subcommands_with_handler", "issues"],
  },
});

phase("Phase 5: SKILL.md Crate References");
log("Checking each SKILL.md references valid crate imports...");

const p5 = await agent(`For each SKILL.md in ${REPO}/skills/ that has a skill_path in ${REPO}/skills/SKILL_ROUTING_RUNTIME.json, check:

1. Does the SKILL.md reference any crate import that doesn't exist?
2. Does the SKILL.md reference any file path that doesn't exist?
3. Does the SKILL.md reference any command that isn't installed?

Focus on finding BROKEN references — paths/files/commands the SKILL.md tells the agent to use but don't exist.

Also check the SKILL_HEALTH_MANIFEST.json — is it truly empty? Does it need updating?
`, {
  label: "SKILL.md reference audit",
  schema: {
    type: "object",
    properties: {
      skills_checked: { type: "integer" },
      skills_with_broken_refs: { type: "array", items: { type: "object", properties: { skill: { type: "string" }, broken_refs: { type: "array", items: { type: "string" } } } } },
      health_manifest_status: { type: "string" },
      issues: { type: "array", items: { type: "string" } },
    },
    required: ["skills_checked", "skills_with_broken_refs", "issues"],
  },
});

// Synthesis
log("=== SYNTHESIS ===");
const summary = { p1, p2, p3, p4, p5 };

const allIssues = [
  ...(p1.issues || []).map(i => `[Deps] ${i}`),
  ...(p2.issues || []).map(i => `[Fns] ${i}`),
  ...(p3.issues || []).map(i => `[Feat] ${i}`),
  ...(p4.issues || []).map(i => `[CLI] ${i}`),
  ...(p5.issues || []).map(i => `[SKILL] ${i}`),
];

const critical = allIssues.filter(i => /CRIT|P0|断裂|死依赖|unused.*crate/i.test(i));
const high = allIssues.filter(i => /HIGH|P1|not found|broken/i.test(i));
log(`Total issues: ${allIssues.length}`);
if (critical.length) { log(`CRITICAL (${critical.length}):`); critical.forEach(i => log(`  ${i}`)); }
if (high.length) { log(`HIGH (${high.length}):`); high.forEach(i => log(`  ${i}`)); }
log(`Other (${allIssues.length - critical.length - high.length}):`);
allIssues.filter(i => !critical.includes(i) && !high.includes(i)).forEach(i => log(`  ${i}`));

return summary;
