import contextlib
import matplotlib.pyplot as plt
import matplotlib as mpl
import matplotlib.font_manager as fm

# ---------------------------------------------------------------------------
# CJK glyph compatibility — character substitution table
# ---------------------------------------------------------------------------
# Common CJK fonts (PingFang SC, Hiragino Sans GB, STHeiti, etc.) are missing
# several Unicode characters that frequently appear in scientific labels.
# This table maps each problematic char to a safe ASCII/Latin equivalent.
# ---------------------------------------------------------------------------

_CJK_CHAR_SUBSTITUTIONS = {
    # Minus and Dashes
    "\u2212": "-", "\u2010": "-", "\u2011": "-", "\u2012": "-", "\u2013": "-", "\u2014": "-",
    # Superscript digits
    "\u00B2": "2", "\u00B3": "3", "\u00B9": "1", "\u2070": "0", "\u2074": "4", "\u2075": "5",
    "\u2076": "6", "\u2077": "7", "\u2078": "8", "\u2079": "9", "\u207B": "-",
    # Subscript digits
    "\u2080": "0", "\u2081": "1", "\u2082": "2", "\u2083": "3", "\u2084": "4",
    "\u2085": "5", "\u2086": "6", "\u2087": "7", "\u2088": "8", "\u2089": "9",
    # Common Scientific/Math Symbols (safe fallbacks)
    "\u00B1": "+/-",   # PLUS-MINUS
    "\u00D7": "x",     # MULTIPLICATION
    "\u00F7": "/",     # DIVISION
    "\u221E": "inf",   # INFINITY
    "\u2248": "~",     # ALMOST EQUAL TO
    "\u2264": "<=",    # LESS-THAN OR EQUAL TO
    "\u2265": ">=",    # GREATER-THAN OR EQUAL TO
    "\u2206": "Delta",  # INCREMENT
    "\u03BC": "u",     # GREEK SMALL LETTER MU
    "\u03B1": "alpha", 
    "\u03B2": "beta",
}

_CJK_TRANS_TABLE = str.maketrans(_CJK_CHAR_SUBSTITUTIONS)


def sanitize_cjk_text(text):
    """Replace Unicode characters that are missing from common CJK fonts."""
    if not isinstance(text, str):
        return text
    return text.translate(_CJK_TRANS_TABLE)


def patch_figure_cjk(fig):
    """Sanitize all text artists in a matplotlib figure for CJK compat."""
    modified = 0
    for text_obj in fig.findobj(mpl.text.Text):
        original = text_obj.get_text()
        cleaned = sanitize_cjk_text(original)
        if cleaned != original:
            text_obj.set_text(cleaned)
            modified += 1
    return modified


def _find_cjk_font():
    """Auto-detect available CJK fonts on the system."""
    preferred = [
        "PingFang SC", "Hiragino Sans GB", "STHeiti", "Songti SC",
        "Kaiti SC", "Arial Unicode MS", "Noto Sans SC", "Noto Sans CJK SC",
        "Source Han Sans SC", "WenQuanYi Micro Hei", "SimHei", "Microsoft YaHei"
    ]
    available = {f.name for f in fm.fontManager.ttflist}
    for name in preferred:
        if name in available:
            return name
    return None


@contextlib.contextmanager
def publication_style(style="default", palette=None, **kwargs):
    """Context manager for publication-grade matplotlib styles.
    
    Usage:
        with publication_style('nature', palette='vibrant'):
            plt.plot(x, y)
    """
    orig_rc = mpl.rcParams.copy()
    try:
        apply_publication_style(style=style, palette=palette, **kwargs)
        yield
    finally:
        plt.rcParams.update(orig_rc)


def apply_publication_style(
    style: str = "default",
    palette: str = "okabe-ito",
    font_family: str = None,
    font_size: int = None,
    dpi: int = 300,
    locale: str = "en",
):
    """Apply publication-grade matplotlib defaults."""
    
    # Colorblind-safe palettes (Paul Tol & Okabe-Ito)
    PALETTES = {
        "okabe-ito": ["#E69F00", "#56B4E9", "#009E73", "#F0E442", "#0072B2", "#D55E00", "#CC79A7", "#000000"],
        "vibrant": ["#0077BB", "#33BBEE", "#009988", "#EE7733", "#CC3311", "#EE3377", "#BBBBBB"],
        "muted": ["#332288", "#88CCEE", "#44AA99", "#117733", "#999933", "#DDCC77", "#CC6677", "#882255", "#AA4499"],
        "bright": ["#4477AA", "#EE6677", "#228833", "#CCBB44", "#66CCEE", "#AA3377", "#BBBBBB"],
        "high-contrast": ["#004488", "#DDAA33", "#BB5566"],
    }

    selected_palette = PALETTES.get(palette.lower() if palette else "", PALETTES["okabe-ito"])
    base_params = {
        "font.size": 10,
        "axes.labelsize": 10,
        "axes.titlesize": 11,
        "legend.fontsize": 9,
        "xtick.labelsize": 9,
        "ytick.labelsize": 9,
        "lines.linewidth": 1.5,
        "lines.markersize": 5,
        "axes.linewidth": 0.8,
        "axes.grid": False,
        "axes.spines.top": False,
        "axes.spines.right": False,
        "xtick.direction": "out",
        "ytick.direction": "out",
        "legend.frameon": False,
        "figure.dpi": dpi,
        "savefig.dpi": dpi,
        "savefig.bbox": "tight",
        "savefig.pad_inches": 0.05,
        "pdf.fonttype": 42,
        "ps.fonttype": 42,
        "axes.unicode_minus": False,
        "axes.prop_cycle": mpl.cycler(color=selected_palette),
    }

    # Journal Overrides
    journal_styles = {
        "ieee": {
            "font.family": "serif",
            "font.serif": ["Times New Roman", "Times", "DejaVu Serif"],
            "figure.figsize": (3.5, 2.5),
        },
        "nature": {
            "font.family": "sans-serif",
            "font.sans-serif": ["Arial", "Helvetica", "DejaVu Sans"],
            "figure.figsize": (3.5, 2.8),
            "font.size": 7, "axes.labelsize": 7, "axes.titlesize": 8,
            "legend.fontsize": 6, "xtick.labelsize": 6, "ytick.labelsize": 6,
        },
        "science": {
            "font.family": "sans-serif",
            "font.sans-serif": ["Arial", "Helvetica", "DejaVu Sans"],
            "figure.figsize": (3.5, 3.5),
            "font.size": 8, "axes.labelsize": 8, "axes.titlesize": 9,
        },
        "cell": {
            "font.family": "sans-serif",
            "font.sans-serif": ["Helvetica", "Arial", "DejaVu Sans"],
            "figure.figsize": (3.5, 3.5),
            "font.size": 8, "axes.labelsize": 8,
        },
        "lancet": {
            "font.family": "sans-serif",
            "font.sans-serif": ["Univers", "Helvetica", "Arial"],
            "figure.figsize": (3.5, 2.8),
            "font.size": 9,
        },
        "bmj": {
            "font.family": "sans-serif",
            "font.sans-serif": ["Verdana", "Arial", "Helvetica"],
            "figure.figsize": (3.5, 2.8),
            "font.size": 9,
        },
        "neurips": {
            "font.family": "serif",
            "font.serif": ["Computer Modern Roman", "Times New Roman"],
            "figure.figsize": (5.5, 3.5),
        }
    }

    plt.rcParams.update(base_params)
    if style.lower() in journal_styles:
        plt.rcParams.update(journal_styles[style.lower()])
    
    if font_family: plt.rcParams["font.family"] = font_family
    if font_size: plt.rcParams["font.size"] = font_size

    # CJK Support
    if locale == "zh":
        cjk_font = _find_cjk_font()
        if cjk_font:
            plt.rcParams.update({
                "font.family": "sans-serif",
                "font.sans-serif": [cjk_font, "DejaVu Sans", "Arial"],
            })
        else:
            import warnings
            warnings.warn("No CJK font found. Chinese text may render as tofu.")

# Auto-apply default if not imported as module
if __name__ == "__main__":
    apply_publication_style()
else:
    # Initial load default
    apply_publication_style()
