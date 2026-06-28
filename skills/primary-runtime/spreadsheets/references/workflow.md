# spreadsheets workflow

Use this condensed loop when operating the `spreadsheets` artifact gate:

1. Lock the workbook shape.
   - Template/tracker vs analytical report vs dashboard vs model
   - Final output path
   - Whether an existing workbook must be edited

2. Build in the right order.
   - Sheets and structure first
   - Inputs and source blocks next
   - Formulas after the structure is stable
   - Formatting, validation, and charts last

3. Verify compactly.
   - Inspect key ranges for values and formulas
   - Scan for common formula errors such as `#REF!`, `#DIV/0!`, `#VALUE!`, `#NAME?`, `#N/A`
   - Render important sheets once to catch clipping or unreadable layout

4. Export and finalize.
   - Export one final `.xlsx`
   - Stop once the workbook is correct, auditable, and readable

Default quality target: a workbook that looks intentional and remains editable by a normal Excel user.
