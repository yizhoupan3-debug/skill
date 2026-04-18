# Responses image-generation quick reference

The system copy uses the same direct VibeProxy Local Responses path as the local `imagegen` skill.

- Endpoint: `POST /v1/responses`
- Tool: `{"type": "image_generation"}`
- Edit mode: `action: "edit"` with `input_image`
- Output extraction: decode `image_generation_call.result`
