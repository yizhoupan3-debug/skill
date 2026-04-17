#!/usr/bin/env python3
"""
Test CJK font rendering on macOS with matplotlib.

Generates test figures containing Chinese text to verify that no tofu (□)
boxes appear. Outputs PNGs to /tmp/cjk_font_test/.

Usage:
    python3 test_cjk_fonts.py
"""

import sys
from pathlib import Path

try:
    import matplotlib
    matplotlib.use("Agg")  # non-interactive backend
    import matplotlib.pyplot as plt
    import matplotlib.font_manager as fm
    import numpy as np
except ImportError:
    print("ERROR: matplotlib and numpy required.")
    print("  pip install matplotlib numpy")
    sys.exit(1)

# Add assets to path so we can import publication_rcparams
SKILL_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(SKILL_ROOT / "assets"))

from publication_rcparams import (
    _find_cjk_font,
    apply_publication_style,
    sanitize_cjk_text,
    patch_figure_cjk,
)


OUTPUT_DIR = Path("/tmp/cjk_font_test")


def test_font_detection():
    """Test that CJK font detection works on this system."""
    font = _find_cjk_font()
    if font:
        print(f"  CJK font detected: {font}")
    else:
        print("  FAIL: No CJK font detected! Chinese text will show as tofu.")
    return font


def test_sanitize_function():
    """Test sanitize_cjk_text() covers all known problematic chars."""
    cases = [
        ("x\u00b2 \u2212 y\u00b3", "x2 - y3"),
        ("\u2212\u2010\u2011", "---"),
        ("\u00b9\u2070\u2074\u2075", "1045"),
        ("\u2080\u2081\u2082\u2083", "0123"),
        ("normal text 正常", "normal text 正常"),
    ]
    all_pass = True
    for inp, expected in cases:
        result = sanitize_cjk_text(inp)
        status = "OK" if result == expected else f"FAIL: got '{result}'"
        if result != expected:
            all_pass = False
        print(f"  sanitize({inp!r}) = {result!r}  {status}")
    return all_pass


def test_chinese_line_plot():
    """Generate a line plot with Chinese labels."""
    apply_publication_style(locale="zh")

    fig, ax = plt.subplots(figsize=(5, 3.5))

    x = np.linspace(-2 * np.pi, 2 * np.pi, 200)
    ax.plot(x, np.sin(x), label="正弦函数 sin(x)")
    ax.plot(x, np.cos(x), label="余弦函数 cos(x)")
    ax.plot(x, np.sin(x) * np.exp(-x / 5), label="衰减正弦 damped")

    ax.set_title("中英文混排测试 — Line Plot Test")
    ax.set_xlabel("时间 Time (秒 s)")
    ax.set_ylabel("振幅 Amplitude (V)")
    ax.legend(loc="upper right")

    out = OUTPUT_DIR / "chinese_line_plot.png"
    fig.savefig(out, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  Saved: {out}")
    return out


def test_chinese_bar_chart():
    """Generate a bar chart with Chinese labels."""
    apply_publication_style(locale="zh")

    fig, ax = plt.subplots(figsize=(5, 3.5))

    categories = ["方法 A", "方法 B", "方法 C", "方法 D (基线)"]
    values = [85.3, 91.2, 78.6, 82.1]
    errors = [2.1, 1.8, 3.2, 2.5]
    colors = ["#E69F00", "#56B4E9", "#009E73", "#D55E00"]

    bars = ax.bar(categories, values, yerr=errors, capsize=4,
                  color=colors, edgecolor="black", linewidth=0.5)

    ax.set_title("不同方法的准确率对比")
    ax.set_ylabel("准确率 Accuracy (%)")
    ax.set_ylim(0, 100)

    for bar, val in zip(bars, values):
        ax.text(bar.get_x() + bar.get_width() / 2, bar.get_height() + 3,
                f"{val:.1f}%", ha="center", va="bottom", fontsize=8)

    out = OUTPUT_DIR / "chinese_bar_chart.png"
    fig.savefig(out, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  Saved: {out}")
    return out


def test_patch_figure():
    """Test patch_figure_cjk() auto-fixes problematic chars in a figure."""
    apply_publication_style(locale="zh")

    fig, ax = plt.subplots(figsize=(5, 3.5))

    x = np.linspace(-3, 3, 50)
    ax.plot(x, x ** 2, label="y = x\u00b2")  # superscript 2 — will be patched

    # Deliberately use problematic Unicode chars
    ax.set_title("patch_figure_cjk 测试：x\u00b2 \u2212 y\u00b3")
    ax.set_xlabel("范围 \u22123 to 3")  # Unicode minus
    ax.set_ylabel("f(x) = x\u00b2")    # superscript 2

    # Auto-fix all text in the figure
    n = patch_figure_cjk(fig)
    print(f"  patch_figure_cjk() fixed {n} text objects")

    out = OUTPUT_DIR / "patch_figure_test.png"
    fig.savefig(out, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  Saved: {out}")
    return out


def test_chinese_scatter():
    """Generate a scatter plot with negative values."""
    apply_publication_style(locale="zh")

    fig, ax = plt.subplots(figsize=(5, 3.5))

    np.random.seed(42)
    x = np.random.randn(80)
    y = 0.7 * x + np.random.randn(80) * 0.5

    ax.scatter(x, y, alpha=0.7, s=25, edgecolors="black", linewidth=0.3)
    ax.axhline(0, color="gray", linewidth=0.5, linestyle="--")
    ax.axvline(0, color="gray", linewidth=0.5, linestyle="--")

    ax.set_title("散点图：负数刻度 & 负号渲染测试")
    ax.set_xlabel("特征值 X (含负数 -3 to 3)")
    ax.set_ylabel("响应变量 Y")

    out = OUTPUT_DIR / "chinese_scatter.png"
    fig.savefig(out, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  Saved: {out}")
    return out


def test_english_default():
    """Verify English mode still works (backward compatibility)."""
    apply_publication_style(locale="en")

    fig, ax = plt.subplots(figsize=(3.5, 2.5))

    x = np.linspace(0, 10, 50)
    ax.plot(x, np.sin(x), label="sin(x)")

    ax.set_title("English Default (no CJK)")
    ax.set_xlabel("Time (s)")
    ax.set_ylabel("Amplitude (V)")
    ax.legend()

    out = OUTPUT_DIR / "english_default.png"
    fig.savefig(out, dpi=150, bbox_inches="tight")
    plt.close(fig)
    print(f"  Saved: {out}")
    return out


def main():
    """Run all CJK font tests."""
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    print(f"Output: {OUTPUT_DIR}\n")

    tests = [
        ("1. Font detection",                    test_font_detection),
        ("2. sanitize_cjk_text() unit tests",    test_sanitize_function),
        ("3. Line plot (中英文混排)",            test_chinese_line_plot),
        ("4. Bar chart (柱状图)",                test_chinese_bar_chart),
        ("5. Scatter (负号测试)",                test_chinese_scatter),
        ("6. patch_figure_cjk() auto-fix",       test_patch_figure),
        ("7. English default (backward compat)", test_english_default),
    ]

    for title, fn in tests:
        print(f"[{title}]")
        fn()
        print()

    outputs = list(OUTPUT_DIR.glob("*.png"))
    print(f"Done: {len(outputs)} images in {OUTPUT_DIR}/")
    for p in sorted(outputs):
        print(f"  {p.name}  ({p.stat().st_size / 1024:.1f} KB)")


if __name__ == "__main__":
    main()
