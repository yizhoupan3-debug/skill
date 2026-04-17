#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

LATEX_CITE_PATTERN = re.compile(
    r"\\cite[a-zA-Z*]*\s*(?:\[[^\]]*\]\s*){0,2}\{([^}]*)\}", re.MULTILINE
)
NUMERIC_CLUSTER_PATTERN = re.compile(r"\[(\d+(?:\s*[-,]\s*\d+)*)\]")
AUTHOR_YEAR_CLUSTER_PATTERN = re.compile(r"\(([^()]*(?:19|20)\d{2}[a-z]?[^()]*)\)")


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def normalize_whitespace(text: str) -> str:
    return re.sub(r"\s+", " ", text).strip()


def sentence_split(text: str) -> list[str]:
    parts = re.split(r"(?<=[。！？!?\.])\s+|\n{2,}", text)
    return [normalize_whitespace(part) for part in parts if normalize_whitespace(part)]


def count_latex_keys(payload: str) -> int:
    return len([item for item in payload.split(",") if item.strip()])


def count_numeric_items(payload: str) -> int:
    return len([item for item in re.split(r"\s*,\s*", payload) if item.strip()])


def count_author_year_items(payload: str) -> int:
    if ";" not in payload:
        return 0
    return len([item for item in payload.split(";") if re.search(r"(?:19|20)\d{2}[a-z]?", item)])


def flag_sentence(sentence: str, threshold: int) -> dict | None:
    reasons: list[str] = []
    citation_count = 0
    ending_cluster = False

    for match in LATEX_CITE_PATTERN.finditer(sentence):
        count = count_latex_keys(match.group(1))
        citation_count += count
        if match.end() >= len(sentence.rstrip(" .。!！?？")):
            ending_cluster = True
    for match in NUMERIC_CLUSTER_PATTERN.finditer(sentence):
        count = count_numeric_items(match.group(1))
        citation_count += count
        if match.end() >= len(sentence.rstrip(" .。!！?？")):
            ending_cluster = True
    for match in AUTHOR_YEAR_CLUSTER_PATTERN.finditer(sentence):
        count = count_author_year_items(match.group(1))
        citation_count += count
        if count and match.end() >= len(sentence.rstrip(" .。!！?？")):
            ending_cluster = True

    if citation_count >= threshold:
        reasons.append(f"dense citation cluster detected ({citation_count} citations)")
    if ending_cluster and citation_count >= 2:
        reasons.append("sentence ends with a stacked citation cluster; consider claim-level placement")

    if not reasons:
        return None

    return {
        "sentence": sentence,
        "citation_count": citation_count,
        "reasons": reasons,
    }


def lint(text: str, threshold: int) -> list[dict]:
    findings = []
    for sentence in sentence_split(text):
        finding = flag_sentence(sentence, threshold)
        if finding:
            findings.append(finding)
    return findings


def render_markdown(findings: list[dict]) -> str:
    lines = ["# Claim-to-Citation Lint", ""]
    if not findings:
        lines.append("- No dense or sentence-ending citation clusters were flagged.")
        return "\n".join(lines) + "\n"

    for idx, finding in enumerate(findings, start=1):
        lines.append(f"## Finding {idx}")
        lines.append(f"- **citation_count**: {finding['citation_count']}")
        for reason in finding["reasons"]:
            lines.append(f"- **reason**: {reason}")
        lines.append(f"- **sentence**: {finding['sentence']}")
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(description="Flag dense or sentence-ending citation clusters in a manuscript.")
    parser.add_argument("--manuscript", required=True, help="Path to manuscript text (.tex/.md/.txt)")
    parser.add_argument("--threshold", type=int, default=3, help="Flag sentences with at least this many citations")
    parser.add_argument("--format", choices=["markdown", "json"], default="markdown")
    args = parser.parse_args()

    findings = lint(read_text(Path(args.manuscript)), args.threshold)
    if args.format == "json":
        print(json.dumps(findings, ensure_ascii=False, indent=2))
    else:
        print(render_markdown(findings), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
