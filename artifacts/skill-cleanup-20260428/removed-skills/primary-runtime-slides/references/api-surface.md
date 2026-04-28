# slides api surface

High-value `@oai/artifact-tool` calls used most often in this skill:

## Startup

```ts
const {
  FileBlob,
  Presentation,
  PresentationFile,
} = await import("@oai/artifact-tool");
```

Create a deck:

```ts
const presentation = Presentation.create({
  slideSize: { width: 1280, height: 720 },
});
```

Import an existing deck:

```ts
const pptx = await FileBlob.load("input.pptx");
const presentation = await PresentationFile.importPptx(pptx);
```

Export:

```ts
const output = await PresentationFile.exportPptx(presentation);
await output.save("output.pptx");
```

## Core authoring patterns

- `presentation.slides.add()`
- `slide.shapes.add({ ... })`
- `slide.tables.add(...)`
- `slide.charts.add(...)`
- `slide.images.add({ blob, fit: "cover" | "contain" })`
- `presentation.export({ slide, format: "png", scale: 1 })`

## Important rules

- Prefer native chart objects for data charts.
- Keep real slide words in editable text objects.
- Match chart typography to the deck typography.
- Use previews to catch clipping, overlap, or unreadable labels before export.
