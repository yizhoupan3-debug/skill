from __future__ import annotations

import importlib.util
from pathlib import Path


MODULE_PATH = Path("/Users/joe/Documents/skill/skills/ppt-pptx/scripts/hybrid_pipeline.py")
SPEC = importlib.util.spec_from_file_location("hybrid_pipeline", MODULE_PATH)
assert SPEC and SPEC.loader
hybrid_pipeline = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(hybrid_pipeline)


def test_build_parser_accepts_build_qa_flags() -> None:
    parser = hybrid_pipeline.build_parser()
    args = parser.parse_args(
        [
            "build-qa",
            "--workdir",
            "tmp/project",
            "--entry",
            "deck.js",
            "--deck",
            "deck.pptx",
            "--rendered-dir",
            "rendered",
            "--json",
        ]
    )

    assert args.command == "build-qa"
    assert args.workdir == "tmp/project"
    assert args.entry == "deck.js"
    assert args.deck == "deck.pptx"
    assert args.rendered_dir == "rendered"
    assert args.json is True


def test_build_parser_accepts_intake_command() -> None:
    parser = hybrid_pipeline.build_parser()
    args = parser.parse_args(["intake", "deck.pptx", "--json"])

    assert args.command == "intake"
    assert args.deck == "deck.pptx"
    assert args.json is True
