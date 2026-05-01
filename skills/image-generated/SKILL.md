---
name: "image-generated"
description: "Generate or edit raster images through VibeProxy Local /v1/responses using the bundled Rust CLI."
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
  - VibeProxy
  - Responses API
  - image-generated
source: local
metadata:
  version: "1.3.0"
  platforms: [codex]
  tags:
    - image-generated
    - vibeproxy
    - responses-api

---

# Image Generated Skill

Generates or edits raster images for the current project through the bundled Rust CLI:

- `cargo run --manifest-path rust_tools/image_gen_rs/Cargo.toml -- generate`
- `cargo run --manifest-path rust_tools/image_gen_rs/Cargo.toml -- edit`
- `cargo run --manifest-path rust_tools/image_gen_rs/Cargo.toml -- generate-batch`

There is no Python execution path for this skill.

## Default execution path

Use the bundled Rust CLI as the default and canonical path for this skill library.

- Endpoint: `http://127.0.0.1:8318/v1/responses`
- Tool payload: `tools: [{"type": "image_generation"}]`
- Default model: `gpt-5.4`
- Override endpoint with `VIBEPROXY_RESPONSES_URL`
- Optional bearer auth can be supplied with `VIBEPROXY_BEARER_TOKEN` or `VIBEPROXY_API_KEY`

Use the direct Responses path as the only execution path in this skill library.

Reason:
- this VibeProxy `/v1/responses` path has been locally verified to return `image_generation_call`
- the goal here is deterministic local execution, not provider-surface guessing
- keeping one canonical route avoids provider-surface drift

## Rules

- Use `rust_tools/image_gen_rs` by default for all normal image generation and editing requests.
- Do not ask for OpenAI API credentials; this path does not use them.
- Do not create Python or one-off image generation runners when the bundled Rust CLI already fits.
- Keep the existing command surface: `generate`, `edit`, `generate-batch`.
- `edit` supports local images through `input_image` on the Responses API path.
- `--mask` is currently unsupported on this direct path; do not imply otherwise.
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
- Edit an existing local bitmap image while preserving most of it
- Produce several variants from one or many prompts
- Use one or more images as edit/reference inputs on the direct Responses API path

## When not to use

- Extending an existing SVG/icon/logo system that should stay vector-native
- Simple shapes or diagrams that are better produced directly in SVG, HTML/CSS, or canvas
- Small edits to a source asset that already exists in a deterministic native format
- Any request where the user clearly wants code-native output instead of generated raster output
- Scientific or publication figure code such as `科研出图`, matplotlib, seaborn, plotnine, or journal-style charts -> use `$scientific-figure-plotting`

## Decision tree

Think about two separate questions:

1. Is this `generate` or `edit`?
2. Is this one asset or many?

Intent:

- If the user wants to modify an existing image while preserving most of it, use `edit`.
- If the user provides images only as references for style, composition, or mood, still treat it as generation unless they explicitly want the existing image changed.
- If the user provides no image, use `generate`.

Execution strategy:

- Use `generate` for one prompt -> one or more variants.
- Use `generate-batch` for many prompts from JSONL.
- Use `edit` when local image files need to be part of the request.

## Workflow

1. Decide `generate`, `edit`, or `generate-batch`.
2. Decide whether the output is preview-only or meant for the current project.
3. Collect prompt, constraints, exact text, and any local input images up front.
4. For each input image, label its role explicitly in the prompt:
   - edit target
   - style reference
   - supporting insert/compositing input
5. Normalize the prompt into a short structured spec when it helps.
6. Run the bundled CLI against the VibeProxy Responses endpoint.
7. Inspect the output for subject, text accuracy, composition, and preserved invariants.
8. Iterate with one targeted change at a time.
9. Persist only the selected finals into the workspace unless the user explicitly asked to keep discarded variants.
10. Report the final saved path, final prompt, and that the request used the VibeProxy `/v1/responses` path.

## Prompt augmentation

Use the user's prompt specificity to decide how much augmentation is appropriate:

- If the prompt is already specific, normalize it without inventing new creative requirements.
- If the prompt is generic, add only the minimum structure needed to improve the output materially.

Allowed augmentation:

- composition/framing cues
- intended-use hints
- practical layout guidance
- explicit invariants for edits

Do not add:

- extra characters or objects that are not implied
- arbitrary brand palettes or slogans
- fake precision the user never asked for

## Shared prompt schema

```text
Use case: <taxonomy slug>
Asset type: <where the asset will be used>
Primary request: <user's main prompt>
Input images: <Image 1: role; Image 2: role> (optional)
Scene/backdrop: <environment>
Subject: <main subject>
Style/medium: <photo/illustration/3D/etc>
Composition/framing: <wide/close/top-down; placement>
Lighting/mood: <lighting + mood>
Color palette: <palette notes>
Materials/textures: <surface details>
Text (verbatim): "<exact text>"
Constraints: <must keep/must avoid>
Avoid: <negative constraints>
```

## Use-case taxonomy

Generate:

- photorealistic-natural
- product-mockup
- ui-mockup
- infographic-diagram
- logo-brand
- illustration-story
- stylized-concept
- historical-scene

Edit:

- text-localization
- identity-preserve
- precise-object-edit
- lighting-weather
- background-extraction
- style-transfer
- compositing
- sketch-to-render

## Examples

Generation example:

```text
Use case: product-mockup
Asset type: landing page hero
Primary request: a minimal hero image of a ceramic coffee mug
Style/medium: clean product photography
Composition/framing: wide composition with usable negative space for page copy
Lighting/mood: soft studio lighting
Constraints: no logos, no text, no watermark
```

Edit example:

```text
Use case: precise-object-edit
Asset type: product photo background replacement
Primary request: replace only the background with a warm sunset gradient
Constraints: change only the background; keep the product and its edges unchanged; no text; no watermark
```
