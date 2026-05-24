---
name: tao-ci
description: |
  Draft tailored professor outreach emails（套磁信）for Yizhou Pan.
  Use when the user names `$tao-ci`, points to a professor workbook like `港三教授名单.xlsx`,
  or wants a professor-specific summer research email ready to paste.
short_description: Draft professor-specific summer research outreach emails from workbook data
routing_layer: L3
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
source: project
metadata:
  version: 1.2.0
  platforms:
  - codex
  tags:
  - tao
  - ci
trigger_hints:
  - 套磁
  - 教授套磁
  - cold email 教授
  - research outreach
  - 学术联系
  - 导师联系
  - 套磁信
  - professor inquiry

---
# Tao-ci

Use this skill to produce a direct-use outreach email for a specific professor. Default output is plain text only, in this exact order: first line the professor email address, second line the subject line, then the English email body. Add Chinese notes, a `.docx`, workbook updates, or browser mail automation only when the user asks.

## Core resources

Read these before drafting:

- `references/applicant-profile.md` for Yizhou Pan's fixed background, differentiators, and experience library.
- `references/writing-rules.md` for the email structure, fit logic, selection strategy, and self-check rubric.
- Treat these two reference files as the fixed source of truth for applicant-side research status and wording constraints. Do not backfill older workspace drafts into the skill when they conflict.

## Workflow

1. Identify the target professor.
   - If the user attaches a workbook such as `港三教授名单.xlsx`, inspect it with the spreadsheet tooling available in the current environment.
   - If the workbook path is omitted but there is exactly one obvious professor workbook in the working directory, use it and state that assumption briefly.
   - Use the best match from the workbook row. If the script reports ambiguity, or the user only gave a short surname such as `Li`, ask one short disambiguation question before drafting.
   - Pull at least these fields when available: `名字`, `邮箱`, `职称`, `研究方向`, `匹配标签`, `匹配点简述`, `主页`, `Google Scholar`, `近5年代表作`, `是否招生`, `匹配度`, `套磁信已发`, `套磁状态`, `备注`.
   - If the workbook says the professor is already contacted, teaching-only, or not recruiting, warn the user before drafting and keep the draft conservative unless the user still wants it.

2. Verify current professor context from primary sources.
   - Browse the official homepage listed in the workbook.
   - Browse Google Scholar or another primary publication source for 1-2 representative papers.
   - Prefer recent or still-central work that clearly intersects with the applicant's profile.
   - Do not rely on memory for current titles, affiliations, or research focus.
   - Verify the surname used in `Dear Professor [Last Name]`. For initials or stylized names such as `G.D. LI`, infer the normal salutation from the homepage or publication listing, then use `Professor Li`.

3. Select the applicant material.
   - Choose the most relevant 1-2 experiences from `references/applicant-profile.md`.
   - Prioritize transferability, not completeness.
   - Ensure the draft surfaces at least 2 real differentiators, usually from:
     - RMB 10B+ quantitative fund internship
     - first-author research work
     - cross-market empirical validation
     - statistics + finance training

4. Draft the email.
   - Follow the 5-paragraph structure from `references/writing-rules.md`.
   - Keep the total length at 250-350 words excluding signature.
   - Paragraph 2 must reach Level 3:
     - state what the professor's work does
     - connect it to a problem Yizhou Pan has actually faced
     - give one concrete one-sentence research proposal or fusion direction
   - Paragraph 3 must frame experience as capability that can deploy to the professor's agenda.
   - Keep tone professional and collaborative, not submissive.
   - Write like a strong human applicant, not a polished LLM sample: prefer concrete claims over ornate transitions, vary sentence lengths, and remove any sentence that sounds reusable across professors.

5. Self-check and revise before answering.
   - Re-score the draft against the bundled rubric.
   - Revise until all core dimensions are at least 4/5:
     - paper understanding
     - research proposal specificity
     - mutual fit
     - capability framing
     - information density
     - differentiators
     - tone
     - length

6. Optionally send the email or open the compose page.
   - Only do this when the user explicitly asks to send, open, or prefill the enterprise mailbox.
   - **Delegate to `sustech-mailer` skill** for all email delivery:
     - Provide the exact `to`, `subject`, body, and attachment plan.
     - Open `https://exmail.qq.com` when browser compose is requested.
     - If the user explicitly says to send now, explain that direct SMTP is retired until a Rust mailer exists and provide a paste-ready payload.
     - Preview only when the user asks to preview, review, or inspect the final payload before delivery.
     - See `sustech-mailer` SKILL.md for credential setup and delivery options.

## Output contract

Default output in plain text, not a Markdown code block:

immkpso@ust.hk
Inquiry regarding Summer Research Opportunities - Yizhou Pan

Dear Professor ...
...
Warm regards,
Yizhou Pan

Keep the response compact and paste-ready. Do not prepend analysis, fit notes, labels such as `Email:` or `Subject:`, or bullets unless the user asks.

Optional output when requested:

- Chinese paraphrase after the English email
- `.docx` export
- brief fit notes
- follow-up email draft
- workbook status update
- enterprise mailbox compose page opened and prefilled in the browser (via `sustech-mailer`)

## Guardrails

- Never invent papers, methods, results, or professor interests.
- If the professor-work fit is weak, narrow the claim instead of overstating it.
- Do not use generic praise such as "I have long admired your work."
- Do not dump the CV chronologically.
- Do not answer with an outline, checklist, or explanation when the user asked for the actual email. Deliver the finished subject and body directly.
- Do not write in a visibly AI-polished register: avoid stacked abstract nouns, symmetrical three-part lists, and generic enthusiasm phrases.
- Do not update `套磁信已发` / `套磁状态` merely because a draft was generated. Update the workbook only if the user explicitly asks to record a sent or drafted state.
- If the workbook already shows the professor as contacted, mention that fact before drafting so the user can decide whether to proceed.
- If homepage or recent publication verification is unavailable, say so briefly and draft conservatively from the workbook plus bundled references.
- Do not send or open compose unless the user explicitly asks to send, open, or prefill the email in the current turn.

## Quick command

Example browser compose:

```bash
open "https://exmail.qq.com"
```

## When to use

- The user mentions "套磁信", "tao-ci", or professor outreach emails for Yizhou Pan
- The user references a professor workbook (e.g., `港三教授名单.xlsx`)
- The user wants to draft a tailored email to a specific professor for research opportunities
- The user @-mentions a professor found in a previously loaded workbook

## Do not use

- The task is only about sending an email (not drafting) → delegate to `sustech-mailer`
- The task is general email writing or editing → use `$copywriting`, `$documentation-engineering`, or `$paper-writing` depending on intent
- The task is academic paper writing → use `$paper-writing`
- The task is generic prose naturalization unrelated to professor outreach
- The user is not Yizhou Pan or the email is not for professor outreach
