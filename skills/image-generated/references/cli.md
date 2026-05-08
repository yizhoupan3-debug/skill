# Image Generation CLI

The bundled Rust CLI `rust_tools/image_gen_rs` handles communication with the OpenAI Images API.

## Usage

### Generate

```bash
cargo run --manifest-path rust_tools/image_gen_rs/Cargo.toml -- generate --prompt "A futuristic city"
```

### Edit (DALL-E 2 only)

```bash
cargo run --manifest-path rust_tools/image_gen_rs/Cargo.toml -- edit --prompt "Add a cat" --image path/to/image.png
```

## Configuration

- `OPENAI_API_KEY`: Required for authentication.
- `OPENAI_IMAGES_URL`: Optional override for the generation endpoint.

## Implementation Details

- Uses `reqwest` for HTTP requests.
- Supports both URL downloading and Base64 decoding of results.
- Automatically handles image downscaling if requested via `--downscale-max-dim`.
