# Responses image-generation quick reference

This file documents the direct VibeProxy Local path used by `rust_tools/image_gen_rs`.

## Endpoint

- `POST /v1/responses`

Default local URL:

- `http://127.0.0.1:8318/v1/responses`

## Minimal generate payload

```json
{
  "model": "gpt-5.4",
  "input": "Generate a simple red square centered on a white background.",
  "tools": [
    {
      "type": "image_generation"
    }
  ]
}
```

## Tool options used by this CLI

Inside `tools[0]`:

- `type: "image_generation"`
- `size`
- `quality`
- `background`
- `output_format`
- `output_compression`
- `moderation`
- `input_fidelity` for edit
- `action: "edit"` for edit mode

## Edit payload shape

The script uses `input_image` items inside `input` and sends local files as Base64 data URLs.

```json
{
  "model": "gpt-5.4",
  "input": [
    {
      "role": "user",
      "content": [
        {"type": "input_text", "text": "Change only the background to a warm sunset."},
        {"type": "input_image", "image_url": "data:image/png;base64,..."}
      ]
    }
  ],
  "tools": [
    {
      "type": "image_generation",
      "action": "edit",
      "input_fidelity": "high"
    }
  ]
}
```

## Output extraction

Look for `output[*].type == "image_generation_call"` and decode `result` from Base64 into a file.

## Notes

- This script uses the Responses API image-generation tool, not the legacy image-generation endpoints.
- The script keeps `generate`, `edit`, and `generate-batch`, but internally uses repeated single-image Responses calls.
- Local input images are sent as data URLs.
- `--mask` is currently unsupported in this script because that would require a separate mask/file-upload path.
- The default model is `gpt-5.4`, but `--model` can override it.
