#!/usr/bin/env python3
"""
smoke_test.py

End-to-end smoke test for the ppt-pptx skill.

Verifies:
- outline -> deck.js -> deck.pptx
- template -> deck.pptx
- sample deck.js -> deck.pptx
- render, overflow, font, and structure checks where applicable
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
ASSETS = ROOT / "assets"
EXAMPLES = ROOT / "examples"

NODE_DEPS = [
    "pptxgenjs",
    "skia-canvas",
    "linebreak",
    "fontkit",
    "prismjs",
    "mathjax-full",
    "js-yaml",
]

RUST_TOOL_MANIFEST = ROOT.parents[1] / "rust_tools" / "pptx_tool_rs" / "Cargo.toml"


def rust_tool_env() -> dict[str, str]:
    target_root = ROOT.parents[1] / "rust_tools" / "target" / "debug"
    binary = target_root / "pptx_tool_rs"
    env = os.environ.copy()
    env["PPT_PPTX_RUST_TOOL_BIN"] = str(binary)
    env["PPT_PPTX_RUST_TOOL_MANIFEST"] = str(RUST_TOOL_MANIFEST)
    return env


def officecli_available() -> bool:
    return shutil.which("officecli") is not None


def run(
    cmd: list[str],
    cwd: Path,
    label: str,
    *,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    proc = subprocess.run(
        cmd,
        cwd=str(cwd),
        text=True,
        capture_output=True,
        env=env,
    )
    if proc.returncode != 0:
        raise RuntimeError(
            f"{label} failed\n"
            f"cmd: {' '.join(cmd)}\n"
            f"stdout:\n{proc.stdout}\n"
            f"stderr:\n{proc.stderr}"
        )
    return proc


def copy_common_python_tools(dest: Path) -> None:
    for name in [
        "rust_bridge.py",
        "render_slides.py",
        "slides_test.py",
        "detect_font.py",
        "extract_pptx_structure.py",
        "sanitize_pptx.py",
    ]:
        shutil.copy2(SCRIPTS / name, dest / name)


def npm_bootstrap(dest: Path) -> None:
    run(["npm", "init", "-y"], dest, "npm init")
    run(["npm", "install", *NODE_DEPS], dest, "npm install")


def count_pngs(path: Path) -> int:
    return len(list(path.glob("*.png")))


def officecli_doctor(workdir: Path) -> dict | None:
    if not officecli_available():
        return None
    proc = run(
        [sys.executable, str(SCRIPTS / "officecli_bridge.py"), "doctor", "deck.pptx", "--json"],
        workdir,
        "officecli doctor",
    )
    return json.loads(proc.stdout)


def scenario_outline(root: Path) -> dict:
    workdir = root / "outline"
    workdir.mkdir(parents=True, exist_ok=True)

    shutil.copy2(EXAMPLES / "outline_overload.yaml", workdir / "outline.yaml")
    shutil.copy2(SCRIPTS / "outline_to_deck.js", workdir / "outline_to_deck.js")
    shutil.copytree(ASSETS / "pptxgenjs_helpers", workdir / "pptxgenjs_helpers")
    copy_common_python_tools(workdir)
    shutil.copy2(SCRIPTS / "officecli_bridge.py", workdir / "officecli_bridge.py")
    shutil.copy2(SCRIPTS / "hybrid_pipeline.py", workdir / "hybrid_pipeline.py")
    npm_bootstrap(workdir)
    env = rust_tool_env()

    run(["node", "outline_to_deck.js", "outline.yaml", "-o", "deck.js"], workdir, "outline_to_deck")
    run(["node", "deck.js"], workdir, "generated deck.js")
    run(
        [sys.executable, "render_slides.py", "deck.pptx", "--output_dir", "rendered"],
        workdir,
        "render_slides",
        env=env,
    )
    run([sys.executable, "slides_test.py", "deck.pptx"], workdir, "slides_test", env=env)
    run(
        [sys.executable, "detect_font.py", "deck.pptx", "--include-missing", "--include-substituted"],
        workdir,
        "detect_font",
        env=env,
    )
    run(
        [sys.executable, "extract_pptx_structure.py", "deck.pptx", "-o", "structure.json"],
        workdir,
        "extract_structure",
        env=env,
    )
    hybrid = run(
        [sys.executable, "hybrid_pipeline.py", "qa", "deck.pptx", "--rendered-dir", "rendered", "--json"],
        workdir,
        "hybrid qa",
        env=env,
    )
    hybrid_payload = json.loads(hybrid.stdout)

    result = {
        "name": "outline_flow",
        "workdir": str(workdir),
        "deck_exists": (workdir / "deck.pptx").exists(),
        "rendered_pngs": count_pngs(workdir / "rendered"),
        "structure_json": (workdir / "structure.json").exists(),
        "hybrid_render_pngs": hybrid_payload["render"]["png_count"],
    }
    doctor = officecli_doctor(workdir)
    if doctor:
        result["officecli_issue_count"] = doctor["issues"]["count"]
        result["officecli_validation_ok"] = doctor["validation"]["ok"]
    return result


def scenario_template(root: Path) -> dict:
    workdir = root / "template"
    assets_dir = workdir / "assets"
    workdir.mkdir(parents=True, exist_ok=True)
    assets_dir.mkdir(parents=True, exist_ok=True)

    shutil.copy2(ASSETS / "deck.template.js", workdir / "deck.js")
    shutil.copytree(ASSETS / "pptxgenjs_helpers", workdir / "pptxgenjs_helpers")
    copy_common_python_tools(workdir)
    shutil.copy2(SCRIPTS / "officecli_bridge.py", workdir / "officecli_bridge.py")
    shutil.copy2(SCRIPTS / "hybrid_pipeline.py", workdir / "hybrid_pipeline.py")
    npm_bootstrap(workdir)
    env = rust_tool_env()

    run(["node", "deck.js"], workdir, "template deck.js")
    run(
        [sys.executable, "render_slides.py", "deck.pptx", "--output_dir", "rendered"],
        workdir,
        "render_slides",
        env=env,
    )
    run([sys.executable, "slides_test.py", "deck.pptx"], workdir, "slides_test", env=env)
    run(
        [sys.executable, "detect_font.py", "deck.pptx", "--include-missing", "--include-substituted"],
        workdir,
        "detect_font",
        env=env,
    )
    hybrid = run(
        [sys.executable, "hybrid_pipeline.py", "qa", "deck.pptx", "--rendered-dir", "rendered", "--json"],
        workdir,
        "hybrid qa",
        env=env,
    )
    hybrid_payload = json.loads(hybrid.stdout)

    result = {
        "name": "template_flow",
        "workdir": str(workdir),
        "deck_exists": (workdir / "deck.pptx").exists(),
        "rendered_pngs": count_pngs(workdir / "rendered"),
        "hybrid_render_pngs": hybrid_payload["render"]["png_count"],
    }
    doctor = officecli_doctor(workdir)
    if doctor:
        result["officecli_issue_count"] = doctor["issues"]["count"]
        result["officecli_validation_ok"] = doctor["validation"]["ok"]
    return result


def scenario_sample_deck(root: Path) -> dict:
    workdir = root / "sample_deck"
    workdir.mkdir(parents=True, exist_ok=True)

    shutil.copy2(ROOT / "deck.js", workdir / "deck.js")
    shutil.copytree(ASSETS / "pptxgenjs_helpers", workdir / "pptxgenjs_helpers")
    copy_common_python_tools(workdir)
    shutil.copy2(SCRIPTS / "officecli_bridge.py", workdir / "officecli_bridge.py")
    shutil.copy2(SCRIPTS / "hybrid_pipeline.py", workdir / "hybrid_pipeline.py")
    npm_bootstrap(workdir)
    env = rust_tool_env()

    run(["node", "deck.js"], workdir, "sample deck.js")
    run(
        [sys.executable, "render_slides.py", "deck.pptx", "--output_dir", "rendered"],
        workdir,
        "render_slides",
        env=env,
    )
    hybrid = run(
        [sys.executable, "hybrid_pipeline.py", "qa", "deck.pptx", "--rendered-dir", "rendered", "--json"],
        workdir,
        "hybrid qa",
        env=env,
    )
    hybrid_payload = json.loads(hybrid.stdout)

    result = {
        "name": "sample_deck_flow",
        "workdir": str(workdir),
        "deck_exists": (workdir / "deck.pptx").exists(),
        "rendered_pngs": count_pngs(workdir / "rendered"),
        "hybrid_render_pngs": hybrid_payload["render"]["png_count"],
    }
    doctor = officecli_doctor(workdir)
    if doctor:
        result["officecli_issue_count"] = doctor["issues"]["count"]
        result["officecli_validation_ok"] = doctor["validation"]["ok"]
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description="Run end-to-end smoke tests for skills/ppt-pptx.")
    parser.add_argument("--keep-workdir", action="store_true", help="Keep the temporary workspace.")
    parser.add_argument("--json", action="store_true", help="Emit machine-readable JSON summary.")
    parser.add_argument(
        "--strict-officecli",
        action="store_true",
        help="Fail if optional OfficeCLI doctor finds issues or validation failures.",
    )
    args = parser.parse_args()

    temp_dir_obj = tempfile.TemporaryDirectory(prefix="pptx-smoke-")
    temp_root = Path(temp_dir_obj.name)

    try:
        run(
            ["cargo", "build", "--manifest-path", str(RUST_TOOL_MANIFEST)],
            ROOT.parents[1],
            "cargo build pptx_tool_rs",
        )
        results = [
            scenario_outline(temp_root),
            scenario_template(temp_root),
            scenario_sample_deck(temp_root),
        ]
        payload = {
            "status": "pass",
            "root": str(temp_root),
            "officecli_available": officecli_available(),
            "results": results,
        }
        if args.strict_officecli and any(
            item.get("officecli_issue_count", 0) > 0 or item.get("officecli_validation_ok") is False
            for item in results
        ):
            raise RuntimeError("OfficeCLI strict audit found deck issues or validation failures")
        if args.json:
            print(json.dumps(payload, ensure_ascii=False, indent=2))
        else:
            print(f"PASS: ppt-pptx smoke test ({temp_root})")
            for item in results:
                extras = ", ".join(f"{k}={v}" for k, v in item.items() if k not in {"name", "workdir"})
                print(f"- {item['name']}: {extras}")

        if args.keep_workdir:
            print(f"Kept workspace: {temp_root}")
        return 0
    except Exception as exc:
        if args.json:
            print(json.dumps({"status": "fail", "root": str(temp_root), "error": str(exc)}, ensure_ascii=False, indent=2))
        else:
            print(f"FAIL: {exc}", file=sys.stderr)
            print(f"Workspace: {temp_root}", file=sys.stderr)
        if args.keep_workdir:
            print(f"Kept workspace: {temp_root}", file=sys.stderr)
        return 1
    finally:
        if not args.keep_workdir:
            temp_dir_obj.cleanup()


if __name__ == "__main__":
    raise SystemExit(main())
