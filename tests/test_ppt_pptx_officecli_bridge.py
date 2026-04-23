from __future__ import annotations

import importlib.util
from pathlib import Path


MODULE_PATH = Path("/Users/joe/Documents/skill/skills/ppt-pptx/scripts/officecli_bridge.py")
SPEC = importlib.util.spec_from_file_location("officecli_bridge", MODULE_PATH)
assert SPEC and SPEC.loader
officecli_bridge = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(officecli_bridge)


def test_summarize_doctor_compacts_outline_issues_and_validation() -> None:
    summary = officecli_bridge.summarize_doctor(
        "deck.pptx",
        {
            "success": True,
            "data": {
                "totalSlides": 3,
                "slides": [{"index": 1}, {"index": 2}, {"index": 3}],
            },
        },
        {
            "success": True,
            "data": {
                "Count": 3,
                "Issues": [
                    {"Path": "/slide[1]", "Message": "Slide has no title"},
                    {
                        "Path": "/slide[2]/shape[@id=4]",
                        "Message": "text overflow: suggest.height=0.6cm",
                    },
                    {
                        "Path": "/slide[3]/shape[@id=7]",
                        "Message": "text overflow: suggest.height=0.9cm",
                    },
                ],
            },
        },
        {
            "success": True,
            "message": "Found 1 validation error(s): schema mismatch",
        },
        "1.0.53",
    )

    assert summary["officecli_version"] == "1.0.53"
    assert summary["outline"]["total_slides"] == 3
    assert summary["issues"]["count"] == 3
    assert summary["issues"]["overflow_count"] == 2
    assert summary["issues"]["title_count"] == 1
    assert summary["validation"]["ok"] is False


def test_summarize_doctor_accepts_clean_validation_message() -> None:
    summary = officecli_bridge.summarize_doctor(
        "deck.pptx",
        {"success": True, "data": {"totalSlides": 1, "slides": [{"index": 1}]}},
        {"success": True, "data": {"Count": 0, "Issues": []}},
        {"success": True, "message": "Found 0 validation error(s)."},
        "1.0.53",
    )

    assert summary["issues"]["count"] == 0
    assert summary["validation"]["ok"] is True


def test_build_parser_accepts_doctor_failure_flags() -> None:
    parser = officecli_bridge.build_parser()
    args = parser.parse_args(
        [
            "doctor",
            "deck.pptx",
            "--fail-on-issues",
            "--fail-on-validation",
            "--json",
        ]
    )

    assert args.command == "doctor"
    assert args.fail_on_issues is True
    assert args.fail_on_validation is True
    assert args.json is True
