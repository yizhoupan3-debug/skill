#!/usr/bin/env python3
"""
Render Graphviz DOT files to PNG, SVG, or PDF.

Usage:
    python3 render_dot.py input.dot                      # default: PNG 300dpi
    python3 render_dot.py input.dot -f svg               # SVG output
    python3 render_dot.py input.dot -f pdf               # PDF output
    python3 render_dot.py input.dot -e neato             # use neato engine
    python3 render_dot.py input.dot --dpi 600            # high DPI
    python3 render_dot.py input.dot -o output/diagram    # custom output path (no ext)
    python3 render_dot.py *.dot -f png --dpi 300         # batch render

Requires: graphviz (brew install graphviz / apt install graphviz)
"""

import argparse
import subprocess
import sys
from pathlib import Path


ENGINES = ["dot", "neato", "fdp", "sfdp", "circo", "twopi", "osage", "patchwork"]
FORMATS = ["png", "svg", "pdf", "eps"]


def check_graphviz():
    """Verify graphviz is installed."""
    try:
        subprocess.run(["dot", "-V"], capture_output=True, check=True)
    except FileNotFoundError:
        print("ERROR: graphviz not found. Install it first:")
        print("  macOS:  brew install graphviz")
        print("  Ubuntu: sudo apt-get install graphviz")
        print("  pip:    pip install graphviz  (Python bindings only)")
        sys.exit(1)


def render(input_path: Path, output_path: Path, engine: str, fmt: str, dpi: int):
    """Render a single DOT file."""
    cmd = [
        engine,
        f"-T{fmt}",
        f"-Gdpi={dpi}",
        str(input_path),
        "-o", str(output_path),
    ]
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, check=True)
        print(f"OK: {input_path} -> {output_path}")
        if result.stderr:
            print(f"  warnings: {result.stderr.strip()}")
    except subprocess.CalledProcessError as e:
        print(f"FAIL: {input_path}")
        print(f"  error: {e.stderr.strip()}")
        return False
    return True


def main():
    """Parse arguments and render DOT files."""
    parser = argparse.ArgumentParser(
        description="Render Graphviz DOT files to image formats."
    )
    parser.add_argument("inputs", nargs="+", help="Input .dot/.gv file(s)")
    parser.add_argument("-f", "--format", default="png", choices=FORMATS,
                        help="Output format (default: png)")
    parser.add_argument("-e", "--engine", default="dot", choices=ENGINES,
                        help="Layout engine (default: dot)")
    parser.add_argument("--dpi", type=int, default=300,
                        help="DPI for raster output (default: 300)")
    parser.add_argument("-o", "--output", default=None,
                        help="Output path (without extension). "
                             "Only valid for single input.")

    args = parser.parse_args()
    check_graphviz()

    if args.output and len(args.inputs) > 1:
        print("ERROR: --output can only be used with a single input file.")
        sys.exit(1)

    success_count = 0
    fail_count = 0

    for input_str in args.inputs:
        input_path = Path(input_str)
        if not input_path.exists():
            print(f"SKIP: {input_path} not found")
            fail_count += 1
            continue

        if args.output:
            output_path = Path(f"{args.output}.{args.format}")
        else:
            output_path = input_path.with_suffix(f".{args.format}")

        output_path.parent.mkdir(parents=True, exist_ok=True)

        if render(input_path, output_path, args.engine, args.format, args.dpi):
            success_count += 1
        else:
            fail_count += 1

    print(f"\nDone: {success_count} rendered, {fail_count} failed")
    sys.exit(1 if fail_count > 0 else 0)


if __name__ == "__main__":
    main()
