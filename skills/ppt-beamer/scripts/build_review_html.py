#!/usr/bin/env python3
from __future__ import annotations

import argparse
import html
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Build a local HTML gallery for rendered Beamer slide PNGs."
    )
    parser.add_argument("images_dir", help="Directory containing rendered slide PNGs")
    parser.add_argument(
        "--output",
        default="review.html",
        help="Output HTML path. Defaults to review.html inside the images directory.",
    )
    parser.add_argument(
        "--title",
        default="Beamer Slide Review",
        help="HTML document title",
    )
    return parser.parse_args()


def build_html(title: str, image_paths: list[Path]) -> str:
    cards = []
    for image_path in image_paths:
        label = image_path.stem
        src = html.escape(image_path.name)
        cards.append(
            f"""
<article class="card">
  <header>{html.escape(label)}</header>
  <img src="{src}" alt="{html.escape(label)}">
</article>""".strip()
        )
    cards_html = "\n".join(cards)
    return f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{html.escape(title)}</title>
  <style>
    :root {{
      color-scheme: light;
      --bg: #f3f5f7;
      --card: #ffffff;
      --text: #17324d;
      --muted: #5c6b77;
      --border: #d9e0e6;
    }}
    * {{ box-sizing: border-box; }}
    body {{
      margin: 0;
      font-family: "Helvetica Neue", Helvetica, Arial, sans-serif;
      background: var(--bg);
      color: var(--text);
    }}
    main {{
      max-width: 1600px;
      margin: 0 auto;
      padding: 24px;
    }}
    h1 {{
      margin: 0 0 8px;
      font-size: 28px;
    }}
    p {{
      margin: 0 0 20px;
      color: var(--muted);
    }}
    .grid {{
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(360px, 1fr));
      gap: 18px;
    }}
    .card {{
      background: var(--card);
      border: 1px solid var(--border);
      border-radius: 12px;
      padding: 14px;
      box-shadow: 0 8px 30px rgba(23, 50, 77, 0.08);
    }}
    .card header {{
      margin-bottom: 10px;
      font-size: 14px;
      font-weight: 700;
      letter-spacing: 0.02em;
    }}
    img {{
      display: block;
      width: 100%;
      height: auto;
      border-radius: 8px;
      border: 1px solid var(--border);
      background: white;
    }}
  </style>
</head>
<body>
  <main>
    <h1>{html.escape(title)}</h1>
    <p>Use this gallery for fast scan-based QA. Check clipping, overlap, empty space, and unreadable labels.</p>
    <section class="grid">
      {cards_html}
    </section>
  </main>
</body>
</html>
"""


def main() -> int:
    args = parse_args()
    images_dir = Path(args.images_dir).expanduser().resolve()
    if not images_dir.is_dir():
        raise SystemExit(f"Images directory not found: {images_dir}")

    image_paths = sorted(images_dir.glob("*.png"))
    if not image_paths:
        raise SystemExit(f"No PNG files found in: {images_dir}")

    output = Path(args.output)
    if not output.is_absolute():
        output = images_dir / output

    output.write_text(build_html(args.title, image_paths), encoding="utf-8")
    print(output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
