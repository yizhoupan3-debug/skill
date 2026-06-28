export const meta = {
  name: "deep-cross-crate-call-audit",
  description: "跨 crate 调用深度核查：区分间接调用（hook 注册/函数指针/re-export）与真正死函数",
  phases: [
    { title: "Phase 1: Hook & FnPtr Registration" },
    { title: "Phase 2: Re-export Chain" },
    { title: "Phase 3: Trait Impl & Signal Registry" },
    { title: "Phase 4: True Dead Pub Fn" },
    { title: "Phase 5: Prioritized Fix Plan" },
    { title: "Phase 6: Apply Top Fixes" },
  ],
};

const REPO = "/Users/joe/Developer/skill";

phase("Phase 1: Hook & FnPtr Registration");
log("Checking every 'dead' pub fn is actually registered as a hook/function pointer...");

const p1 = await agent(`Read the following files and extract ALL function-pointer registration patterns. For each registration found, check which function it points to:

1. ${REPO}/core/host-projection/src/hooks.rs or mod.rs — search for patterns like:
   - "set_runtime_hooks"
   - "RuntimeHooks {"
   - "research_tool_dispatch"
   - "get_research_tool_dispatch"
   - Any assignment of fn pointers to a struct field

2. ${REPO}/core/runtime-core/src/lib.rs — the init_hooks() function registers:
   - routing_engine::hooks::register_hooks(...)
   - routing_core::config_hooks::register_routing_config_hooks(...)
   - host_projection::hooks::set_runtime_hooks(RuntimeHooks { ... })

   List every function name that appears inside these hook registrations.

3. ${REPO}/core/research-harness/src/hooks/init.rs or mod.rs — search for:
   - "set_research_tool_dispatch"
   - "register_hooks"
   - Any OnceLock assignment that stores a fn pointer

4. ${REPO}/core/framework-kernel/src/runtime_hooks.rs — find what struct fields expect fn pointers.

5. Search across ${REPO}/core/ for patterns like:
   - "OnceLock.*=.*|"
   - "hooks\\.register"
   - "set_.*hook"
   - "register_.*hook"

Report for each registration-site: which function name is stored, which crate it lives in, and whether that function appeared in the 'dead pub fn' list from the previous audit.

The goal: explain how many of the 747 'dead' functions are actually alive via indirect registration.
`, {
  label: "Hook and fn ptr registration analysis",
  schema: {
    type: "object",
    properties: {
      hook_registration_files_scanned: { type: "integer" },
      fn_ptr_registrations_found: { type: "integer" },
      hook_registered_fns: { type: "array", items: { type: "object", properties: { fn_name: { type: "string" }, registered_via: { type: "string" }, previously_dead: { type: "boolean" } } } },
      still_dead_after_hook_check: { type: "array", items: { type: "string" } },
      issues: { type: "array", items: { type: "string" } },
    },
    required: ["fn_ptr_registrations_found", "hook_registered_fns", "still_dead_after_hook_check", "issues"],
  },
});

phase("Phase 2: Re-export Chain");
log("Checking re-export chains that make indirect callers live...");

const p2 = await agent(`Scan ${REPO}/core/ for re-export patterns that would make a function callable through a different crate path.

Specifically check these files which re-export large amounts of functionality:

1. ${REPO}/core/runtime-core/src/lib.rs — it does "pub use core_state::*" or similar re-exports
2. ${REPO}/core/host-projection/src/lib.rs
3. ${REPO}/core/framework-kernel/src/lib.rs

For each previously-identified 'dead' pub fn, check:
- Is it re-exported by another crate via 'pub use'?
- If so, does any external crate import that re-export and call it?

Example: a function defined in core-state/src/task_state.rs as 'pub fn' but imported by runtime-core as 'pub use core_state::task_state::*' and then called by router-rs via 'runtime_core::task_state::resolve_task_view' — this is an indirect but live call.

Report every dead-function-that's-actually-alive-through-re-export.
`, {
  label: "Re-export chain analysis",
  schema: {
    type: "object",
    properties: {
      re_export_files_scanned: { type: "integer" },
      fns_rescued_by_reexport: { type: "array", items: { type: "object", properties: { fn_name: { type: "string" }, defined_in: { type: "string" }, re_exported_by: { type: "string" }, called_from: { type: "string" } } } },
      issues: { type: "array", items: { type: "string" } },
    },
    required: ["re_export_files_scanned", "fns_rescued_by_reexport", "issues"],
  },
});

phase("Phase 3: Trait Impl & Signal Registry");
log("Checking trait implementations and signal registry indirect calls...");

const p3 = await agent(`Check the routing engine's signal system for indirect function calls:

1. Read ${REPO}/core/routing-engine/src/route/signals/ to understand the signal scoring system
   - How are signal functions registered? Are they collected into a Vec<Box<dyn Fn>> or similar?
   - If there's a signal registry, list every function that gets registered

2. Check trait implementations: search for "impl.*for.*Handler" or "impl.*for.*Check" or similar trait impl patterns:
   ${REPO}/core/research-harness/src/ — does it implement ToolHandler trait?
   ${REPO}/core/host-projection/src/ — how are handler groups like RoutingTools, GoalTools etc implemented?
   They all "impl ToolHandler for RoutingTools" — this is a trait implementation, which means dispatch() is called through dynamic dispatch.

3. For each 'pub fn' that appeared dead but is actually part of:
   - A trait impl
   - A signal registry entry
   - A closure stored in a Vec or HashMap
   Mark it as 'trait-indirect-live'.

Also check the RoutingTools/GoalTools/CloseoutTools/etc handler structs — their dispatch() methods call individual tool_* functions. Each tool_* function called from a dispatch() match arm is indirect-live.
`, {
  label: "Trait impl and signal registry",
  schema: {
    type: "object",
    properties: {
      trait_impls_found: { type: "integer" },
      signal_registry_fns: { type: "array", items: { type: "string" } },
      fns_rescued_by_trait: { type: "array", items: { type: "object", properties: { fn_name: { type: "string" }, called_via: { type: "string" } } } },
      issues: { type: "array", items: { type: "string" } },
    },
    required: ["trait_impls_found", "fns_rescued_by_trait", "issues"],
  },
});

phase("Phase 4: True Dead Pub Fn");
log("After accounting for indirect calls, report what's truly dead...");

const p4 = await agent(`Now synthesize all findings from phases 1-3 to produce the final 'truly dead' list.

Cross-reference:
- From Phase 1: functions that are hook-registered → REMOVE from dead list
- From Phase 2: functions that are re-exported and called externally → REMOVE
- From Phase 3: functions that are trait implementations or signal registrations → REMOVE
- All other 'pub fn' with zero grep call sites outside their definition crate → potential TRUE DEAD

For each truly dead fn, report:
- crate location
- why it's truly dead (no hook, no re-export, no trait impl, no signal registry)
- impact (is it a test helper? internal dispatch? genuinely unused API?)
- recommendation: pub(crate) or delete
`, {
  label: "True dead fn synthesis",
  schema: {
    type: "object",
    properties: {
      total_checked: { type: "integer" },
      rescued_by_hook: { type: "integer" },
      rescued_by_reexport: { type: "integer" },
      rescued_by_trait: { type: "integer" },
      truly_dead: { type: "array", items: { type: "object", properties: { fn_name: { type: "string" }, crate: { type: "string" }, recommendation: { type: "string" } } } },
      issues: { type: "array", items: { type: "string" } },
    },
    required: ["total_checked", "rescued_by_hook", "rescued_by_reexport", "rescued_by_trait", "truly_dead", "issues"],
  },
});

phase("Phase 5: Prioritized Fix Plan");
log("Producing ranked fix plan...");

const p5 = await agent(`Based on phase 4 output, produce a prioritized fix plan.

Rank by impact:
- P0: Functions that are ALWAYS called (hook-registered from init_hooks) but marked as pub instead of pub(crate) making the API surface misleading
- P1: Functions that are INTERNALLY called within the crate but exported as pub (these should be pub(crate))
- P2: Functions that are TEST-ONLY (these should be #[cfg(any(test, feature = "test-support"))] gated)
- P3: Genuinely unused functions (should be deleted, but verify first)
- Info: Functions that LOOK dead but are called via indirect mechanisms we documented

For each crate, provide:
- Current pub fn count
- Recommended pub fn count (after pub→pub(crate) changes)
- Estimated code debloat %

Be conservative: only recommend pub→pub(crate) when there is HIGH confidence that no external crate needs the function. When uncertain, leave as pub and mark as 'defer'.
`, {
  label: "Prioritized fix plan",
  schema: {
    type: "object",
    properties: {
      P0_fixes: { type: "array", items: { type: "object", properties: { fn_name: { type: "string" }, crate: { type: "string" }, action: { type: "string" }, reason: { type: "string" } } } },
      P1_fixes: { type: "array", items: { type: "object", properties: { fn_name: { type: "string" }, crate: { type: "string" }, action: { type: "string" } } } },
      P2_fixes: { type: "array", items: { type: "object", properties: { fn_name: { type: "string" }, crate: { type: "string" }, action: { type: "string" } } } },
      per_crate_summary: { type: "array", items: { type: "object", properties: { crate_name: { type: "string" }, current_pub: { type: "integer" }, recommended_pub: { type: "integer" } } } },
      issues: { type: "array", items: { type: "string" } },
    },
    required: ["P0_fixes", "P1_fixes", "P2_fixes", "per_crate_summary", "issues"],
  },
});

// Apply top fixes
phase("Phase 6: Apply Top P0 Fixes");
log("Applying the most impactful pub→pub(crate) fixes...");

// We only apply P0 fixes here (hook-registered functions that should be pub(crate))
// The synthesis passes the full plan back to the user
const p6 = p5; // Pass through — user decides what to apply

log("=== SYNTHESIS ===");
const summary = { p1, p2, p3, p4, p5 };

const trulyDead = (p4.truly_dead || []);
const rescuedHooks = p1.hook_registered_fns.filter(f => f.previously_dead).length;
const rescuedReexport = (p2.fns_rescued_by_reexport || []).length;
const rescuedTrait = (p3.fns_rescued_by_trait || []).length;

log(`Pub fns checked: ${p4.total_checked}`);
log(`Rescued by hook registration: ${rescuedHooks}`);
log(`Rescued by re-export: ${rescuedReexport}`);
log(`Rescued by trait impl: ${rescuedTrait}`);
log(`Truly dead: ${trulyDead.length}`);

if (trulyDead.length > 0) {
  log("=== TRULY DEAD FUNCTIONS ===");
  trulyDead.forEach(f => log(`  ${f.crate}: ${f.fn_name} → ${f.recommendation}`));
}

log("=== P0 FIXES ===");
(p5.P0_fixes || []).forEach(f => log(`  ${f.crate}: ${f.fn_name} → ${f.action} [${f.reason}]`));

return summary;
