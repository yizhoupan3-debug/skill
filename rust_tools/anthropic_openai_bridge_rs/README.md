# Anthropic OpenAI Bridge RS

Tiny Rust bridge for Claude Code style Anthropic Messages requests.

It accepts:

- `POST /v1/messages`
- `POST /v1/messages/count_tokens`
- `GET /v1/models`
- `GET /health`

It forwards to an OpenAI-compatible `/v1/chat/completions` backend.

Example:

```sh
cargo run --manifest-path rust_tools/anthropic_openai_bridge_rs/Cargo.toml --release -- \
  --listen 127.0.0.1:8320 \
  --upstream-base http://127.0.0.1:8318/v1 \
  --upstream-key sk-dummy \
  --model gpt-5.5
```

Then point Claude Code at:

```sh
ANTHROPIC_BASE_URL=http://127.0.0.1:8320
ANTHROPIC_AUTH_TOKEN=sk-dummy
ANTHROPIC_MODEL=gpt-5.5
```
