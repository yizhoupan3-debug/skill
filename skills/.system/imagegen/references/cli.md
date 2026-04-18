# CLI reference (`scripts/image_gen.py`)

This system copy mirrors the direct image-generation path used by `imagegen`.

- Endpoint: `http://127.0.0.1:8318/v1/responses`
- Tool payload: `tools: [{"type": "image_generation"}]`
- Commands: `generate`, `edit`, `generate-batch`
- Auth: `VIBEPROXY_BEARER_TOKEN` or `VIBEPROXY_API_KEY`

Use `python "$IMAGE_GEN" generate --prompt "Test" --dry-run` to preview the request shape.
