---
name: sustech-mailer
description: |
  Send emails from the SUSTech student mailbox via SMTP with auto-generated content.
  Use when the user asks to send, draft, or compose an email from 12312411@mail.sustech.edu.cn, or when another skill delegates delivery. Supports subject/body generation, optional preview, browser compose, and homework-submission emails in Chinese. Best for outbound mail via the student enterprise mailbox.
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - SUSTech 邮件
  - SMTP
  - 发邮件
  - 写邮件
  - 12312411@mail.sustech.edu.cn
  - draft
  - compose an email from 12312411@mail
  - email
  - sustech
  - mailer
metadata:
  version: 2.2.0
  platforms:
  - codex
  tags:
  - email
  - smtp
  - sustech
  - mailer
  - homework
---

# SUSTech Mailer

Draft emails from `12312411@mail.sustech.edu.cn` and prepare paste-ready Tencent
Enterprise Mail payloads. Direct SMTP delivery is disabled until a Rust mailer
is available.

## Prerequisites

For browser delivery, log in to `https://exmail.qq.com` first. Attachments must be added manually in the browser.

## Modes

### Default mode (English, professional)

For professor outreach, general inquiries, follow-ups, etc. Auto-generates
English subject and body from conversation context.

### Homework submission mode (`--mode homework`)

For simple assignment/project submissions. Generates an **English** email with:
- Course name and assignment/project number
- Attached homework file(s)
- Brief, polite closing

Example output:

```
Dear [Recipient],

Please find attached my submission for [Course Name] Assignment/Project [X].

If you have any questions, please feel free to contact me.

Thank you!

Best regards,
Yizhou Pan
```

> [!NOTE]
> `[Course Name]` and `[X]` are placeholders — the agent **must** fill them from user context (e.g. "Data Science A" "HW3"). Never send emails with unfilled placeholders.

## Workflow

### 1. Gather email parameters

Collect from the calling context or user input:

| Field | Source | Required |
|-------|--------|----------|
| **To** | user provides, or passed by an upstream automation/workbook row | ✓ |
| **Subject** | auto-generated if not provided (see §2) | ✓ |
| **Body** | auto-generated if not provided (see §2) | ✓ |
| **Mode** | `default` or `homework`; auto-inferred from context (see below) | optional |
| **Attachments** | mode-dependent defaults; override with flags | optional |
| **Course** | course name, required in homework mode | homework only |
| **Assignment** | assignment/project identifier (e.g. "HW3", "proj1") | homework only |
| **CC** | optional CC recipient(s) | optional |

**Mode-Specific Default Attachments:**
- **`default` mode:** Automatically attaches `CV_Yizhou_Pan.pdf` and `Academic transcript.pdf`.
- **`homework` mode:** No default attachments.
- To send an email in `default` mode *without* these attachments, you **MUST** use the `--no-attachments` flag.

### 2. Auto-generate subject and body (when not provided)

When called without explicit subject/body, generate them from context:

**Default mode** subject rules:
- For professor outreach: `Inquiry regarding [Research Area] Opportunities - Yizhou Pan`
- For follow-up: `Re: [Original Subject] - Yizhou Pan`
- For general: `[Concise Purpose] - Yizhou Pan`
- Always append ` - Yizhou Pan` so the recipient can identify the sender

**Default mode** body rules:
- Infer the purpose from conversation context (outreach, follow-up, inquiry, etc.)
- Use a professional but natural English tone
- Keep the body 150–400 words unless the user specifies length
- End with `Warm regards,\nYizhou Pan`
- Do not use generic LLM-polished language

**Homework mode** subject rules:
- Format: `[Course Name] Assignment/Project [X] Submission - Yizhou Pan 12312411`
- Example: `Fundamentals of Data Science A Assignment 3 Submission - Yizhou Pan 12312411`

**Homework mode** body rules:
- **Must be in English**
- Keep it simple and direct — no more than 3–4 sentences
- Include: generic greeting → what is being submitted (course + assignment number) → request to review attachment → thanks → sign-off
- Do not include flowery language or unnecessary formality
- End with name

**Mode auto-inference**: if the user mentions "交作业", "提交作业", "作业", "proj", homework file paths, or a TA/course email, automatically select homework mode. Otherwise default to professional English mode.

### 3. Optional preview

Show a preview only when one of these is true:

- the user explicitly asks to preview, review, inspect, or confirm before sending
- the chosen path is browser compose rather than direct send
- the calling skill asks for a dry-run style preview

For direct send requests, explain that direct SMTP is currently unavailable and provide a paste-ready payload instead.

**Preview Protocol:**
1. Format the preview directly from the resolved `to` / `subject` / `body` / attachment plan.
2. Ensure attachments listed in the preview are manually attachable files.
3. Format the output into a clean preview block:

```
━━━━━━━━━━━ EMAIL PREVIEW (DRY-RUN) ━━━━━━━━━━━
From:    Yizhou Pan <12312411@mail.sustech.edu.cn>
To:      <recipient>
CC:      <cc or omit if none>
Subject: <subject>
Mode:    <default | homework>
Attach:  <file list with manual attach status>

<body text>
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Describe changes, or ask to send directly.
```

- Recipients, Subject, Body, and Attachments MUST match the final paste-ready payload exactly.
- If the user says "send", "发", "确认", or similar after a preview → proceed to step 4.
- If the user requests changes → revise and show preview again.

### 4. Send or open browser compose page

Two delivery methods are available:

#### 4a. Paste-ready browser send (default)

Open Tencent Enterprise Mail and paste the final payload manually:

```bash
open "https://exmail.qq.com"
```

#### 4b. System mail client fallback

Use `mailto:` only when the user explicitly accepts that attachment handling is manual and body formatting may be client-dependent:

```bash
open "mailto:<recipient>?subject=<url-encoded-subject>&body=<url-encoded-body>"
```

Use browser compose when:
- The user explicitly asks to "open in browser" / "打开邮箱" / "跳转确认"
- Direct SMTP delivery is unavailable
- The user wants to review visually before sending

Direct-send rule:

- If the user explicitly asks to send now, provide the exact payload and open the browser when requested.
- Do not claim an email was sent unless a working non-retired delivery tool confirms it.

### 5. Report result

After opening browser:
- Confirm with: `Compose page opened — paste, review, attach files, then send in browser`

## Script reference

See [references/script_examples.md](references/script_examples.md) for browser and paste-ready examples.

### Credentials

| Env var | Description |
|---------|-------------|
No local credentials are used by this skill while direct SMTP is retired.

## Guardrails

- Never send unless recipient, subject, and body are fully resolved.
- Never invent send intent: direct send requires either the user's explicit send request in the current turn or an upstream delegated send-now action.
- Never hardcode passwords in scripts or skill files.
- If direct SMTP is requested, say it is currently unavailable pending Rust mailer support.
- Do not fabricate recipient addresses; always use the address provided by the user or calling skill.
- In homework mode, do not attach CV/transcript by default — only attach the homework file(s).
- In homework mode, **always warn** if no attachment is provided — submitting homework without a file is likely a mistake.

## When to use

- The user asks to send an email from their SUSTech student mailbox
- Another automation or upstream workflow delegates the email-sending step
- The user says "发邮件", "send email", "用学生邮箱发", "交作业", "提交作业", or similar
- The user wants to compose and preview an email before sending
- The user wants to open the browser compose page for review

## Do not use

- The user is composing an email that does not need to be sent from the SUSTech mailbox
- The task is drafting email content only without sending → just write the text directly
- The user wants to use a different email account or provider
