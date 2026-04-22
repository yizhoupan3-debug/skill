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
runtime_requirements:
  python:
    - certifi
  env:
    - SUSTECH_SMTP_USER
    - SUSTECH_SMTP_PASS
  files:
    - ~/.sustech-mailer-smtp.env
    - ~/.tao-ci-smtp.env
    - /Users/joe/Documents/套磁/CV_Yizhou_Pan.pdf
    - /Users/joe/Documents/套磁/Academic transcript.pdf
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

Send emails from `12312411@mail.sustech.edu.cn` via SMTP over the Tencent
Enterprise Mail relay (`smtp.exmail.qq.com:465` SSL). This skill handles
credential management, content generation, optional preview, browser compose
page, and delivery.

## Prerequisites

SMTP credentials must be configured before first use. See
`scripts/.env.example` for the template.

```bash
cp scripts/.env.example ~/.sustech-mailer-smtp.env
# then edit ~/.sustech-mailer-smtp.env and fill in the authorization code
```

To obtain an authorization code:
1. Log in to `exmail.qq.com`
2. Settings → Mail sending/receiving → Enable POP3/IMAP/SMTP
3. Settings → Account binding → Generate "client-specific password"

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
- the chosen path is browser compose rather than direct SMTP send
- the calling skill asks for a dry-run style preview

For direct SMTP send requests, do **not** force an extra preview/approval turn.

**Preview Protocol:**
1.  If SMTP credentials are available and a preview is requested, run `send_email.py --dry-run` with all planned parameters.
2.  If SMTP credentials are unavailable and the path is browser compose, format the preview directly from the resolved `to` / `subject` / `body` / attachment plan, then open `open_compose.py`.
3.  Format the output into a clean preview block:

```
━━━━━━━━━━━ EMAIL PREVIEW (DRY-RUN) ━━━━━━━━━━━
From:    Yizhou Pan <12312411@mail.sustech.edu.cn>
To:      <recipient>
CC:      <cc or omit if none>
Subject: <subject>
Mode:    <default | homework>
Attach:  <file list with ✓/✗ status from dry-run output>

<body text from dry-run output>
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Describe changes, or ask to send directly.
```

- **Recipients, Subject, Body, and Attachments MUST match the dry-run output exactly when SMTP dry-run is used.**
- In browser-compose fallback, the preview must match the exact values passed to `open_compose.py`.
- If the user says "send", "发", "确认", or similar after a preview → proceed to step 4.
- If the user requests changes → revise and show preview again.

### 4. Send or open browser compose page

Two delivery methods are available:

#### 4a. SMTP send (default)

Write the body to a temp file, then call:

```bash
python3 <skill_dir>/scripts/send_email.py \
  --to "<recipient>" \
  --subject "<subject>" \
  --body-file /tmp/sustech_mailer_body.txt \
  [--mode homework] \
  [--cc "ta@sustech.edu.cn"] \
  [--no-attachments] \
  [--extra-attachment /path/to/hw.pdf]
```

#### 4b. Browser compose page (`--open-browser`)

Open the Tencent Enterprise Mail web compose page with fields pre-filled:

```bash
# Default: open exmail.qq.com web compose page
python3 <skill_dir>/scripts/open_compose.py \
  --to "<recipient>" \
  --subject "<subject>" \
  --body-file /tmp/sustech_mailer_body.txt

# Fallback: open system default mail client via mailto:
python3 <skill_dir>/scripts/open_compose.py \
  --to "<recipient>" \
  --subject "<subject>" \
  --body-file /tmp/sustech_mailer_body.txt \
  --mailto
```

The default opens `https://exmail.qq.com/cgi-bin/compose_send` with pre-filled
recipient, subject, and body. Use `--mailto` to open the system mail client instead.

Use browser compose when:
- The user explicitly asks to "open in browser" / "打开邮箱" / "跳转确认"
- SMTP credentials are not configured
- The user wants to review visually before sending

Direct-send rule:

- If the user explicitly asks to send now, or an upstream skill delegates a send-now action with resolved `to` / `subject` / `body`, send immediately through SMTP when credentials are available.
- Do not require a second manual approval round once the send intent is already explicit.

### 5. Report result

After sending:
- Confirm success with the exact message: `✓ Email sent to <recipient>`
- If failed, show the error and suggest remediation (e.g. check authorization code)

After opening browser:
- Confirm with: `✓ Compose page opened — review and send in browser`

## Script reference

See [references/script_examples.md](references/script_examples.md) for full execution examples of:
- `scripts/send_email.py` (default, homework mode, proxy, body-file)
- `scripts/open_compose.py` (web compose page, mailto client)

### Credentials

| Env var | Description |
|---------|-------------|
| `SUSTECH_SMTP_USER` | Sender email (default: `12312411@mail.sustech.edu.cn`) |
| `SUSTECH_SMTP_PASS` | Authorization code from Tencent Enterprise Mail |

Legacy env vars `TAOXI_SMTP_USER` / `TAOXI_SMTP_PASS` are also supported for compatibility.

Credentials file: `~/.sustech-mailer-smtp.env` (see `scripts/.env.example`).
Legacy path `~/.tao-ci-smtp.env` is also checked as a compatibility fallback.

## Guardrails

- Never send unless recipient, subject, and body are fully resolved.
- Never invent send intent: direct send requires either the user's explicit send request in the current turn or an upstream delegated send-now action.
- Never hardcode passwords in scripts or skill files.
- If credentials are missing, print a clear setup guide and exit — do not prompt for password inline.
- If SMTP connection fails, suggest checking network, authorization code validity, or whether SMTP service is enabled.
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
