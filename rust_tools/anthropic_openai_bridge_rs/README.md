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
  --listen 0.0.0.0:8320 \
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
- `AOB_MAX_REQUEST_BYTES=67108864`: caps Anthropic request bodies before JSON
  parsing so large tool transcripts do not grow memory without bound.
- `AOB_UPSTREAM_CONNECT_TIMEOUT_SECS=10`: fails bad upstream endpoints quickly
  instead of tying up bridge workers.
- `AOB_UPSTREAM_REQUEST_TIMEOUT_SECS=300`: caps non-stream upstream waits so a
  wedged completion does not hold a Claude request forever.
- `AOB_UPSTREAM_POOL_MAX_IDLE_PER_HOST=128`: keeps enough warm upstream
  connections for parallel Claude tool loops.
- `AOB_STREAM_CHANNEL_DEPTH=64`: bounds queued SSE bytes per stream while still
  allowing short upstream bursts.

Then point Claude Code at:

```sh
ANTHROPIC_BASE_URL=http://127.0.0.1:8320
ANTHROPIC_AUTH_TOKEN=sk-dummy
ANTHROPIC_MODEL=gpt-5.5
```

For Cowork workspace access, keep the bridge listening on `0.0.0.0:8320` and configure the inference provider URL to a host-reachable address for the Mac, not the workspace-local loopback address. If the app offers Docker-style host resolution, use `http://host.docker.internal:8320`; otherwise use the Mac's LAN address, for example `http://192.168.x.x:8320`.
