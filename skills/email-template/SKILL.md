---
name: email-template
description: |
  Produce cross-client HTML emails that render correctly in Outlook, Gmail, and Apple
  Mail. Delivers table-based layouts with inline CSS, Outlook conditional comments,
  responsive fluid-hybrid patterns, and dark-mode support. Use when the user asks about
  email templates, HTML email compatibility, newsletter markup, or phrases like '邮件模板',
  'HTML 邮件', 'Outlook 兼容', 'MJML', 'React Email', 'email 排版'.
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - email
    - html-email
    - mjml
    - react-email
    - newsletter
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
user-invocable: false
disable-model-invocation: true
trigger_hints:
  - 邮件模板
  - HTML 邮件
  - Outlook 兼容
  - MJML
  - React Email
  - email 排版
  - HTML email compatibility
  - newsletter markup
  - email
  - html email

---

# email-template

This skill owns HTML email development: an environment with unique rendering
constraints that differ significantly from modern web browsers.

## When to use

- Building or debugging HTML email templates
- Ensuring cross-client compatibility (Gmail, Outlook, Apple Mail, Yahoo)
- Converting web designs to email-safe markup
- Using email frameworks (MJML, React Email, Maizzle)
- Best for requests like:
  - "帮我写一个 HTML 邮件模板"
  - "这个邮件在 Outlook 里显示不对"
  - "用 MJML 做一个 newsletter"
  - "Build a responsive transactional email"

## Do not use

- The task is regular web page development → use framework or `$web-platform-basics`
- The task is email delivery infrastructure (SMTP, SPF/DKIM/DMARC) → broader ops scope
- The task is email content/copywriting without template concerns
- The task is CSS engineering for web pages → use `$css-pro`

## Task ownership and boundaries

This skill owns:
- table-based email layout patterns
- inline CSS and CSS support across email clients
- Outlook conditional comments (`<!--[if mso]>`)
- responsive email techniques (fluid hybrid, media queries with fallbacks)
- dark mode email support (`@media (prefers-color-scheme: dark)`, `[data-ogsc]`)
- email framework usage (MJML, React Email, Maizzle)
- image handling (alt text, retina, hosted vs embedded)
- testing and preview workflows (Litmus, Email on Acid, local preview)

This skill does not own:
- email delivery and authentication (SPF, DKIM, DMARC, SMTP configuration)
- email marketing strategy and analytics
- general web page development
- server-side email sending logic

If the task shifts to adjacent skill territory, route to:
- `$web-platform-basics` for general HTML/CSS questions
- `$react` if using React Email and the issue is React-specific
- `$node-backend` for server-side email sending implementation

## Core workflow

### 1. Understand the Environment

Email HTML is **not** modern web HTML:
- No flexbox, no grid, no CSS variables in most clients
- Outlook uses Word as rendering engine → table-based layout is mandatory
- Gmail strips `<style>` in some contexts → inline CSS is safest
- Images may be blocked by default → always use descriptive alt text

### 2. Layout Patterns

#### Basic Table Structure
```html
<table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0">
  <tr>
    <td align="center">
      <table role="presentation" width="600" cellpadding="0" cellspacing="0" border="0">
        <!-- content rows -->
      </tr>
    </td>
  </tr>
</table>
```

#### Responsive Pattern (Fluid Hybrid)
```html
<!--[if mso]>
<table role="presentation" width="600"><tr><td>
<![endif]-->
<div style="max-width: 600px; margin: 0 auto;">
  <!-- content -->
</div>
<!--[if mso]></td></tr></table><![endif]-->
```

### 3. Outlook Conditional Comments

```html
<!--[if mso]>    <!-- Outlook only -->
<!--[if !mso]><!--> <!-- Not Outlook --><!--<![endif]-->
<!--[if gte mso 9]>  <!-- Outlook 2007+ -->
```

### 4. Dark Mode Support

```css
@media (prefers-color-scheme: dark) {
  .email-body { background-color: #1a1a1a !important; }
  .email-text { color: #ffffff !important; }
}
/* Outlook.com dark mode */
[data-ogsc] .email-text { color: #ffffff !important; }
```

### 5. Email Frameworks

#### MJML
```html
<mjml>
  <mj-body>
    <mj-section>
      <mj-column>
        <mj-text>Hello World</mj-text>
        <mj-button href="https://example.com">Click Me</mj-button>
      </mj-column>
    </mj-section>
  </mj-body>
</mjml>
```
Compile: `npx mjml input.mjml -o output.html`

#### React Email
```tsx
import { Html, Head, Body, Container, Text, Button } from '@react-email/components';

export default function WelcomeEmail() {
  return (
    <Html>
      <Head />
      <Body style={{ backgroundColor: '#f6f6f6' }}>
        <Container>
          <Text>Hello World</Text>
          <Button href="https://example.com">Click Me</Button>
        </Container>
      </Body>
    </Html>
  );
}
```

### 6. Testing Checklist

- [ ] Gmail (web, iOS, Android) — strips `<style>` blocks in some views
- [ ] Outlook 365 / 2019 / 2016 (Windows) — Word rendering engine
- [ ] Apple Mail (macOS, iOS) — best CSS support
- [ ] Yahoo Mail — limited media query support
- [ ] Dark mode — all major clients
- [ ] Image blocking — alt text and background colors visible
- [ ] Link tracking — UTM parameters, preview text

## Hard constraints

- Always use `role="presentation"` on layout tables.
- Always use inline CSS as the primary styling method; `<style>` blocks as progressive enhancement.
- Always include descriptive alt text on images.
- Never rely on CSS Grid, Flexbox, or CSS Variables for core layout.
- Always test with Outlook-specific conditional comments for Outlook-specific fixes.
- Keep total email size under 102 KB (Gmail clips emails larger than this).
- Use web-safe fonts with fallback stacks; custom fonts are progressive enhancement only.

## Trigger examples

- "Use $email-template to build a responsive transactional email."
- "帮我写一个兼容 Outlook 的 HTML 邮件模板。"
- "用 MJML 做一个 newsletter 模板。"
- "这个邮件在 Gmail 里样式丢了怎么修？"
