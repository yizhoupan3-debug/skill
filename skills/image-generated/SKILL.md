---
name: "image-generated"
description: "Generate or edit raster images through official OpenAI API using the bundled Rust CLI."
routing_layer: L1
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - 生成图片
  - 生图
  - 编辑图片
  - 画一张图
  - 做封面图
  - image generation
  - generate image
  - OpenAI
  - DALL-E
  - image-generated
source: local
metadata:
  version: "2.0.0"
  platforms: [codex]
  tags:
    - image-generated
    - openai
    - dall-e

---

# Image Generated Skill

Generates or edits raster images for the current project through the bundled Rust CLI:

- `cargo run --manifest-path rust_tools/image_gen_rs/Cargo.toml -- generate`
- `cargo run --manifest-path rust_tools/image_gen_rs/Cargo.toml -- edit`
- `cargo run --manifest-path rust_tools/image_gen_rs/Cargo.toml -- generate-batch`

There is no Python execution path for this skill.

## Default execution path

Use the bundled Rust CLI as the default and canonical path for this skill library.

- Endpoint: `https://api.openai.com/v1/images/generations`
- Default model: `dall-e-3`
- Override endpoint with `OPENAI_IMAGES_URL`
- Authentication: Requires `OPENAI_API_KEY` environment variable.

Use the official OpenAI API as the canonical execution path in this skill library.

Reason:
- Direct access to official DALL-E models ensures the highest quality and consistency.
- Removes dependency on local proxy middleware.

## Rules

- Use `rust_tools/image_gen_rs` by default for all normal image generation and editing requests.
- Ensure `OPENAI_API_KEY` is available in the environment.
- Do not create Python or one-off image generation runners when the bundled Rust CLI already fits.
- Keep the existing command surface: `generate`, `edit`, `generate-batch`.
- For project-bound assets, save into the workspace rather than leaving finals in temp locations.
- Do not overwrite an existing asset unless the user explicitly asked for replacement.

Shared prompt guidance lives in:

- `references/prompting.md`
- `references/sample-prompts.md`
- `references/awesome-prompts.md`

CLI/runtime details live in:

- `references/cli.md`
- `references/image-api.md`
- `references/codex-network.md`
- `rust_tools/image_gen_rs`

## When to use

- Generate a new bitmap image: hero image, product shot, mockup, concept art, comic, infographic
- Edit an existing local bitmap image (DALL-E 2 only)
- Produce several variants from one or many prompts
- Use official OpenAI DALL-E models for high-quality visual assets

## When not to use

- Extending an existing SVG/icon/logo system that should stay vector-native
- Simple shapes or diagrams that are better produced directly in SVG, HTML/CSS, or canvas
- Small edits to a source asset that already exists in a deterministic native format
- Any request where the user clearly wants code-native output instead of generated raster output
- Scientific or publication figure code such as `科研出图`, matplotlib, seaborn, plotnine, or journal-style charts -> use `$scientific-figure-plotting`

## Workflow

1. Decide `generate`, `edit`, or `generate-batch`.
2. Decide whether the output is preview-only or meant for the current project.
3. Collect prompt, constraints, exact text, and any local input images up front.
4. Normalize the prompt into a short structured spec when it helps.
5. Run the bundled CLI against the OpenAI API endpoint.
6. Inspect the output for subject, text accuracy, composition, and preserved invariants.
7. Iterate with one targeted change at a time.
8. Persist only the selected finals into the workspace unless the user explicitly asked to keep discarded variants.
9. Report the final saved path, final prompt, and that the request used the official OpenAI API.

Prompt schema, taxonomy, and examples live in the prompt references above; load
only the relevant reference when the request needs that extra structure.
