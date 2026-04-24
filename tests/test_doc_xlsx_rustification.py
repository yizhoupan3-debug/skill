from pathlib import Path
import subprocess

PROJECT_ROOT = Path("/Users/joe/Documents/skill")
DOC_SKILL = PROJECT_ROOT / "skills" / "doc"
XLSX_SKILL = PROJECT_ROOT / "skills" / "xlsx"
OOXML_MANIFEST = PROJECT_ROOT / "rust_tools" / "ooxml_parser_rs" / "Cargo.toml"
OOXML_MAIN = PROJECT_ROOT / "rust_tools" / "ooxml_parser_rs" / "src" / "main.rs"


def test_doc_and_xlsx_skills_have_no_python_scripts() -> None:
    assert not list(DOC_SKILL.rglob("*.py"))
    assert not list(XLSX_SKILL.rglob("*.py"))


def test_doc_and_xlsx_skill_docs_point_to_rust_tooling() -> None:
    docs = "\n".join(
        path.read_text(encoding="utf-8")
        for root in (DOC_SKILL, XLSX_SKILL)
        for path in root.rglob("*.md")
    )
    forbidden = [
        "openpyxl",
        "pandas",
        "python-docx",
        "pdf2image",
        "render_docx.py",
        "render_xlsx.py",
        "inspect_xlsx.py",
    ]
    for token in forbidden:
        assert token not in docs
    assert "ooxml_parser_rs" in docs
    assert "render-docx" in docs
    assert "render-xlsx" in docs
    assert " -- docx <docx>" in docs


def test_doc_and_xlsx_agent_prompts_are_rust_first() -> None:
    prompts = "\n".join(
        [
            (DOC_SKILL / "agents" / "openai.yaml").read_text(encoding="utf-8"),
            (XLSX_SKILL / "agents" / "openai.yaml").read_text(encoding="utf-8"),
        ]
    )
    assert "Rust-first" in prompts
    assert "Rust OOXML CLI" in prompts


def test_ooxml_rust_cli_owns_docx_and_xlsx_render_commands() -> None:
    source = OOXML_MAIN.read_text(encoding="utf-8")
    assert "Docx { input, json }" in source
    assert "RenderXlsx(RenderXlsxArgs)" in source
    assert "RenderDocx(RenderDocxArgs)" in source
    assert "fn inspect_docx(" in source
    assert "fn render_xlsx(" in source
    assert "fn render_docx(" in source


def test_ooxml_cli_help_lists_docx_and_xlsx_render_commands() -> None:
    result = subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(OOXML_MANIFEST),
            "--",
            "--help",
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    assert "docx" in result.stdout
    assert "render-docx" in result.stdout
    assert "render-xlsx" in result.stdout
