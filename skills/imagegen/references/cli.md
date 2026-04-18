# CLI reference (`scripts/image_gen.py`)

This CLI is the default image-generation path for this skill library.

## What it does

- `generate`: create a new image from a prompt
- `edit`: edit one or more local input images via `input_image`
- `generate-batch`: run many generation jobs from a JSONL file

Real runs use:

- VibeProxy Local Responses endpoint: `http://127.0.0.1:8318/v1/responses`
- Built-in tool payload: `tools: [{"type": "image_generation"}]`

`--dry-run` prints the computed request shape and output path(s) without sending the request.

## Quick start

Set a stable path to the skill CLI:

```bash
export CODEX_HOME="${CODEX_HOME:-$HOME/.codex}"
export IMAGE_GEN="$CODEX_HOME/skills/imagegen/scripts/image_gen.py"
```

Dry-run:

```bash
python "$IMAGE_GEN" generate --prompt "Test" --dry-run
```

Generate:

```bash
python "$IMAGE_GEN" generate \
  --prompt "A cozy alpine cabin at dawn" \
  --out output/imagegen/alpine-cabin.png
```

Edit:

```bash
python "$IMAGE_GEN" edit \
  --image input.png \
  --prompt "Replace only the background with a warm sunset" \
  --out output/imagegen/sunset-edit.png
```

Batch:

```bash
python "$IMAGE_GEN" generate-batch \
  --input tmp/imagegen/prompts.jsonl \
  --out-dir output/imagegen/batch
```

## Endpoint and auth

- Default endpoint: `http://127.0.0.1:8318/v1/responses`
- Override with `VIBEPROXY_RESPONSES_URL`
- This path uses only the local VibeProxy endpoint
- Optional bearer auth can be provided with `VIBEPROXY_BEARER_TOKEN` or `VIBEPROXY_API_KEY`

## Defaults

- Model: `gpt-5.4`
- Size: `1024x1024`
- Quality: `auto`
- Output format: `png`
- Default output path: `output/imagegen/output.png`

## Guardrails

- Use the bundled CLI directly (`python "$IMAGE_GEN" ...`).
- Do not create one-off image runners unless the user explicitly asks for a custom wrapper.
- Keep final outputs in the workspace for project-bound assets.
- `--mask` is currently unsupported on this direct Responses path.

## Common recipes

Generate with prompt augmentation fields:

```bash
python "$IMAGE_GEN" generate \
  --prompt "A minimal hero image of a ceramic coffee mug" \
  --use-case "product-mockup" \
  --style "clean product photography" \
  --composition "wide product shot with usable negative space for page copy" \
  --constraints "no logos, no text" \
  --out output/imagegen/mug-hero.png
```

Generate and write a downscaled web copy:

```bash
python "$IMAGE_GEN" generate \
  --prompt "A cozy alpine cabin at dawn" \
  --downscale-max-dim 1024 \
  --out output/imagegen/alpine-cabin.png
```

Batch JSONL example:

```bash
mkdir -p tmp/imagegen output/imagegen/batch
cat > tmp/imagegen/prompts.jsonl << 'EOF'
{"prompt":"Cavernous hangar interior with a compact shuttle parked near the center","use_case":"stylized-concept","composition":"wide-angle, low-angle","lighting":"volumetric light rays through drifting fog","constraints":"no logos or trademarks; no watermark","size":"1536x1024"}
{"prompt":"Gray wolf in profile in a snowy forest","use_case":"photorealistic-natural","composition":"eye-level","constraints":"no logos or trademarks; no watermark","size":"1024x1024"}
EOF

python "$IMAGE_GEN" generate-batch \
  --input tmp/imagegen/prompts.jsonl \
  --out-dir output/imagegen/batch \
  --concurrency 5
```

## Notes

- `--n` is preserved for compatibility; the script realizes multiple variants by sending repeated single-image Responses calls.
- `edit` accepts repeated `--image` flags and sends them as ordered `input_image` items.
- `input-fidelity` is edit-only.
- Downscaling requires Pillow.

## See also

- `references/image-api.md`
- `references/codex-network.md`
- `references/sample-prompts.md`
