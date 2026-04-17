#!/usr/bin/env python3
"""
Initialize an autoresearch workspace with standard structure and templates.

Usage:
    python3 init_research.py --project <name> --question "<research question>" [--dir <path>]
"""

import argparse
import os
from datetime import datetime
from pathlib import Path


RESEARCH_STATE_TEMPLATE = """project: {project}
question: "{question}"
status: bootstrap
hypotheses: []
current_direction: null
novelty_check: null
created: {date}
updated: {date}
"""

RESEARCH_LOG_TEMPLATE = """# Research Log — {project}

## {date} — Project initialized

- **Research question**: {question}
- **Status**: Bootstrap phase
- **Next step**: Literature scan → form initial hypotheses → novelty gate
"""

FINDINGS_TEMPLATE = """# Findings — {project}

## Research Question

{question}

## Initial Assumptions

_Fill in your starting assumptions here._

## Current Understanding

_Updated after each outer loop reflection._

## Positioning Strategy

_Updated after novelty gate check._
"""

PROTOCOL_TEMPLATE = """# Experiment Protocol: [hypothesis-slug]

## Hypothesis

_State the hypothesis clearly._

## What change

_What is being tested?_

## Prediction

_What do you expect to happen?_

## Why

_What is the rationale?_

## Method

_How will this be tested?_

## Success criteria

_What metric/threshold defines success?_

## Label

- [ ] CONFIRMATORY
- [ ] EXPLORATORY
"""


def init_workspace(project: str, question: str, base_dir: str = "."):
    """
    Create the autoresearch workspace structure.

    Args:
        project: Project name (used for directory and filenames)
        question: One-sentence research question
        base_dir: Parent directory for the project
    """
    root = Path(base_dir) / project
    date = datetime.now().strftime("%Y-%m-%d")

    # Create directory structure
    dirs = [
        root,
        root / "literature",
        root / "src",
        root / "data",
        root / "experiments",
        root / "to_human",
        root / "paper",
    ]
    for d in dirs:
        d.mkdir(parents=True, exist_ok=True)

    # Write research-state.yaml
    (root / "research-state.yaml").write_text(
        RESEARCH_STATE_TEMPLATE.format(
            project=project, question=question, date=date
        )
    )

    # Write research-log.md
    (root / "research-log.md").write_text(
        RESEARCH_LOG_TEMPLATE.format(
            project=project, question=question, date=date
        )
    )

    # Write findings.md
    (root / "findings.md").write_text(
        FINDINGS_TEMPLATE.format(project=project, question=question)
    )

    # Write protocol template
    (root / "experiments" / "PROTOCOL_TEMPLATE.md").write_text(
        PROTOCOL_TEMPLATE
    )

    print(f"✅ Research workspace initialized at: {root.resolve()}")
    print(f"   Project: {project}")
    print(f"   Question: {question}")
    print(f"   Next: run literature scan → form hypotheses → novelty gate")


def main():
    parser = argparse.ArgumentParser(
        description="Initialize an autoresearch workspace"
    )
    parser.add_argument(
        "--project", required=True, help="Project name"
    )
    parser.add_argument(
        "--question", required=True, help="One-sentence research question"
    )
    parser.add_argument(
        "--dir", default=".", help="Parent directory (default: current)"
    )
    args = parser.parse_args()

    init_workspace(args.project, args.question, args.dir)


if __name__ == "__main__":
    main()
