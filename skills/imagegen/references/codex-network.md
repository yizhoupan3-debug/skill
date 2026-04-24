# Codex network / local endpoint notes

This skill's default image path calls a local VibeProxy endpoint:

- `http://127.0.0.1:8318/v1/responses`

## Why this path exists

- The local VibeProxy Responses path has been directly verified to return `image_generation_call`.
- This skill standardizes on one direct local route instead of depending on runtime-managed tool injection.
- Using the bundled CLI keeps execution deterministic and inspectable.

## Network expectations

- The default target is loopback only (`127.0.0.1`), not the public OpenAI endpoint.
- If the local proxy is running, the call is usually just a normal local HTTP POST.
- If the local proxy is down, requests will fail before any model-side image generation begins.

## Auth expectations

- This path uses no OpenAI API credential.
- If your local VibeProxy requires bearer auth, provide it with `VIBEPROXY_BEARER_TOKEN` or `VIBEPROXY_API_KEY`.

## Troubleshooting

- First verify the proxy is listening on `127.0.0.1:8318`.
- Then run `cargo run --manifest-path /Users/joe/Documents/skill/rust_tools/image_gen_rs/Cargo.toml -- generate --prompt "Test" --dry-run` to confirm the request shape.
- If live calls fail, compare the bundled CLI payload against a direct working `curl` to `/v1/responses`.
