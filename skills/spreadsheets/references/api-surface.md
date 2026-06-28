# spreadsheets api surface

High-value `@oai/artifact-tool` workbook operations used most often in this skill.

## Startup

Import existing workbook:

```js
import { FileBlob, SpreadsheetFile } from "@oai/artifact-tool";

const input = await FileBlob.load("input.xlsx");
const workbook = await SpreadsheetFile.importXlsx(input);
```

Create new workbook:

```js
import { SpreadsheetFile, Workbook } from "@oai/artifact-tool";

const workbook = Workbook.create();
const sheet = workbook.worksheets.add("Inputs");
```

Export:

```js
const output = await SpreadsheetFile.exportXlsx(workbook);
await output.save("output.xlsx");
```

## High-value patterns

- Prefer block writes for values/formulas instead of per-cell loops.
- Build non-formula structure before cross-sheet formulas.
- Use inspection to confirm critical ranges:

```js
const check = await workbook.inspect({
  kind: "table",
  range: "Dashboard!A1:H20",
  include: "values,formulas",
});
```

- Use match scans for formula errors before export:

```js
const errors = await workbook.inspect({
  kind: "match",
  searchTerm: "#REF!|#DIV/0!|#VALUE!|#NAME\\?|#N/A",
  options: { useRegex: true, maxResults: 300 },
});
```

## Design rules

- Prefer formulas over hardcoded derived values.
- Use charts for summary-analysis prompts where visual synthesis matters.
- Keep workbook logic auditable and editable.
