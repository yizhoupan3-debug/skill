from __future__ import annotations

import json
import subprocess
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]
IMAGEGEN_SKILL = PROJECT_ROOT / "skills" / "imagegen"
RUST_TOOLS_MANIFEST = PROJECT_ROOT / "rust_tools" / "Cargo.toml"
IMAGEGEN_MANIFEST = PROJECT_ROOT / "rust_tools" / "image_gen_rs" / "Cargo.toml"


def run_imagegen(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(IMAGEGEN_MANIFEST),
            "--",
            *args,
        ],
        check=True,
        capture_output=True,
        text=True,
    )


def run_imagegen_error(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "--manifest-path",
            str(IMAGEGEN_MANIFEST),
            "--",
            *args,
        ],
        check=False,
        capture_output=True,
        text=True,
    )


def parse_concatenated_json(text: str) -> list[dict]:
    decoder = json.JSONDecoder()
    idx = 0
    payloads = []
    while idx < len(text):
        while idx < len(text) and text[idx].isspace():
            idx += 1
        if idx >= len(text):
            break
        payload, idx = decoder.raw_decode(text, idx)
        payloads.append(payload)
    return payloads


def test_imagegen_rust_cli_is_workspace_member() -> None:
    manifest = RUST_TOOLS_MANIFEST.read_text(encoding="utf-8")
    assert '"image_gen_rs"' in manifest


def test_imagegen_skill_docs_point_to_rust_cli_only() -> None:
    docs = "\n".join(
        path.read_text(encoding="utf-8") for path in IMAGEGEN_SKILL.rglob("*.md")
    )
    assert "rust_tools/image_gen_rs" in docs
    assert "scripts/image_gen.py" not in docs
    assert "python \"$IMAGE_GEN\"" not in docs


def test_imagegen_generate_dry_run_uses_responses_tool() -> None:
    result = run_imagegen(
        "generate",
        "--prompt",
        "red square",
        "--use-case",
        "infographic-diagram",
        "--dry-run",
    )
    payload = json.loads(result.stdout)
    assert payload["endpoint"] == "http://127.0.0.1:8318/v1/responses"
    assert payload["model"] == "gpt-5.4"
    assert payload["tools"][0]["type"] == "image_generation"
    assert payload["input"].startswith("Use case: infographic-diagram")
    assert "Primary request: red square" in payload["input"]


def test_imagegen_batch_dry_run_has_single_out_dir_flag(tmp_path: Path) -> None:
    prompts = tmp_path / "prompts.jsonl"
    prompts.write_text(
        '{"prompt":"red square","out":"red.png"}\nblue circle\n',
        encoding="utf-8",
    )
    result = run_imagegen(
        "generate-batch",
        "--input",
        str(prompts),
        "--out-dir",
        str(tmp_path / "out"),
        "--dry-run",
    )
    payloads = parse_concatenated_json(result.stdout)
    assert len(payloads) == 2
    assert payloads[0]["outputs"][0].endswith("/out/red.png")
    assert payloads[1]["outputs"][0].endswith("/out/002-blue-circle.png")


def test_imagegen_dry_run_does_not_create_output_dirs(tmp_path: Path) -> None:
    out_dir = tmp_path / "preview-only"
    run_imagegen(
        "generate",
        "--prompt",
        "preview",
        "--out-dir",
        str(out_dir),
        "--dry-run",
    )
    assert not out_dir.exists()


def test_imagegen_rejects_invalid_output_compression() -> None:
    result = run_imagegen_error(
        "generate",
        "--prompt",
        "red square",
        "--output-compression",
        "101",
        "--dry-run",
    )
    assert result.returncode != 0
    assert "output-compression must be between 0 and 100" in result.stderr


def test_imagegen_batch_slugs_non_ascii_prompts(tmp_path: Path) -> None:
    prompts = tmp_path / "prompts.jsonl"
    prompts.write_text("一只猫\n", encoding="utf-8")
    result = run_imagegen(
        "generate-batch",
        "--input",
        str(prompts),
        "--out-dir",
        str(tmp_path / "out"),
        "--dry-run",
    )
    payload = json.loads(result.stdout)
    assert payload["outputs"][0].endswith("/out/001-image.png")
