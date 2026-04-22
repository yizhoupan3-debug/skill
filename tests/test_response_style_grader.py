from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = PROJECT_ROOT / "scripts" / "grade_response_style.py"
FIXTURE_PATH = PROJECT_ROOT / "tests" / "response_style_grader_cases.json"
FIXTURES = json.loads(FIXTURE_PATH.read_text(encoding="utf-8"))


if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts.grade_response_style import audit_response_style


def test_response_style_fixture_schema_is_stable() -> None:
    assert FIXTURES["schema_version"] == "response-style-grader-cases-v1"
    assert FIXTURES["cases"]


def test_response_style_grader_matches_cases() -> None:
    for case in FIXTURES["cases"]:
        score, findings = audit_response_style(str(case["text"]))
        if case["should_pass"]:
            assert score == 0, f"{case['id']} should pass but got {findings}"
        else:
            assert score > 0, f"{case['id']} should fail"
            findings_text = "\n".join(findings)
            for marker in case["expected_findings"]:
                assert marker in findings_text, f"{case['id']} missing finding {marker!r}"


def test_response_style_grader_cli_json_output(tmp_path: Path) -> None:
    sample = tmp_path / "reply.txt"
    sample.write_text("现在能跑了，还差最后一轮验证。", encoding="utf-8")

    completed = subprocess.run(
        [sys.executable, str(SCRIPT_PATH), str(sample), "--json"],
        cwd=PROJECT_ROOT,
        text=True,
        capture_output=True,
        check=True,
    )
    payload = json.loads(completed.stdout)
    assert payload["passed"] is True
    assert payload["score"] == 0
    assert payload["findings"] == []


def test_response_style_grader_batch_jsonl_output(tmp_path: Path) -> None:
    batch = tmp_path / "replies.jsonl"
    batch.write_text(
        "\n".join(
            [
                json.dumps(
                    {"id": "good", "text": "现在能跑了，还差最后一轮验证。"},
                    ensure_ascii=False,
                ),
                json.dumps(
                    {
                        "id": "bad",
                        "text": "我已经检查了当前 routing layer 和 control plane，所以 route engine 现在稳定。",
                    },
                    ensure_ascii=False,
                ),
            ]
        )
        + "\n",
        encoding="utf-8",
    )

    completed = subprocess.run(
        [sys.executable, str(SCRIPT_PATH), "--batch-jsonl", str(batch), "--json"],
        cwd=PROJECT_ROOT,
        text=True,
        capture_output=True,
    )
    assert completed.returncode == 1
    payload = json.loads(completed.stdout)
    assert payload["total"] == 2
    assert payload["passed"] == 1
    assert payload["failed"] == 1
    assert payload["results"][0]["id"] == "good"
    assert payload["results"][0]["passed"] is True
    assert payload["results"][1]["id"] == "bad"
    assert payload["results"][1]["passed"] is False
