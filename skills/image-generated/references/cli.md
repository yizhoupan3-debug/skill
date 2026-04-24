# CLI reference (`rust_tools/image_gen_rs`)

This Rust CLI is the default image-generation path for this skill library.

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
export IMAGE_GEN_MANIFEST="/Users/joe/Documents/skill/rust_tools/image_gen_rs/Cargo.toml"
```

Dry-run:

```bash
cargo run --manifest-path "$IMAGE_GEN_MANIFEST" -- generate --prompt "Test" --dry-run
```

Generate:

```bash
cargo run --manifest-path "$IMAGE_GEN_MANIFEST" -- generate \
  --prompt "A cozy alpine cabin at dawn" \
  --out output/image-generated/alpine-cabin.png
```

Edit:

```bash
cargo run --manifest-path "$IMAGE_GEN_MANIFEST" -- edit \
  --image input.png \
  --prompt "Replace only the background with a warm sunset" \
  --out output/image-generated/sunset-edit.png
```

Batch:

```bash
cargo run --manifest-path "$IMAGE_GEN_MANIFEST" -- generate-batch \
  --input tmp/image-generated/prompts.jsonl \
  --out-dir output/image-generated/batch
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
- Default output path: `output/image-generated/output.png`
- `--dry-run` is preview-only and does not create output directories.

## Guardrails

- Use the bundled Rust CLI directly (`cargo run --manifest-path "$IMAGE_GEN_MANIFEST" -- ...`).
- Do not create one-off image runners unless the user explicitly asks for a custom wrapper.
- Keep final outputs in the workspace for project-bound assets.
- `--mask` is currently unsupported on this direct Responses path.

## Common recipes

Generate with prompt augmentation fields:

```bash
cargo run --manifest-path "$IMAGE_GEN_MANIFEST" -- generate \
  --prompt "A minimal hero image of a ceramic coffee mug" \
  --use-case "product-mockup" \
  --style "clean product photography" \
  --composition "wide product shot with usable negative space for page copy" \
  --constraints "no logos, no text" \
  --out output/image-generated/mug-hero.png
```

Generate and write a downscaled web copy:

```bash
cargo run --manifest-path "$IMAGE_GEN_MANIFEST" -- generate \
  --prompt "A cozy alpine cabin at dawn" \
  --downscale-max-dim 1024 \
  --out output/image-generated/alpine-cabin.png
```

Batch JSONL example:

```bash
mkdir -p tmp/image-generated output/image-generated/batch
cat > tmp/image-generated/prompts.jsonl << 'EOF'
{"prompt":"Cavernous hangar interior with a compact shuttle parked near the center","use_case":"stylized-concept","composition":"wide-angle, low-angle","lighting":"volumetric light rays through drifting fog","constraints":"no logos or trademarks; no watermark","size":"1536x1024"}
{"prompt":"Gray wolf in profile in a snowy forest","use_case":"photorealistic-natural","composition":"eye-level","constraints":"no logos or trademarks; no watermark","size":"1024x1024"}
EOF

cargo run --manifest-path "$IMAGE_GEN_MANIFEST" -- generate-batch \
  --input tmp/image-generated/prompts.jsonl \
  --out-dir output/image-generated/batch \
  --concurrency 5
```

## Notes

- `--n` is preserved for compatibility; the CLI realizes multiple variants by sending repeated single-image Responses calls.
- `--out-dir` is available on `generate`, `edit`, and `generate-batch`; batch output names use prompt-derived slugs and fall back to `image` for non-ASCII prompts.
- `edit` accepts repeated `--image` flags and sends them as ordered `input_image` items.
- `input-fidelity` is edit-only.
- `--output-compression` must be between `0` and `100`.
- Downscaling is handled by Rust and requires no Python/Pillow dependency.

## See also

- `references/image-api.md`
- `references/codex-network.md`
- `references/sample-prompts.md`
