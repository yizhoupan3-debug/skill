# Writing Rules

Use these rules to draft a professor-specific outreach email that is short, technical, and credible.

## Default deliverable

Unless the user asks for another format, return:

- first line: professor email address from the workbook
- second line: subject line only
- one English email body that is ready to paste

Return plain text rather than a Markdown code block. Do not automatically return a `.docx`, Chinese translation, or analysis notes.

## Structure

Keep the email to 5 paragraphs and 250-350 words excluding signature.

### Paragraph 1: introduction and purpose

Use a direct opening. Include:

- Yizhou Pan
- SUSTech
- Statistics major, Finance minor
- junior
- GPA 3.86/4.0
- purpose: inquire about a summer research internship from June to October 2026

Avoid honorific flattery.
Use the correct surname in `Dear Professor [Last Name]`. For stylized names or initials, verify the conventional surname from the homepage or publication listing.
If the email uses a first-author paper signal, phrase it as a short research-maturity note such as `one study now under submission to Advanced Engineering Informatics (AEI)`. Keep it inside the opening paragraph, and do not turn the venue into the main point of the email.

### Paragraph 2: why this professor

This is the deciding paragraph. It must reach Level 3.

Level 1:

- only mentions a paper title or broad topic

Level 2:

- explains a method or contribution

Level 3:

- explains what the professor's method solves
- connects it to a real issue from Yizhou Pan's own work
- proposes one concrete fusion direction in one sentence

Use 3-4 sentences. The ideal logic is:

1. Mention a representative paper or theme and the technical problem it solves.
2. Link that problem to an issue Yizhou Pan encountered in a relevant project.
3. Add a specific research proposal showing how the professor's method and Yizhou Pan's prior work could combine.

Bad:

- "I am very interested in your research."
- "Your work is impressive and inspiring."

Good:

- "I believe [professor method] could regularize or identify a structure that my [experience] currently explores adaptively, making the search space both more interpretable and more predictive."

### Paragraph 3: relevant experience

Use the capability-to-deployment frame.

Write:

- what problem Yizhou Pan solved
- what capability that built
- how that capability could deploy to the professor's agenda

Do not list tasks. Use 1-2 concrete outcomes, such as:

- resolved tensor OOM for minute-frequency RL training
- achieved cross-market out-of-sample IC consistency
- modeled cross-asset volatility linkages

Close the paragraph by explicitly looping back to the professor's research direction.

### Paragraph 4: logistics

State:

- full-time availability in summer 2026
- self-funded
- CV and transcript attached

### Paragraph 5: close

Thank the professor briefly and invite a discussion about contribution.

Use `Warm regards` or `Best regards`.

## Tone and style

- professional, concise, technically literate
- confident but not arrogant
- collaborative rather than pleading
- every sentence should carry new information
- slightly uneven in a natural way: mix shorter and longer sentences instead of making every line equally polished
- concrete over ornamental: mention actual methods, data settings, or outcomes instead of abstract praise

## Lower-AIGC heuristics

Apply these before finalizing:

- Cut stock transitions like `I was particularly interested in`, `I was especially drawn to`, `I believe this would be highly valuable`, unless they carry specific information.
- Avoid neat rhetorical symmetry such as `not only ... but also ...` and repeated three-item lists unless they are necessary.
- Prefer one precise technical observation to two generic compliments.
- Keep 1-2 mild human rough edges in rhythm; do not over-smooth every sentence into the same cadence.
- Avoid overusing evaluative adjectives such as `exciting`, `impressive`, `fascinating`, `innovative`, `remarkable`.
- If a sentence could appear in emails to three different professors with only the name changed, rewrite it.
- Use direct verbs like `built`, `adapted`, `tested`, `modeled`, `found`, `ran`, `resolved` more often than abstract phrases like `gained exposure to` or `developed a strong interest in`.

Avoid:

- generic praise
- repeated `I + verb` sentence openings across several consecutive sentences
- any sentence that could be copied unchanged to another professor
- more than 2 main experiences in one draft
- prefacing the email with commentary such as `Here is your draft`
- over-optimized smoothness that makes the email sound machine-polished

## Match strategy

Map professor direction to applicant material:

- RL / ML in finance: A first, B second
- Financial econometrics / time series: B plus D
- High-dimensional statistics: A, then B
- Bayesian / UQ: C, then A
- Network / graph models: B, then A

## Subject line

Use:

`Inquiry regarding Summer Research Opportunities - Yizhou Pan`

Use `Yizhou Pan` as the English rendering of `潘逸舟`.

## Self-check rubric

Revise until all items are at least 4/5:

- paper understanding: method and problem are accurately described
- research proposal: there is one concrete fusion direction
- mutual fit: the professor can see why this student is relevant
- experience framing: capability and deployment, not task inventory
- information density: no filler sentence
- differentiators: at least 2 true advantages surface naturally
- tone: professional and peer-like
- low-AIGC feel: sounds like a capable student writing carefully rather than a generic assistant
- length: 250-350 words, ideally 280-320

## Follow-up

Only generate a follow-up if the user asks. Keep it shorter and lighter than the initial email, and never recommend more than one follow-up.
