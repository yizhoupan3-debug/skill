from pathlib import Path
import json
import subprocess

PACKAGE_TEMPLATE = Path("/Users/joe/Documents/skill/skills/ppt-pptx/assets/package.template.json")
PACKAGE_JSON = Path("/Users/joe/Documents/skill/skills/ppt-pptx/package.json")
SCRIPTS_DIR = Path("/Users/joe/Documents/skill/skills/ppt-pptx/scripts")
RUST_MANIFEST = Path("/Users/joe/Documents/skill/rust_tools/pptx_tool_rs/Cargo.toml")
RUST_MAIN = Path("/Users/joe/Documents/skill/rust_tools/pptx_tool_rs/src/main.rs")
RUST_DIRECT_BIN = Path("/Users/joe/Documents/skill/rust_tools/pptx_tool_rs/src/bin/ppt.rs")


def test_package_template_uses_rust_tool_runner() -> None:
    scripts = json.loads(PACKAGE_TEMPLATE.read_text(encoding="utf-8"))["scripts"]
    for name, command in scripts.items():
        if name == "build":
            continue
        assert command.startswith("ppt "), (name, command)


def test_skill_package_uses_node_smoke_test() -> None:
    scripts = json.loads(PACKAGE_JSON.read_text(encoding="utf-8"))["scripts"]
    assert scripts["smoke:test"] == "node scripts/smoke_test.js"


def test_skill_scripts_are_no_longer_python() -> None:
    assert not list(SCRIPTS_DIR.glob("*.py"))


def test_rust_manifest_exposes_direct_ppt_cli() -> None:
    manifest = RUST_MANIFEST.read_text(encoding="utf-8")
    assert 'name = "ppt"' in manifest
    assert 'path = "src/bin/ppt.rs"' in manifest
    assert RUST_DIRECT_BIN.exists()


def test_rust_cli_owns_workspace_and_outline_commands() -> None:
    source = RUST_MAIN.read_text(encoding="utf-8")
    assert "Init(InitArgs)" in source
    assert "Outline(OutlineArgs)" in source
    assert "fn init_workspace(" in source


def test_direct_ppt_cli_help_lists_authoring_commands() -> None:
    result = subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(RUST_MANIFEST),
            "--bin",
            "ppt",
            "--",
            "--help",
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    assert "init" in result.stdout
    assert "outline" in result.stdout
