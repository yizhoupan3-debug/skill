#!/usr/bin/env python3
"""
Preview all available matplotlib styles from SciencePlots and built-in styles.

Usage:
    python3 preview_styles.py                    # preview all styles
    python3 preview_styles.py --filter science   # filter by name
    python3 preview_styles.py --output output/   # save to directory
    python3 preview_styles.py --dpi 150          # lower DPI for faster preview

Requires: matplotlib, SciencePlots (optional but recommended)
"""

import argparse
import sys
from pathlib import Path

try:
    import matplotlib.pyplot as plt
    import matplotlib
    import numpy as np
except ImportError:
    print("ERROR: matplotlib and numpy required.")
    print("  pip install matplotlib numpy")
    sys.exit(1)

# Try loading SciencePlots
try:
    import scienceplots  # noqa: F401
    HAS_SCIENCEPLOTS = True
except ImportError:
    HAS_SCIENCEPLOTS = False
    print("NOTE: SciencePlots not installed. Only built-in styles will be shown.")
    print("  pip install SciencePlots")


# Sample data for preview
def generate_sample_data():
    """Generate sample data for style preview plots."""
    np.random.seed(42)
    x = np.linspace(0, 2 * np.pi, 100)
    return {
        "x": x,
        "y1": np.sin(x),
        "y2": np.cos(x),
        "y3": np.sin(x) * np.exp(-x / 5),
        "scatter_x": np.random.randn(50),
        "scatter_y": np.random.randn(50),
    }


def create_preview(style_name, data, output_dir=None, dpi=150):
    """Create a preview figure for a given style."""
    try:
        with plt.style.context(style_name):
            fig, axes = plt.subplots(1, 2, figsize=(7, 3))

            # Line plot
            ax1 = axes[0]
            ax1.plot(data["x"], data["y1"], label="sin(x)")
            ax1.plot(data["x"], data["y2"], label="cos(x)")
            ax1.plot(data["x"], data["y3"], label="damped sin")
            ax1.set_xlabel("x")
            ax1.set_ylabel("y")
            ax1.set_title("Line Plot")
            ax1.legend(fontsize=7)

            # Scatter plot
            ax2 = axes[1]
            ax2.scatter(data["scatter_x"], data["scatter_y"], alpha=0.7, s=20)
            ax2.set_xlabel("x")
            ax2.set_ylabel("y")
            ax2.set_title("Scatter Plot")

            style_label = style_name if isinstance(style_name, str) else " + ".join(style_name)
            fig.suptitle(f"Style: {style_label}", fontsize=11, fontweight="bold")
            fig.tight_layout()

            if output_dir:
                safe_name = style_label.replace("/", "_").replace(" ", "_")
                out_path = Path(output_dir) / f"style_{safe_name}.png"
                fig.savefig(out_path, dpi=dpi, bbox_inches="tight")
                plt.close(fig)
                return str(out_path)
            else:
                plt.close(fig)
                return None

    except Exception as e:
        print(f"  SKIP: {style_name} — {e}")
        return None


def get_available_styles(filter_str=None):
    """Get list of available styles, optionally filtered."""
    styles = sorted(plt.style.available)

    # Add SciencePlots combos if available
    science_combos = []
    if HAS_SCIENCEPLOTS:
        science_combos = [
            ["science"],
            ["science", "ieee"],
            ["science", "nature"],
            ["science", "cell"],
            ["science", "lancet"],
            ["science", "bmj"],
            ["science", "high-vis"],
            ["science", "high-contrast"],
            ["science", "scatter"],
            ["science", "notebook"],
            ["science", "grid"],
            ["science", "retro"],
            ["science", "muted"],
        ]

    all_styles = [(s,) if isinstance(s, str) else tuple(s)
                  for s in styles] + [tuple(s) for s in science_combos]

    if filter_str:
        all_styles = [s for s in all_styles
                      if filter_str.lower() in " ".join(s).lower()]

    return all_styles


def main():
    """Parse arguments and generate style previews."""
    parser = argparse.ArgumentParser(
        description="Preview matplotlib styles including SciencePlots."
    )
    parser.add_argument("--filter", default=None,
                        help="Filter styles by name substring")
    parser.add_argument("--output", default="tmp/style_previews",
                        help="Output directory (default: tmp/style_previews)")
    parser.add_argument("--dpi", type=int, default=150,
                        help="DPI for preview images (default: 150)")
    parser.add_argument("--list", action="store_true",
                        help="List available styles without rendering")

    args = parser.parse_args()
    styles = get_available_styles(args.filter)

    if args.list:
        print(f"Available styles ({len(styles)}):")
        for s in styles:
            print(f"  {' + '.join(s)}")
        return

    output_dir = Path(args.output)
    output_dir.mkdir(parents=True, exist_ok=True)
    data = generate_sample_data()

    success = 0
    total = len(styles)
    print(f"Rendering {total} style previews to {output_dir}/...")

    for style in styles:
        style_arg = list(style) if len(style) > 1 else style[0]
        result = create_preview(style_arg, data, output_dir, args.dpi)
        if result:
            print(f"  OK: {result}")
            success += 1

    print(f"\nDone: {success}/{total} styles rendered to {output_dir}/")


if __name__ == "__main__":
    main()
