from pathlib import Path
import subprocess


PROJECT_ROOT = Path("/Users/joe/Documents/skill")
GH_FIX_CI_SKILL = PROJECT_ROOT / "skills" / "gh-fix-ci"
GH_ADDRESS_COMMENTS_SKILL = PROJECT_ROOT / "skills" / "gh-address-comments"
RUST_TOOLS_MANIFEST = PROJECT_ROOT / "rust_tools" / "Cargo.toml"
GH_SOURCE_GATE_MANIFEST = PROJECT_ROOT / "rust_tools" / "gh_source_gate_rs" / "Cargo.toml"
GH_SOURCE_GATE_MAIN = PROJECT_ROOT / "rust_tools" / "gh_source_gate_rs" / "src" / "main.rs"
GENERATED_ROUTING_SURFACES = [
    PROJECT_ROOT / "skills" / "SKILL_MANIFEST.json",
    PROJECT_ROOT / "skills" / "SKILL_ROUTING_RUNTIME.json",
    PROJECT_ROOT / "skills" / "SKILL_ROUTING_REGISTRY.md",
    PROJECT_ROOT / "skills" / "SKILL_ROUTING_INDEX.md",
    PROJECT_ROOT / "skills" / "SKILL_APPROVAL_POLICY.json",
]


def test_github_source_gate_python_helpers_are_retired() -> None:
    for skill in (GH_FIX_CI_SKILL, GH_ADDRESS_COMMENTS_SKILL):
        assert not (skill / "scripts").exists()
        assert not list(skill.rglob("*.py"))


def test_github_source_gate_docs_point_to_rust_cli_only() -> None:
    docs = "\n".join(
        path.read_text(encoding="utf-8")
        for skill in (GH_FIX_CI_SKILL, GH_ADDRESS_COMMENTS_SKILL)
        for path in skill.rglob("*.md")
    )
    assert "gh_source_gate_rs" in docs
    assert "gh-source-gate" in docs
    assert "inspect-pr-checks" in docs
    assert "fetch-comments" in docs
    assert "inspect_pr_checks.py" not in docs
    assert "fetch_comments.py" not in docs
    assert "python" not in docs.lower()


def test_generated_routing_surfaces_do_not_reference_retired_python_helpers() -> None:
    generated = "\n".join(path.read_text(encoding="utf-8") for path in GENERATED_ROUTING_SURFACES)
    assert "inspect_pr_checks.py" not in generated
    assert "fetch_comments.py" not in generated
    assert "gh-source-gate" in generated


def test_github_source_gate_rust_cli_is_workspace_member() -> None:
    manifest = RUST_TOOLS_MANIFEST.read_text(encoding="utf-8")
    assert '"gh_source_gate_rs"' in manifest
    assert GH_SOURCE_GATE_MANIFEST.exists()


def test_github_source_gate_rust_cli_owns_both_commands() -> None:
    source = GH_SOURCE_GATE_MAIN.read_text(encoding="utf-8")
    assert "InspectPrChecks(InspectPrChecksArgs)" in source
    assert "FetchComments(FetchCommentsArgs)" in source
    assert "fn inspect_pr_checks(" in source
    assert "fn fetch_comments(" in source
    assert "REVIEW_THREADS_QUERY" in source


def test_github_source_gate_help_lists_commands() -> None:
    result = subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(GH_SOURCE_GATE_MANIFEST),
            "--bin",
            "gh-source-gate",
            "--",
            "--help",
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    assert "inspect-pr-checks" in result.stdout
    assert "fetch-comments" in result.stdout
