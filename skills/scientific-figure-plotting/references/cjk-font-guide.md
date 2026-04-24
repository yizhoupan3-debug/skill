# CJK Font Guide for matplotlib on macOS

## Quick Start

Set `matplotlib` font parameters directly in the figure script.

This automatically:
1. Scans system fonts for a usable CJK typeface
2. Sets `font.sans-serif` to the detected font
3. Disables `axes.unicode_minus` (fixes minus sign rendering)

---

## macOS Available Fonts (priority order)

| Font | Type | Notes |
|------|------|-------|
| **PingFang SC** | Sans-serif | macOS 10.11+, best quality |
| **Hiragino Sans GB** | Sans-serif | macOS built-in, good quality |
| **STHeiti** | Sans-serif | macOS built-in, older |
| **Songti SC** | Serif | macOS built-in, formal |
| **Kaiti SC** | Script | macOS built-in, calligraphic |
| **Arial Unicode MS** | Sans-serif | Wide coverage, fallback |

## If No Font Is Detected

Install Noto Sans SC (Google's open CJK font):

```bash
# macOS Homebrew
brew install --cask font-noto-sans-cjk-sc

# pip (for matplotlib only)
pip install matplotlib-cjk-fonts
```

After installing, clear matplotlib's font cache:

```python
import matplotlib.font_manager as fm
fm._load_fontmanager(try_read_cache=False)
```

Or delete the cache file:

```bash
rm -rf ~/.matplotlib/fontlist-*.json
rm -rf ~/.cache/matplotlib/fontlist-*.json
```

---

## Common Issues

### 1. Tofu (□□□) boxes

**Cause**: matplotlib cannot find a CJK font.

**Fix**: Use `locale="zh"` (auto-detects) or manually specify:

```python
plt.rcParams["font.sans-serif"] = ["PingFang SC", "DejaVu Sans"]
plt.rcParams["axes.unicode_minus"] = False
```

### 2. Minus sign shows as a box

**Cause**: CJK fonts lack the Unicode minus character U+2212.

**Fix**: `plt.rcParams["axes.unicode_minus"] = False` — this uses ASCII
hyphen-minus instead.

### 3. Unicode minus (−) still shows as a box in labels

**Cause**: `axes.unicode_minus = False` only affects **matplotlib-generated
tick labels**. If you write `"−3"` (U+2212) directly in `set_xlabel()` etc.,
the CJK font may not have that glyph.

**Fix**: Use ASCII hyphen-minus `-` in hand-written strings:

```python
# Bad — uses Unicode minus U+2212
ax.set_xlabel("Range: −3 to 3")

# Good — uses ASCII hyphen-minus
ax.set_xlabel("Range: -3 to 3")
```

### 3. Special symbols (±, ×, ÷, ∞, ≈, ≤, ≥, Δ, μ) show as boxes

**Cause**: Many CJK fonts lack glyphs for common mathematical and scientific symbols.

**Fix**: Substitute these symbols with ASCII equivalents or suitable Unicode alternatives that are more widely supported.

```python
    # Bad — uses Unicode characters missing in some CJK fonts
    ax.set_xlabel("范围: −3 to 3, 误差 ±0.1, 极限 ∞")
    
    # Good: use ASCII-safe labels or a project-local text sanitizer
    
    # Supported auto-substitutions:
    # − (minus), ± (+/-), × (x), ÷ (/), ∞ (inf), ≈ (~), ≤ (<=), ≥ (>=), Δ (Delta), μ (u)
```

### 4. Font warning spam in console

**Cause**: matplotlib falls back font-by-font.

**Fix**: Set the font explicitly so it finds it on first try.

### 4. Mixed CJK + English looks bad

**Tip**: PingFang SC and Hiragino Sans GB have good Latin glyph coverage,
so mixed text looks balanced. Avoid Songti SC for mixed text (Latin glyphs
are narrow and look cramped).

---

## Manual Override Example

```python
import matplotlib.pyplot as plt

plt.rcParams.update({
    "font.family": "sans-serif",
    "font.sans-serif": ["PingFang SC", "Hiragino Sans GB", "DejaVu Sans"],
    "axes.unicode_minus": False,
})

fig, ax = plt.subplots()
ax.set_title("中英文混排标题 — Mixed Title")
ax.set_xlabel("时间 (s)")
ax.set_ylabel("振幅 (V)")
fig.savefig("test.png", dpi=150, bbox_inches="tight")
```
