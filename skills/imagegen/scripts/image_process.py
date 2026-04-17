#!/usr/bin/env python3
"""
Non-AI image processing CLI for the imagegen skill.

Usage:
    python3 image_process.py resize input.png --width 800
    python3 image_process.py crop input.png --box 100,100,500,400
    python3 image_process.py convert input.png --format webp --quality 85
    python3 image_process.py enhance input.png --brightness 1.2 --contrast 1.3
    python3 image_process.py watermark input.png --text "© 2026"
    python3 image_process.py info input.png

Requires: Pillow (pip install Pillow)
"""

import argparse
import sys
from pathlib import Path

try:
    from PIL import Image, ImageEnhance, ImageFilter, ImageDraw, ImageFont
except ImportError:
    print("ERROR: Pillow not found. Install it:")
    print("  pip install Pillow")
    print("  uv pip install Pillow")
    sys.exit(1)


def cmd_resize(args):
    """Resize an image maintaining aspect ratio."""
    img = Image.open(args.input)
    if args.width and args.height:
        img = img.resize((args.width, args.height), Image.LANCZOS)
    elif args.width:
        ratio = args.width / img.width
        img = img.resize((args.width, int(img.height * ratio)), Image.LANCZOS)
    elif args.height:
        ratio = args.height / img.height
        img = img.resize((int(img.width * ratio), args.height), Image.LANCZOS)
    elif args.scale:
        w = int(img.width * args.scale)
        h = int(img.height * args.scale)
        img = img.resize((w, h), Image.LANCZOS)
    else:
        print("ERROR: specify --width, --height, or --scale")
        sys.exit(1)

    out = args.output or _auto_output(args.input, "_resized")
    img.save(out)
    print(f"OK: {args.input} -> {out} ({img.width}x{img.height})")


def cmd_crop(args):
    """Crop an image to a bounding box."""
    img = Image.open(args.input)
    parts = [int(x) for x in args.box.split(",")]
    if len(parts) != 4:
        print("ERROR: --box must be left,upper,right,lower")
        sys.exit(1)
    cropped = img.crop(tuple(parts))
    out = args.output or _auto_output(args.input, "_cropped")
    cropped.save(out)
    print(f"OK: {args.input} -> {out} ({cropped.width}x{cropped.height})")


def cmd_convert(args):
    """Convert image format."""
    img = Image.open(args.input)
    fmt = args.format.upper()
    if fmt in ("JPEG", "JPG") and img.mode == "RGBA":
        img = img.convert("RGB")

    ext = args.format.lower()
    if ext == "jpg":
        ext = "jpeg"
    out = args.output or str(Path(args.input).with_suffix(f".{args.format.lower()}"))

    save_kwargs = {}
    if args.quality and fmt in ("JPEG", "JPG", "WEBP"):
        save_kwargs["quality"] = args.quality

    img.save(out, **save_kwargs)
    print(f"OK: {args.input} -> {out}")


def cmd_enhance(args):
    """Enhance image brightness, contrast, color, sharpness."""
    img = Image.open(args.input)

    if args.brightness != 1.0:
        img = ImageEnhance.Brightness(img).enhance(args.brightness)
    if args.contrast != 1.0:
        img = ImageEnhance.Contrast(img).enhance(args.contrast)
    if args.color != 1.0:
        img = ImageEnhance.Color(img).enhance(args.color)
    if args.sharpness != 1.0:
        img = ImageEnhance.Sharpness(img).enhance(args.sharpness)
    if args.denoise:
        img = img.filter(ImageFilter.GaussianBlur(radius=args.denoise))

    out = args.output or _auto_output(args.input, "_enhanced")
    img.save(out)
    print(f"OK: {args.input} -> {out}")


def cmd_watermark(args):
    """Add text watermark to image."""
    img = Image.open(args.input).convert("RGBA")
    overlay = Image.new("RGBA", img.size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(overlay)

    try:
        font = ImageFont.truetype(args.font, args.size)
    except (OSError, IOError):
        font = ImageFont.load_default()
        print(f"WARN: font '{args.font}' not found, using default")

    x = args.x if args.x is not None else 10
    y = args.y if args.y is not None else img.height - args.size - 10
    opacity = int(255 * args.opacity)
    draw.text((x, y), args.text, fill=(255, 255, 255, opacity), font=font)

    img = Image.alpha_composite(img, overlay)
    out = args.output or _auto_output(args.input, "_watermarked")
    img.convert("RGB").save(out)
    print(f"OK: {args.input} -> {out}")


def cmd_info(args):
    """Show image metadata."""
    img = Image.open(args.input)
    print(f"File:   {args.input}")
    print(f"Size:   {img.width}x{img.height}")
    print(f"Mode:   {img.mode}")
    print(f"Format: {img.format}")
    if hasattr(img, "info"):
        dpi = img.info.get("dpi")
        if dpi:
            print(f"DPI:    {dpi}")


def _auto_output(input_path, suffix):
    """Generate output path with suffix."""
    p = Path(input_path)
    return str(p.parent / f"{p.stem}{suffix}{p.suffix}")


def main():
    """Parse arguments and dispatch subcommands."""
    parser = argparse.ArgumentParser(description="Non-AI image processing CLI")
    sub = parser.add_subparsers(dest="command", required=True)

    # resize
    p_resize = sub.add_parser("resize", help="Resize image")
    p_resize.add_argument("input")
    p_resize.add_argument("--width", type=int)
    p_resize.add_argument("--height", type=int)
    p_resize.add_argument("--scale", type=float, help="Scale factor (e.g. 0.5)")
    p_resize.add_argument("-o", "--output")

    # crop
    p_crop = sub.add_parser("crop", help="Crop image")
    p_crop.add_argument("input")
    p_crop.add_argument("--box", required=True, help="left,upper,right,lower")
    p_crop.add_argument("-o", "--output")

    # convert
    p_convert = sub.add_parser("convert", help="Convert format")
    p_convert.add_argument("input")
    p_convert.add_argument("--format", required=True, help="webp, jpeg, png, etc.")
    p_convert.add_argument("--quality", type=int, help="Quality 1-100")
    p_convert.add_argument("-o", "--output")

    # enhance
    p_enhance = sub.add_parser("enhance", help="Enhance image")
    p_enhance.add_argument("input")
    p_enhance.add_argument("--brightness", type=float, default=1.0)
    p_enhance.add_argument("--contrast", type=float, default=1.0)
    p_enhance.add_argument("--color", type=float, default=1.0)
    p_enhance.add_argument("--sharpness", type=float, default=1.0)
    p_enhance.add_argument("--denoise", type=float, default=0, help="Blur radius")
    p_enhance.add_argument("-o", "--output")

    # watermark
    p_wm = sub.add_parser("watermark", help="Add text watermark")
    p_wm.add_argument("input")
    p_wm.add_argument("--text", required=True)
    p_wm.add_argument("--font", default="arial.ttf")
    p_wm.add_argument("--size", type=int, default=36)
    p_wm.add_argument("--opacity", type=float, default=0.5, help="0.0-1.0")
    p_wm.add_argument("--x", type=int)
    p_wm.add_argument("--y", type=int)
    p_wm.add_argument("-o", "--output")

    # info
    p_info = sub.add_parser("info", help="Show image info")
    p_info.add_argument("input")

    args = parser.parse_args()
    commands = {
        "resize": cmd_resize,
        "crop": cmd_crop,
        "convert": cmd_convert,
        "enhance": cmd_enhance,
        "watermark": cmd_watermark,
        "info": cmd_info,
    }
    commands[args.command](args)


if __name__ == "__main__":
    main()
