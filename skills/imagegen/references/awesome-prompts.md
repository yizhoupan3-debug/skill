# Awesome Prompts — Image Generation Reference

## Community Resources

| Resource | URL | Best for |
|----------|-----|----------|
| Awesome AI Art Image Synthesis | [GitHub](https://github.com/altryne/awesome-ai-art-image-synthesis) | DALL-E / MidJourney / StableDiffusion prompt tips |
| Awesome Prompt Engineering | [GitHub](https://github.com/promptslab/Awesome-Prompt-Engineering) | Academic survey of Text-to-Image techniques |
| DAIR.AI Prompt Engineering Guide | [GitHub](https://github.com/dair-ai/Prompt-Engineering-Guide) | General prompt engineering living knowledge base |
| Gemini Image Prompt Library | [GitHub](https://github.com/YouMind-OpenLab/awesome-nano-banana-pro-prompts) | Curated prompts with preview images for Google Gemini |

## Key Prompt Principles

### Structure (scene → subject → details → constraints)

```
A [style/medium] of [subject] in [scene/environment],
[composition/framing], [lighting/mood],
[color palette], [quality level].
Avoid: [negative constraints].
```

### Text Rendering

- Quote exact text: `"Hello World"`
- Specify font style and placement
- For tricky words, spell letter-by-letter
- Use `quality=high` for text-heavy outputs

### Edit Invariants

When editing, always repeat what must NOT change:

```
Change only [X]. Keep [Y] unchanged.
Do not alter [Z].
```

### Anti-Tacky Checklist

If results feel "stock photo" or "cheesy", add:

```
Avoid: stock-photo vibe, cheesy lens flare, oversaturated neon,
harsh bloom, oversharpening, clutter, generic gradient backgrounds.
Style: editorial, premium, subtle, restrained.
```

### Model Selection Quick Guide

| Scenario | Model | Quality |
|----------|-------|---------|
| Default (most tasks) | `gpt-image-1.5` | auto |
| Text-heavy | `gpt-image-1.5` | high |
| Quick iteration | `gpt-image-1-mini` | low |
| Detail-critical final | `gpt-image-1.5` | high |
| Strict edits (identity lock) | `gpt-image-1.5` | high + input_fidelity=high |
