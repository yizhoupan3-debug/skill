from pathlib import Path
import json

PACKAGE_TEMPLATE = Path("/Users/joe/Documents/skill/skills/ppt-pptx/assets/package.template.json")
TOOL_RUNNER = Path("/Users/joe/Documents/skill/skills/ppt-pptx/scripts/pptx_tool.js")
PACKAGE_JSON = Path("/Users/joe/Documents/skill/skills/ppt-pptx/package.json")
SCRIPTS_DIR = Path("/Users/joe/Documents/skill/skills/ppt-pptx/scripts")


def test_tool_runner_is_present() -> None:
    assert TOOL_RUNNER.exists()


def test_package_template_uses_rust_tool_runner() -> None:
    scripts = json.loads(PACKAGE_TEMPLATE.read_text(encoding="utf-8"))["scripts"]
    for name, command in scripts.items():
        if name == "build":
            continue
        assert command.startswith("node scripts/pptx_tool.js"), (name, command)


def test_skill_package_uses_node_smoke_test() -> None:
    scripts = json.loads(PACKAGE_JSON.read_text(encoding="utf-8"))["scripts"]
    assert scripts["smoke:test"] == "node scripts/smoke_test.js"


def test_skill_scripts_are_no_longer_python() -> None:
    assert not list(SCRIPTS_DIR.glob("*.py"))
