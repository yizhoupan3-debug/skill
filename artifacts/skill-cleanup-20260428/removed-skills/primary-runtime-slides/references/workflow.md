# slides workflow

Use this condensed loop when operating the `slides` artifact gate:

1. Lock the artifact and output contract.
   - New deck vs import existing deck
   - Final deliverable path
   - Required aspect ratio and audience bar

2. Build source-first.
   - Author in one writable local JS builder
   - Keep real text, tables, and charts editable
   - Use image generation only for text-free art plates

3. Verify before export.
   - Render slide previews
   - Check critical copy visibility, editable text, chart readability, and gross overlap
   - Repair once with targeted patches instead of starting over

4. Export and stop.
   - Export one final `.pptx`
   - Do not iterate endlessly on polish after the deck is correct and readable

Keep the main user-facing answer implementation-light: summarize the visible result and link only the final `.pptx`.
