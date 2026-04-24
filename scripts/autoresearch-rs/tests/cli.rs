use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_autoresearch-rs"))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("autoresearch crate should live under scripts/")
        .to_path_buf()
}

fn temp_base(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "autoresearch-rs-{name}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

fn run_ctl(args: Vec<String>) -> String {
    let output = Command::new(bin())
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("failed to run autoresearch-rs");
    if !output.status.success() {
        panic!(
            "autoresearch-rs failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8(output.stdout).unwrap()
}

fn load_state(workspace: &Path) -> Value {
    serde_yaml::from_str(&fs::read_to_string(workspace.join("research-state.yaml")).unwrap())
        .unwrap()
}

fn text(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path).unwrap()
}

fn s(value: impl Into<String>) -> String {
    value.into()
}

#[test]
fn full_loop_records_scientific_sense_fields() {
    let tmp = temp_base("full-loop");
    run_ctl(vec![
        s("init"),
        s("--project"),
        s("rust-project"),
        s("--question"),
        s("Can Rust own autoresearch?"),
        s("--dir"),
        tmp.display().to_string(),
        s("--mode"),
        s("quick"),
    ]);
    let root = tmp.join("rust-project");
    let state = load_state(&root);
    assert_eq!(state["schema_version"].as_i64(), Some(4));
    assert_eq!(state["novelty_gate"]["status"].as_str(), Some("pending"));

    run_ctl(vec![
        s("draft-claims"),
        s("--workspace"),
        root.display().to_string(),
        s("--count"),
        s("4"),
    ]);
    let state = load_state(&root);
    assert_eq!(
        state["novelty_gate"]["draft_claims"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert!(text(root.join("literature/NOVELTY_SEARCH_PLAN.md"))
        .contains("recommended first search target"));
    assert!(text(root.join("literature/EXTERNAL_RESEARCH.md")).contains("research-claim"));

    run_ctl(vec![
        s("compare-claim"),
        s("--workspace"),
        root.display().to_string(),
        s("--claim"),
        s("Rust controller reduces orchestration drift"),
        s("--axis"),
        s("workflow"),
        s("--closest-prior-work"),
        s("Python controller"),
        s("--overlap"),
        s("medium"),
        s("--difference"),
        s("Rust is the primary control plane."),
        s("--confidence"),
        s("high"),
        s("--verdict"),
        s("defensible"),
        s("--claim-id"),
        s("C1"),
    ]);
    run_ctl(vec![
        s("set-novelty-gate"),
        s("--workspace"),
        root.display().to_string(),
        s("--status"),
        s("passed"),
        s("--decision"),
        s("Proceed"),
    ]);
    run_ctl(vec![
        s("add-hypothesis"),
        s("--workspace"),
        root.display().to_string(),
        s("--claim"),
        s("Rust controller can run the loop"),
        s("--prediction"),
        s("Run and reflection files are materialized"),
        s("--mechanism"),
        s("A single Rust state transition path should reduce drift between state and markdown projections."),
        s("--falsifiable-prediction"),
        s("If the projection layer diverges, the state and generated files will disagree."),
        s("--success-threshold"),
        s("State, run file, reflection file, and ledger all agree on the same run id."),
        s("--stop-condition"),
        s("Stop if the controller cannot keep state and generated files aligned."),
        s("--baseline"),
        s("Manual markdown edits without controller synchronization"),
        s("--confounder"),
        s("A stale generated file could appear to pass without being refreshed."),
        s("--negative-signal"),
        s("Run ledger contains the event but the run file is missing."),
        s("--minimal-test"),
        s("Record one run and one reflection, then inspect state plus generated artifacts."),
        s("--priority"),
        s("high"),
        s("--id"),
        s("rust-loop"),
    ]);
    run_ctl(vec![
        s("record-run"),
        s("--workspace"),
        root.display().to_string(),
        s("--hypothesis-id"),
        s("rust-loop"),
        s("--outcome"),
        s("exploratory"),
        s("--summary"),
        s("Rust recorded the run."),
        s("--metric-name"),
        s("files"),
        s("--metric-value"),
        s("2"),
        s("--sanity-check"),
        s("Generated run and reflection files both exist."),
        s("--baseline-result"),
        s("Manual baseline has no append-only ledger event."),
        s("--rules-in"),
        s("Controller-owned transitions can keep state and projections aligned for the first run."),
        s("--rules-out"),
        s("This does not prove multi-branch concurrency safety."),
        s("--alternative-explanation"),
        s("The pass could come from static templates rather than correct state transitions."),
        s("--threat"),
        s("The test uses a tiny synthetic workspace."),
        s("--interpretation"),
        s("The result supports the controller mechanism, but only for the single-branch path."),
        s("--finding"),
        s("For a single active branch, controller-owned transitions preserve state and generated artifacts."),
        s("--decision-delta"),
        s("Use the Rust controller path for future single-branch run recording instead of manual markdown edits."),
        s("--reuse-note"),
        s("Reuse this as a smoke-test expectation for state/projection alignment."),
        s("--applies-to"),
        s("single active hypothesis workspaces"),
        s("--does-not-apply-to"),
        s("parallel multi-branch writes"),
    ]);
    run_ctl(vec![
        s("reflect"),
        s("--workspace"),
        root.display().to_string(),
        s("--hypothesis-id"),
        s("rust-loop"),
        s("--direction"),
        s("DEEPEN"),
        s("--reason"),
        s("The loop is working."),
        s("--next-step"),
        s("Add broader coverage."),
    ]);

    let state = load_state(&root);
    assert_eq!(state["active_hypothesis"].as_str(), Some("rust-loop"));
    assert_eq!(state["hypotheses"][0]["status"].as_str(), Some("active"));
    assert!(root.join("experiments/rust-loop/run-001.md").is_file());
    assert!(root
        .join("experiments/rust-loop/run-001-reflection.md")
        .is_file());
    assert!(state["hypotheses"][0]["mechanism"]
        .as_str()
        .unwrap()
        .starts_with("A single Rust state transition"));
    assert_eq!(
        state["run_history"][0]["rules_out"],
        serde_json::json!(["This does not prove multi-branch concurrency safety."])
    );
    assert!(
        text(root.join("experiments/rust-loop/protocol.md")).contains("## Baselines / Controls")
    );
    assert!(
        text(root.join("experiments/rust-loop/run-001.md")).contains("## Alternative Explanations")
    );
    let resume = run_ctl(vec![
        s("resume"),
        s("--workspace"),
        root.display().to_string(),
    ]);
    assert!(
        resume.contains("latest_rules_out: This does not prove multi-branch concurrency safety.")
    );
    assert!(resume.contains("latest_finding: For a single active branch"));
    assert!(resume.contains("latest_direction: DEEPEN"));
    let ledger = text(root.join("run-ledger.jsonl"));
    assert!(ledger
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .any(|event| event["kind"].as_str() == Some("run.recorded")));
}

#[test]
fn can_record_external_research_from_arxiv() {
    let tmp = temp_base("external-research");
    run_ctl(vec![
        s("init"),
        s("--project"),
        s("external-research"),
        s("--question"),
        s("Can retrieval augmented generation improve citation grounded research?"),
        s("--dir"),
        tmp.display().to_string(),
    ]);
    let root = tmp.join("external-research");
    run_ctl(vec![
        s("draft-claims"),
        s("--workspace"),
        root.display().to_string(),
        s("--count"),
        s("2"),
    ]);
    run_ctl(vec![
        s("research-claim"),
        s("--workspace"),
        root.display().to_string(),
        s("--claim-id"),
        s("C1"),
        s("--source"),
        s("arxiv"),
        s("--limit"),
        s("1"),
    ]);
    let state = load_state(&root);
    assert_eq!(state["external_research"].as_array().unwrap().len(), 1);
    assert!(state["external_research"][0]["query"]
        .as_str()
        .is_some_and(|query| !query.is_empty()));
    assert!(!state["external_research"][0]["results"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(
        text(root.join("literature/EXTERNAL_RESEARCH.md")).contains("Managed External Research")
    );
}

#[test]
fn batch_research_and_gate_recommendation() {
    let tmp = temp_base("batch-research");
    run_ctl(vec![
        s("init"),
        s("--project"),
        s("batch-research"),
        s("--question"),
        s("Can retrieval augmented generation improve citation grounded research?"),
        s("--dir"),
        tmp.display().to_string(),
    ]);
    let root = tmp.join("batch-research");
    run_ctl(vec![
        s("draft-claims"),
        s("--workspace"),
        root.display().to_string(),
        s("--count"),
        s("2"),
    ]);
    run_ctl(vec![
        s("research-all"),
        s("--workspace"),
        root.display().to_string(),
        s("--source"),
        s("arxiv"),
        s("--limit"),
        s("1"),
        s("--max-claims"),
        s("2"),
    ]);
    let state = load_state(&root);
    assert_eq!(state["external_research"].as_array().unwrap().len(), 2);

    let recommendation = run_ctl(vec![
        s("gate-from-research"),
        s("--workspace"),
        root.display().to_string(),
    ]);
    assert!(recommendation.contains("recommended_status: pending"));
    assert!(recommendation.contains("reviewed_claims:"));

    for (index, axis) in [("C1", "method"), ("C2", "task")] {
        let claim_index = index.trim_start_matches('C').parse::<usize>().unwrap() - 1;
        run_ctl(vec![
            s("compare-claim"),
            s("--workspace"),
            root.display().to_string(),
            s("--claim"),
            state["novelty_gate"]["draft_claims"][claim_index]["claim"]
                .as_str()
                .unwrap()
                .to_string(),
            s("--axis"),
            s(axis),
            s("--closest-prior-work"),
            s("Nearest retrieved arXiv result"),
            s("--overlap"),
            s("medium"),
            s("--difference"),
            s("The scoped contribution is narrower."),
            s("--confidence"),
            s("medium"),
            s("--verdict"),
            s("defensible"),
            s("--claim-id"),
            s(index),
        ]);
    }
    let applied = run_ctl(vec![
        s("gate-from-research"),
        s("--workspace"),
        root.display().to_string(),
        s("--apply"),
    ]);
    assert!(applied.contains("recommended_status: passed"));
    let state = load_state(&root);
    assert_eq!(state["novelty_gate"]["status"].as_str(), Some("passed"));
}

#[test]
fn scientific_sense_fields_are_projected() {
    let tmp = temp_base("scientific-sense");
    run_ctl(vec![
        s("init"),
        s("--project"),
        s("scientific-sense"),
        s("--question"),
        s("Can a controller distinguish research from tuning?"),
        s("--dir"),
        tmp.display().to_string(),
    ]);
    let root = tmp.join("scientific-sense");
    run_ctl(vec![
        s("set-novelty-gate"),
        s("--workspace"),
        root.display().to_string(),
        s("--status"),
        s("passed"),
        s("--decision"),
        s("Proceed"),
    ]);
    run_ctl(vec![
        s("add-hypothesis"),
        s("--workspace"),
        root.display().to_string(),
        s("--id"),
        s("mechanism-first"),
        s("--claim"),
        s("Mechanism-first protocols produce more useful negative results."),
        s("--prediction"),
        s("The run record will name what the result rules out."),
        s("--mechanism"),
        s("Making the causal story explicit forces the run to test an explanation, not a knob."),
        s("--falsifiable-prediction"),
        s("If no rival explanation is ruled out, the run is just tuning."),
        s("--success-threshold"),
        s("The run record contains rules in, rules out, and an alternative explanation."),
        s("--stop-condition"),
        s("Stop if only a metric changes."),
        s("--baseline"),
        s("A parameter sweep with no baseline explanation"),
        s("--confounder"),
        s("Template text could be mistaken for a real interpretation."),
        s("--negative-signal"),
        s("No rules-out statement is recorded."),
        s("--minimal-test"),
        s("One synthetic run record with interpretation fields."),
    ]);
    run_ctl(vec![
        s("record-run"),
        s("--workspace"),
        root.display().to_string(),
        s("--hypothesis-id"),
        s("mechanism-first"),
        s("--outcome"),
        s("exploratory"),
        s("--summary"),
        s("Mechanism fields were captured."),
        s("--metric-name"),
        s("fields"),
        s("--metric-value"),
        s("6"),
        s("--sanity-check"),
        s("State contains the new fields."),
        s("--baseline-result"),
        s("The tuning-only baseline has no rival explanation."),
        s("--rules-in"),
        s("Explicit mechanisms make run interpretation auditable."),
        s("--rules-out"),
        s("A metric-only record is sufficient for research synthesis."),
        s("--alternative-explanation"),
        s("The improvement may come from stricter templates rather than better research reasoning."),
        s("--threat"),
        s("This is a structural test, not a live scientific experiment."),
        s("--interpretation"),
        s("The control plane now preserves scientific reasoning hooks around each run."),
        s("--finding"),
        s("Mechanism-first records are reusable when they state the scope and decision effect."),
        s("--decision-delta"),
        s("Prefer structured findings over chronological run narration."),
        s("--reuse-note"),
        s("Read the reusable finding first, then inspect metrics only if the scope matches."),
        s("--applies-to"),
        s("future autoresearch run summaries"),
        s("--does-not-apply-to"),
        s("raw benchmark logs without a hypothesis"),
    ]);

    let state = load_state(&root);
    let hypothesis = &state["hypotheses"][0];
    let run = &state["run_history"][0];
    assert!(hypothesis["mechanism"]
        .as_str()
        .unwrap()
        .starts_with("Making the causal story explicit"));
    assert_eq!(
        hypothesis["baselines"],
        serde_json::json!(["A parameter sweep with no baseline explanation"])
    );
    assert_eq!(
        run["rules_in"],
        serde_json::json!(["Explicit mechanisms make run interpretation auditable."])
    );
    assert!(run["alternative_explanations"][0]
        .as_str()
        .unwrap()
        .starts_with("The improvement may come"));
    assert!(run["finding"]
        .as_str()
        .unwrap()
        .starts_with("Mechanism-first records are reusable"));

    let protocol = text(root.join("experiments/mechanism-first/protocol.md"));
    let run_record = text(root.join("experiments/mechanism-first/run-001.md"));
    let findings = text(root.join("findings.md"));
    assert!(protocol.contains("## Proposed Mechanism"));
    assert!(protocol.contains("A parameter sweep with no baseline explanation"));
    assert!(run_record.contains("## Reusable Finding"));
    assert!(run_record.contains("## Reuse Scope"));
    assert!(run_record.contains("## Threats To Interpretation"));
    assert!(run_record.contains("The control plane now preserves scientific reasoning hooks"));
    assert!(findings.contains("### Reuse Notes"));
    assert!(findings.contains("Read the reusable finding first"));
    assert!(findings.contains("### Alternative Explanations To Clear"));
    let reuse_index = text(root.join("findings-reuse-index.md"));
    assert!(reuse_index.contains("Mechanism-first records are reusable"));
    assert!(reuse_index.contains("raw benchmark logs without a hypothesis"));
}

#[test]
fn can_annotate_older_run_for_reuse() {
    let tmp = temp_base("annotate-run");
    run_ctl(vec![
        s("init"),
        s("--project"),
        s("annotate-run"),
        s("--question"),
        s("Can old run logs become reusable findings?"),
        s("--dir"),
        tmp.display().to_string(),
    ]);
    let root = tmp.join("annotate-run");
    run_ctl(vec![
        s("set-novelty-gate"),
        s("--workspace"),
        root.display().to_string(),
        s("--status"),
        s("passed"),
        s("--decision"),
        s("Proceed"),
    ]);
    run_ctl(vec![
        s("add-hypothesis"),
        s("--workspace"),
        root.display().to_string(),
        s("--id"),
        s("old-log"),
        s("--claim"),
        s("Old logs become useful when annotated with scope."),
    ]);
    run_ctl(vec![
        s("record-run"),
        s("--workspace"),
        root.display().to_string(),
        s("--hypothesis-id"),
        s("old-log"),
        s("--outcome"),
        s("exploratory"),
        s("--summary"),
        s("A chronological note was recorded."),
    ]);
    let next = run_ctl(vec![
        s("next"),
        s("--workspace"),
        root.display().to_string(),
    ]);
    assert!(next.contains("annotate-run --run-id run-001"));
    let audit = run_ctl(vec![
        s("audit-reuse"),
        s("--workspace"),
        root.display().to_string(),
    ]);
    assert!(audit.contains("missing_annotations: 1"));
    assert!(audit.contains("run-001: missing finding, decision_delta, reuse_note"));

    run_ctl(vec![
        s("annotate-run"),
        s("--workspace"),
        root.display().to_string(),
        s("--run-id"),
        s("run-001"),
        s("--finding"),
        s("Old logs are reusable only after the finding and scope are explicit."),
        s("--decision-delta"),
        s("Backfill reusable fields before citing old results."),
        s("--reuse-note"),
        s("Use this as the migration path for legacy run records."),
        s("--applies-to"),
        s("legacy autoresearch workspaces"),
        s("--does-not-apply-to"),
        s("unverified external notes"),
    ]);
    let state = load_state(&root);
    assert_eq!(
        state["run_history"][0]["finding"].as_str(),
        Some("Old logs are reusable only after the finding and scope are explicit.")
    );
    let reuse_index = text(root.join("findings-reuse-index.md"));
    assert!(reuse_index.contains("Backfill reusable fields before citing old results."));
    assert!(reuse_index.contains("legacy autoresearch workspaces"));
    let audit = run_ctl(vec![
        s("audit-reuse"),
        s("--workspace"),
        root.display().to_string(),
        s("--apply"),
    ]);
    assert!(audit.contains("missing_annotations: 0"));
}
