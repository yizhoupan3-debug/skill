# Image API Reference

This skill uses the official OpenAI Images API.

## Endpoints

- Generation: `https://api.openai.com/v1/images/generations`
- Edits: `https://api.openai.com/v1/images/edits` (DALL-E 2 only)
- Variations: `https://api.openai.com/v1/images/variations` (DALL-E 2 only)

## Authentication

Authentication is handled via the `OPENAI_API_KEY` environment variable.

## Models

- `dall-e-3`: Default for generation. Supports higher quality and complex prompts.
- `dall-e-2`: Required for image editing and variations.

## Response Format

The Rust CLI defaults to requesting `b64_json` or `url` depending on the use case.
Official OpenAI responses contain a `data` array with the image data.
