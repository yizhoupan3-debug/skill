# Anthropic OpenAI Bridge RS

Tiny Rust bridge for Claude Code style Anthropic Messages requests.

The default path is optimized for Claude Code running a GPT backend: Claude's
requested model name is replaced with `AOB_MODEL`, Claude-only thinking blocks
are stripped, and stream obfuscation is omitted unless explicitly enabled.

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

Useful loss-control knobs:

- `AOB_PRESERVE_REQUEST_MODEL=false`: keep GPT model routing stable instead of
  forwarding Claude model aliases upstream.
- `AOB_SYSTEM_ROLE=developer`: map Anthropic `system` onto OpenAI's developer
  role; set `system` for older upstreams.
- `AOB_REASONING_EFFORT=low|medium|high`: optionally force GPT reasoning
  effort rather than relying on Claude-side prompt text.
- `AOB_STREAM_OBFUSCATION=omit|false|true`: default `false` matches OpenAI's
  non-obfuscated stream shape while avoiding obfuscation bytes.
- `AOB_MAX_TOKENS_FIELD=auto`: uses `max_completion_tokens` for GPT-5/o-series
  model names and `max_tokens` for older chat-compatible names.
- `AOB_STREAM_HEARTBEAT_SECS=5`: emits SSE comments during upstream pauses so
  Claude Code keeps the stream alive and feels less stuck.

Then point Claude Code at:

```sh
ANTHROPIC_BASE_URL=http://127.0.0.1:8320
ANTHROPIC_AUTH_TOKEN=sk-dummy
ANTHROPIC_MODEL=gpt-5.5
```
