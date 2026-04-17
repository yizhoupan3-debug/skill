# Non-AI Image Processing Reference

Operations that do not require the OpenAI API (no `OPENAI_API_KEY` needed).
Use Pillow (PIL) for Python or ImageMagick for CLI.

## Dependencies

```bash
uv pip install Pillow
# Optional for SVG conversion:
uv pip install cairosvg
```

## Resize / Crop / Rotate

```python
from PIL import Image

img = Image.open('input.png')

# Resize (maintain aspect ratio)
img.thumbnail((800, 800))
img.save('resized.png')

# Crop (left, upper, right, lower)
cropped = img.crop((100, 100, 500, 400))
cropped.save('cropped.png')

# Rotate
rotated = img.rotate(90, expand=True)
rotated.save('rotated.png')
```

## Color / Brightness / Contrast / Sharpen

```python
from PIL import Image, ImageEnhance, ImageFilter

img = Image.open('input.png')

# Brightness (1.0 = original, >1 brighter, <1 darker)
img = ImageEnhance.Brightness(img).enhance(1.2)

# Contrast
img = ImageEnhance.Contrast(img).enhance(1.3)

# Color saturation
img = ImageEnhance.Color(img).enhance(1.5)

# Sharpen
img = img.filter(ImageFilter.SHARPEN)

# Denoise (GaussianBlur as simple denoise)
img = img.filter(ImageFilter.GaussianBlur(radius=1))

img.save('enhanced.png')
```

## Format Conversion

```python
from PIL import Image

img = Image.open('input.png')

# PNG → WebP
img.save('output.webp', 'WebP', quality=85)

# PNG → JPEG (must convert RGBA → RGB)
if img.mode == 'RGBA':
    img = img.convert('RGB')
img.save('output.jpg', 'JPEG', quality=90)

# SVG → PNG (use cairosvg)
import cairosvg
cairosvg.svg2png(url='input.svg', write_to='output.png', output_width=1024)
```

## Watermark

```python
from PIL import Image, ImageDraw, ImageFont

img = Image.open('input.png')
draw = ImageDraw.Draw(img)
font = ImageFont.truetype('arial.ttf', 36)
draw.text((10, img.height - 50), 'Watermark', fill=(255, 255, 255, 128), font=font)
img.save('watermarked.png')
```

## ImageMagick CLI (fallback)

```bash
# Resize
convert input.png -resize 800x600 output.png

# Format conversion
convert input.png output.webp

# Add text watermark
convert input.png -gravity SouthEast -pointsize 36 \
  -fill 'rgba(255,255,255,0.5)' -annotate +10+10 'Watermark' output.png

# Batch resize
mogrify -resize 50% *.png
```

## Batch Processing Pattern

```python
from pathlib import Path
from PIL import Image

input_dir = Path('input/')
output_dir = Path('output/')
output_dir.mkdir(exist_ok=True)

for img_path in input_dir.glob('*.png'):
    img = Image.open(img_path)
    img.thumbnail((1200, 1200))
    img.save(output_dir / img_path.name)
```
