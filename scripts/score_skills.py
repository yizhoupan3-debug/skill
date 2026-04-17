#!/usr/bin/env python3
"""Score skill quality across 7 automated dimensions.

Dimensions:
  1. Structure (10%) — frontmatter + required headings
  2. Boundaries (10%) — When to use / Do not use sections
  3. Description (10%) — length + trigger phrasing
  4. Size (10%) — token count in reasonable range
  5. Routing metadata (10%) — routing_layer/owner/gate/session_start
  6. Progressive loading (10%) — large skills have references/
  7. Content depth (20%) — workflow steps, code blocks, cross-references
  8. Rigor (20%) — verification evidence + context7 + PUA adherence
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

# Import shared utilities from check_skills
sys.path.insert(0, str(Path(__file__).resolve().parent))
from check_skills import (  # noqa: E402
    estimate_tokens,
    iter_skill_dirs,
    parse_frontmatter,
    TOKEN_WARN_THRESHOLD,
)

DIMENSION_WEIGHTS = {
    "structure": 0.10,
    "boundaries": 0.10,
    "description": 0.10,
    "size": 0.10,
    "routing_metadata": 0.10,
    "progressive_loading": 0.10,
    "content_depth": 0.20,
    "rigor": 0.20,
}

TRIGGER_PHRASES = [
    "Use when", "Best for", "适用于", "Use proactively",
    "When the user", "Use this skill when",
]

REQUIRED_HEADINGS = ["## When to use", "## Do not use"]

ROUTING_FIELDS = ["routing_layer", "routing_owner", "routing_gate", "session_start", "routing_priority"]


def score_structure(metadata: dict, body: str) -> int:
    """Score structural completeness (0-100).

    Args:
        metadata: Parsed frontmatter dict.
        body: SKILL.md body text.

    Returns:
        Score 0-100.
    """
    score = 0
    # Has name (25 pts)
    if metadata.get("name", "").strip():
        score += 25
    # Has description (25 pts)
    if metadata.get("description", "").strip():
        score += 25
    # Has section headings (25 pts)
    if "## " in body:
        score += 25
    # Has overview or core content (25 pts)
    heading_count = body.count("## ")
    if heading_count >= 3:
        score += 25
    elif heading_count >= 1:
        score += 15
    return score


def score_boundaries(body: str) -> int:
    """Score boundary clarity (0-100).

    Args:
        body: SKILL.md body text.

    Returns:
        Score 0-100.
    """
    score = 0
    body_lower = body.lower()
    # Has "When to use" (40 pts)
    if "## when to use" in body_lower:
        score += 40
    elif "when to use" in body_lower:
        score += 20
    # Has "Do not use" (40 pts)
    if "## do not use" in body_lower:
        score += 40
    elif "do not use" in body_lower or "不要" in body or "不适用" in body:
        score += 20
    # Boundary items have substantive content (20 pts)
    # Check that each section has at least 2 bullet points
    when_idx = body_lower.find("## when to use")
    donot_idx = body_lower.find("## do not use")
    if when_idx >= 0 and donot_idx >= 0:
        when_section = body[when_idx:donot_idx]
        donot_end = body.find("## ", donot_idx + 14)
        donot_section = body[donot_idx:donot_end] if donot_end > 0 else body[donot_idx:]
        when_bullets = when_section.count("\n- ")
        donot_bullets = donot_section.count("\n- ")
        if when_bullets >= 3 and donot_bullets >= 3:
            score += 20
        elif when_bullets >= 2 and donot_bullets >= 2:
            score += 10
    return score


def score_description(metadata: dict) -> int:
    """Score description quality (0-100).

    Args:
        metadata: Parsed frontmatter dict.

    Returns:
        Score 0-100.
    """
    desc = metadata.get("description", "").strip()
    if not desc:
        return 0

    score = 0
    # Length/efficiency (30 pts) — dense enough to trigger, short enough to stay cheap
    desc_len = len(desc)
    if 120 <= desc_len <= 450:
        score += 30
    elif 80 <= desc_len < 120 or 451 <= desc_len <= 600:
        score += 24
    elif 50 <= desc_len < 80 or 601 <= desc_len <= 750:
        score += 16
    elif 751 <= desc_len <= 900:
        score += 8
    elif desc_len >= 30:
        score += 10

    # Trigger phrasing (30 pts)
    has_trigger = any(phrase.lower() in desc.lower() for phrase in TRIGGER_PHRASES)
    if has_trigger:
        score += 30
    elif "when" in desc.lower() or "适用" in desc or "触发" in desc:
        score += 15

    # First-line brief <= 120 chars per AGENTS.md convention (20 pts)
    first_line = desc.split("\n")[0].strip()
    if len(first_line) <= 120:
        score += 20
    elif len(first_line) <= 150:
        score += 10

    # Contains domain-specific nouns or tool names (20 pts)
    import re
    tool_patterns = re.findall(r"[A-Z][a-z]+(?:[A-Z][a-z]+)*|\.[a-z]{2,4}\b|`[^`]+`", desc)
    if len(tool_patterns) >= 3:
        score += 20
    elif len(tool_patterns) >= 1:
        score += 10

    return min(score, 100)


def score_size(body: str, has_references: bool) -> int:
    """Score body size reasonableness (0-100).

    Args:
        body: SKILL.md body text.
        has_references: Whether the skill has a references/ directory.

    Returns:
        Score 0-100.
    """
    tokens = estimate_tokens(body)
    if tokens == 0:
        return 0

    # Ideal range: 300-3000 tokens
    if 300 <= tokens <= 3000:
        return 100
    elif 100 <= tokens < 300:
        return 70  # a bit thin
    elif 3000 < tokens <= TOKEN_WARN_THRESHOLD:
        return 80  # slightly large but OK
    elif tokens > TOKEN_WARN_THRESHOLD:
        return 90 if has_references else 40  # large but has refs = OK
    else:
        return 30  # very thin


def score_routing_metadata(metadata: dict) -> int:
    """Score routing metadata completeness (0-100).

    Args:
        metadata: Parsed frontmatter dict.

    Returns:
        Score 0-100.
    """
    score = 0
    per_field = 100 // len(ROUTING_FIELDS)
    for field in ROUTING_FIELDS:
        if metadata.get(field, "").strip():
            score += per_field
    return min(score, 100)


def score_progressive_loading(body: str, has_references: bool) -> int:
    """Score progressive loading practice (0-100).

    Args:
        body: SKILL.md body text.
        has_references: Whether the skill has a references/ directory.

    Returns:
        Score 0-100.
    """
    tokens = estimate_tokens(body)
    # Small skills don't need references
    if tokens <= 2000:
        return 100
    # Medium skills get partial credit
    if tokens <= TOKEN_WARN_THRESHOLD:
        return 100 if has_references else 70
    # Large skills really should have references
    return 100 if has_references else 30


def score_content_depth(body: str) -> int:
    """Score content actionability and depth (0-100).

    Checks for workflow steps, code blocks, and cross-references
    to other skills — things that distinguish a real skill from a
    placeholder.

    Args:
        body: SKILL.md body text.

    Returns:
        Score 0-100.
    """
    import re

    score = 0

    # Workflow steps: numbered or bulleted actionable items (35 pts)
    step_patterns = re.findall(r"^\s*(?:\d+\.|[-*])\s+\S", body, re.MULTILINE)
    step_count = len(step_patterns)
    if step_count >= 15:
        score += 35
    elif step_count >= 10:
        score += 28
    elif step_count >= 5:
        score += 20
    elif step_count >= 2:
        score += 10

    # Code blocks or structured examples (30 pts)
    code_blocks = body.count("```")
    if code_blocks >= 6:
        score += 30
    elif code_blocks >= 4:
        score += 22
    elif code_blocks >= 2:
        score += 15
    elif code_blocks >= 1:
        score += 5

    # Cross-references to other skills via $skill-name or backtick refs (20 pts)
    xrefs = len(re.findall(r"\$[a-z][-a-z0-9]+", body))
    if xrefs >= 8:
        score += 20
    elif xrefs >= 5:
        score += 15
    elif xrefs >= 2:
        score += 10
    elif xrefs >= 1:
        score += 5

    # Section heading depth (15 pts)
    h2_count = body.count("\n## ")
    h3_count = body.count("\n### ")
    if h2_count >= 4 and h3_count >= 2:
        score += 15
    elif h2_count >= 3:
        score += 10
    elif h2_count >= 2:
        score += 5

    return min(score, 100)


def score_rigor(metadata: dict, body: str) -> int:
    """Score mental rigor and anti-laziness enforcement (0-100).

    Args:
        metadata: Parsed frontmatter dict.
        body: SKILL.md body text.

    Returns:
        Score 0-100.
    """
    score = 0
    body_lower = body.lower()

    # 1. Verification Evidence Requirement (40 pts)
    # Checks for [Verification Evidence] block mentions or templates
    if "[verification evidence]" in body_lower or "验证证据" in body:
        score += 40
    elif "verify" in body_lower or "验证" in body:
        score += 20

    # 2. Context7 Integration (30 pts)
    # Checks for mandatory documentation lookup via context7
    if "context7" in body_lower or "query-docs" in body_lower:
        score += 30
    elif "official docs" in body_lower or "官方文档" in body:
        score += 15

    # 3. PUA / Anti-Laziness Tags & References (30 pts)
    tags = metadata.get("tags", [])
    if any(t in ["pua", "anti-laziness", "rigor"] for t in tags):
        score += 20
    if "pua_core" in body_lower or "anti-laziness methodology" in body_lower:
        score += 10

    return min(score, 100)


def score_skill(skill_dir: Path) -> dict:
    """Score a single skill across all dimensions.

    Args:
        skill_dir: Path to the skill directory.

    Returns:
        Dict with per-dimension scores, weighted total, and metadata.
    """
    skill_md = skill_dir / "SKILL.md"
    slug = skill_dir.name
    has_references = (skill_dir / "references").is_dir()

    if not skill_md.is_file():
        return {
            "name": slug,
            "total": 0,
            "dimensions": {},
            "error": "missing SKILL.md",
        }

    text = skill_md.read_text(encoding="utf-8")
    metadata, body, error = parse_frontmatter(text)
    if error:
        return {
            "name": slug,
            "total": 0,
            "dimensions": {},
            "error": error,
        }

    body_text = body.strip()

    dimensions = {
        "structure": score_structure(metadata, body_text),
        "boundaries": score_boundaries(body_text),
        "description": score_description(metadata),
        "size": score_size(body_text, has_references),
        "routing_metadata": score_routing_metadata(metadata),
        "progressive_loading": score_progressive_loading(body_text, has_references),
        "content_depth": score_content_depth(body_text),
        "rigor": score_rigor(metadata, body_text),
    }

    total = sum(
        dimensions[dim] * weight
        for dim, weight in DIMENSION_WEIGHTS.items()
    )

    return {
        "name": slug,
        "total": round(total, 1),
        "dimensions": dimensions,
        "tokens": estimate_tokens(body_text),
        "has_references": has_references,
    }


def rating(score: float) -> str:
    """Convert score to human-readable rating.

    Args:
        score: Numeric score 0-100.

    Returns:
        Rating string.
    """
    if score >= 90:
        return "★★★★★ Excellent"
    elif score >= 80:
        return "★★★★☆ Good"
    elif score >= 70:
        return "★★★☆☆ Acceptable"
    elif score >= 60:
        return "★★☆☆☆ Needs Work"
    else:
        return "★☆☆☆☆ Poor"


def main() -> int:
    parser = argparse.ArgumentParser(description="Score skill quality across 6 dimensions.")
    parser.add_argument(
        "skill_dirs",
        type=Path,
        nargs="*",
        help="Specific skill directories to score.",
    )
    parser.add_argument(
        "--all",
        action="store_true",
        help="Score all skills.",
    )
    parser.add_argument(
        "--skills-root",
        type=Path,
        default=Path(__file__).resolve().parents[1] / "skills",
        help="Path to the skills root directory.",
    )
    parser.add_argument(
        "--below",
        type=float,
        help="Only show skills scoring below this threshold.",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        dest="json_output",
        help="Output as JSON.",
    )
    parser.add_argument(
        "--report",
        type=Path,
        help="Write quality report to a markdown file.",
    )
    args = parser.parse_args()

    if args.all:
        skills_root = args.skills_root.resolve()
        dirs = [d for _, d in iter_skill_dirs(skills_root, include_system=True)]
    elif args.skill_dirs:
        dirs = [d.resolve() for d in args.skill_dirs]
    else:
        parser.error("provide skill directories or use --all")
        return 1

    results = [score_skill(d) for d in dirs]
    results.sort(key=lambda r: r["total"], reverse=True)

    if args.below is not None:
        results = [r for r in results if r["total"] < args.below]

    if args.json_output:
        summary = {
            "total_scored": len(results),
            "avg_score": round(sum(r["total"] for r in results) / max(len(results), 1), 1),
            "skills": results,
        }
        print(json.dumps(summary, indent=2, ensure_ascii=False))
        return 0

    # Text output
    if not results:
        print("No skills to score.")
        return 0

    avg = sum(r["total"] for r in results) / len(results)
    print(f"Skill Quality Scores ({len(results)} skills, avg: {avg:.1f})")
    print("=" * 70)
    for r in results:
        score = r["total"]
        name = r["name"]
        if r.get("error"):
            print(f"  {name:35s} ERROR: {r['error']}")
            continue
        dims = r["dimensions"]
        dim_str = " ".join(f"{k[:4]}:{v}" for k, v in dims.items())
        print(f"  {name:35s} {score:5.1f}  {rating(score):20s}  {dim_str}")

    # Generate report if requested
    if args.report:
        _write_report(args.report, results, avg)
        print(f"\nReport written to: {args.report}")

    return 0


def _write_report(path: Path, results: list[dict], avg: float) -> None:
    """Write a markdown quality report.

    Args:
        path: Output file path.
        results: List of skill score results.
        avg: Average score.
    """
    excellent = [r for r in results if r["total"] >= 90]
    good = [r for r in results if 80 <= r["total"] < 90]
    acceptable = [r for r in results if 70 <= r["total"] < 80]
    needs_work = [r for r in results if 60 <= r["total"] < 70]
    poor = [r for r in results if r["total"] < 60]

    lines = [
        "# Skill Quality Report",
        "",
        f"**Scored:** {len(results)} skills | **Average:** {avg:.1f}/100",
        "",
        f"| Rating | Count |",
        f"|---|---|",
        f"| ★★★★★ Excellent (90+) | {len(excellent)} |",
        f"| ★★★★☆ Good (80-89) | {len(good)} |",
        f"| ★★★☆☆ Acceptable (70-79) | {len(acceptable)} |",
        f"| ★★☆☆☆ Needs Work (60-69) | {len(needs_work)} |",
        f"| ★☆☆☆☆ Poor (<60) | {len(poor)} |",
        "",
        "## All Skills",
        "",
        "| Skill | Score | Struct | Bound | Desc | Size | Route | Prog | Depth | Rigor |",
        "|---|---|---|---|---|---|---|---|---|---|",
    ]
    for r in results:
        if r.get("error"):
            lines.append(f"| `{r['name']}` | ERROR | — | — | — | — | — | — |")
            continue
        d = r["dimensions"]
        lines.append(
            f"| `{r['name']}` | **{r['total']}** "
            f"| {d['structure']} | {d['boundaries']} | {d['description']} "
            f"| {d['size']} | {d['routing_metadata']} | {d['progressive_loading']} "
            f"| {d['content_depth']} | {d['rigor']} |"
        )

    if poor:
        lines.extend([
            "",
            "## ⚠ Skills Needing Attention",
            "",
        ])
        for r in poor:
            if r.get("error"):
                continue
            d = r["dimensions"]
            weak = [k for k, v in d.items() if v < 50]
            lines.append(
                f"- **`{r['name']}`** ({r['total']}): "
                f"weak in {', '.join(weak)}"
            )

    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


if __name__ == "__main__":
    raise SystemExit(main())
