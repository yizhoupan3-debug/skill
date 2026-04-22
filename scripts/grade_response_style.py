#!/usr/bin/env python3
"""Grade whether a user-facing response stays plain, short, and friendly."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path


JARGON_PATTERNS: dict[str, str] = {
    "internal_jargon": r"\b(control plane|routing layer|route engine|compatibility projection|overlay skill|owner skill|session id|singleton)\b",
    "internal_jargon_cn": r"(控制面|路由层|兼容投影|所有者技能|叠加技能|会话 id|路由引擎)",
}

STATUS_REPORT_PATTERNS: dict[str, str] = {
    "status_report_opener": r"^(我(会|先|正在|准备|继续|已经)|接下来|下面我来|先给你|我现在先)",
    "tool_talk": r"(我检查了|我看了|我刚跑了|我这边跑了|我已经修改了|我准备去改|我先去查|我先去检查)",
}

WEAK_STYLE_PATTERNS: dict[str, str] = {
    "hedging": r"(应该|可能|也许|大概|估计|看起来|有点像)",
    "mechanical_wrapup": r"(总之|综上|以上就是|下面是我做的|修改如下|我做了以下改动)",
}

MAX_DEFAULT_CHARS = 240
MAX_DEFAULT_PARAGRAPHS = 2
MAX_DEFAULT_LINES = 6


def _count_paragraphs(text: str) -> int:
    blocks = [block.strip() for block in re.split(r"\n\s*\n", text.strip()) if block.strip()]
    return len(blocks)


def _count_nonempty_lines(text: str) -> int:
    return sum(1 for line in text.splitlines() if line.strip())


def audit_response_style(
    text: str,
    *,
    max_chars: int = MAX_DEFAULT_CHARS,
    max_paragraphs: int = MAX_DEFAULT_PARAGRAPHS,
    max_lines: int = MAX_DEFAULT_LINES,
) -> tuple[int, list[str]]:
    """Return a style score and findings. Lower is better."""

    score = 0
    findings: list[str] = []
    stripped = text.strip()

    for category, pattern in JARGON_PATTERNS.items():
        matches = re.findall(pattern, stripped, flags=re.IGNORECASE)
        if matches:
            score += len(matches) * 2
            findings.append(f"[{category}] found internal jargon: {matches[0]}")

    opener_window = stripped[:80]
    for category, pattern in STATUS_REPORT_PATTERNS.items():
        match = re.search(pattern, opener_window, flags=re.IGNORECASE)
        if match:
            score += 2
            findings.append(f"[{category}] opener sounds like a progress report: {match.group(0)}")

    for category, pattern in WEAK_STYLE_PATTERNS.items():
        matches = re.findall(pattern, stripped, flags=re.IGNORECASE)
        if matches:
            score += len(matches)
            findings.append(f"[{category}] weak style marker: {matches[0]}")

    length = len(stripped)
    if length > max_chars:
        score += 2
        findings.append(f"[too_long] {length} chars exceeds {max_chars}")

    paragraph_count = _count_paragraphs(stripped)
    if paragraph_count > max_paragraphs:
        score += paragraph_count - max_paragraphs
        findings.append(f"[too_many_paragraphs] {paragraph_count} paragraphs exceeds {max_paragraphs}")

    line_count = _count_nonempty_lines(stripped)
    if line_count > max_lines:
        score += 1
        findings.append(f"[too_many_lines] {line_count} non-empty lines exceeds {max_lines}")

    bullet_count = len(re.findall(r"(?m)^\s*[-*]\s+", stripped))
    if bullet_count >= 4:
        score += 1
        findings.append(f"[bullet_heavy] {bullet_count} bullet lines makes the reply feel list-heavy")

    return score, findings


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Grade whether a response stays plain, short, and user-friendly."
    )
    parser.add_argument("file", nargs="?", help="Text file to grade. Reads stdin when omitted.")
    parser.add_argument(
        "--batch-jsonl",
        help="Grade a JSONL file where each line is an object with `id` and `text`.",
    )
    parser.add_argument("--max-chars", type=int, default=MAX_DEFAULT_CHARS)
    parser.add_argument("--max-paragraphs", type=int, default=MAX_DEFAULT_PARAGRAPHS)
    parser.add_argument("--max-lines", type=int, default=MAX_DEFAULT_LINES)
    parser.add_argument("--json", action="store_true", help="Emit JSON instead of plain text.")
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.batch_jsonl:
        rows: list[dict[str, object]] = []
        failed = 0
        for raw_line in Path(args.batch_jsonl).read_text(encoding="utf-8").splitlines():
            line = raw_line.strip()
            if not line:
                continue
            payload = json.loads(line)
            score, findings = audit_response_style(
                str(payload.get("text", "")),
                max_chars=args.max_chars,
                max_paragraphs=args.max_paragraphs,
                max_lines=args.max_lines,
            )
            passed = score == 0
            if not passed:
                failed += 1
            rows.append(
                {
                    "id": str(payload.get("id", f"row-{len(rows) + 1}")),
                    "score": score,
                    "passed": passed,
                    "findings": findings,
                }
            )

        report = {
            "total": len(rows),
            "passed": len(rows) - failed,
            "failed": failed,
            "results": rows,
        }
        if args.json:
            print(json.dumps(report, ensure_ascii=False, indent=2))
        else:
            print(f"Batch total: {report['total']}")
            print(f"Passed: {report['passed']}")
            print(f"Failed: {report['failed']}")
            for row in rows:
                if row["passed"]:
                    continue
                print(f"- {row['id']}: {'; '.join(row['findings'])}")
        return 0 if failed == 0 else 1

    if args.file:
        content = Path(args.file).read_text(encoding="utf-8")
    else:
        content = sys.stdin.read()

    score, findings = audit_response_style(
        content,
        max_chars=args.max_chars,
        max_paragraphs=args.max_paragraphs,
        max_lines=args.max_lines,
    )
    passed = score == 0

    if args.json:
        print(
            json.dumps(
                {
                    "score": score,
                    "passed": passed,
                    "findings": findings,
                },
                ensure_ascii=False,
                indent=2,
            )
        )
    else:
        print("Style grade:", score)
        if findings:
            print("Findings:")
            for finding in findings:
                print(f"- {finding}")
        else:
            print("Clean: response stayed plain and compact.")

    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
