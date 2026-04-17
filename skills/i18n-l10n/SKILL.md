---
name: i18n-l10n
description: |
  Internationalization and localization overlay for web/mobile projects. Covers hardcoded
  string detection, translation key design, locale files, date/number/currency formatting,
  RTL support, pluralization rules, and i18n library integration (react-intl, next-intl,
  i18next, vue-i18n). Use when the user wants multi-language support, hardcoded string
  checks, i18n review, or l10n best practices. 适用于 '国际化' '多语言' 'i18n' 'l10n' '翻译'
  '硬编码文本检查' 'locale'.
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - i18n
    - l10n
    - internationalization
    - localization
    - translation
    - multi-language
    - overlay
risk: low
source: local
routing_layer: L3
routing_owner: overlay
routing_gate: none
session_start: n/a
---
# i18n-l10n

This skill owns internationalization and localization quality as an overlay:
detecting hardcoded strings, reviewing translation key design, and ensuring
proper locale handling across frameworks.

## When to use

- The user wants to add multi-language support to a project
- The task involves checking for hardcoded user-facing strings
- The user wants to review or improve i18n implementation
- The user asks about translation key naming, locale file structure, or pluralization
- The user says "国际化", "多语言", "i18n", "翻译", "hardcoded strings", "locale"
- Best for requests like:
  - "帮我加多语言支持"
  - "检查这个项目有没有硬编码文本"
  - "i18n 做得对不对"
  - "翻译 key 怎么组织"

## Do not use

- The task is purely content translation without engineering → use a translation service
- The task is general frontend development → use framework owner skill
- The task is backend API design → use `$api-design`
- The task is accessibility → use `$accessibility-auditor`

## Task ownership and boundaries

This skill owns:
- Hardcoded string detection and extraction
- Translation key naming conventions
- Locale file structure and organization
- Date, number, and currency formatting
- Pluralization rules (ICU MessageFormat, CLDR)
- RTL layout consideration
- i18n library integration patterns (react-intl, next-intl, i18next, vue-i18n)
- Translation workflow and missing key detection

This skill does not own:
- Actual content translation
- General frontend patterns → `$frontend-code-quality`
- Accessibility → `$accessibility-auditor`

## Core workflow

### 1. Assess current state

- Identify existing i18n library (if any)
- Scan for hardcoded user-facing strings
- Check locale file structure and completeness
- Identify formatting patterns (dates, numbers, currencies)

### 2. Check i18n quality

#### String extraction
- [ ] No hardcoded user-facing strings in components
- [ ] Translation keys are descriptive (namespace.context.element)
- [ ] Keys are organized by feature/page, not by language
- [ ] Default language has all keys defined

#### Formatting
- [ ] Dates use locale-aware formatting (not manual string building)
- [ ] Numbers and currencies use Intl.NumberFormat or equivalent
- [ ] Pluralization uses ICU MessageFormat / CLDR rules
- [ ] Units of measurement are localizable

#### Structure
- [ ] Locale files are split by feature (not one giant file)
- [ ] Missing translations are detected at build time or runtime
- [ ] Fallback locale is configured
- [ ] Language switching works without page reload

#### Layout
- [ ] RTL layout is considered (if applicable)
- [ ] Text expansion is handled (German ~30% longer than English)
- [ ] Dynamic content areas flex with translated text length

### 3. Recommend improvements

Prioritize by:
1. Hardcoded strings in production code
2. Missing translations in active locales
3. Broken formatting (dates, numbers)
4. Missing pluralization rules
5. Structural improvements (key naming, file organization)

## Output defaults

```markdown
## i18n Review Summary
- Scope: [project/module]
- Library: [react-intl / next-intl / i18next / vue-i18n / none]
- Active locales: [en, zh, ...]
- Hardcoded strings found: N

## Findings
1. [Severity] Description
   - Location: ...
   - Fix: ...

## Priority Improvements
| # | Improvement | Impact | Effort |
|---|-------------|--------|--------|

## Missing Translations
| Key | Locale | Status |
|-----|--------|--------|
```

## Hard constraints

- Do not translate content yourself unless the user explicitly asks
- Do not use string concatenation for translatable text; use ICU MessageFormat
- Do not hardcode locale-specific formatting (date format, decimal separator)
- Do not assume LTR layout; check for RTL support requirements
- Keep translation keys semantic, not the literal English text

## Trigger examples

- "Use $i18n-l10n to check this project for hardcoded strings."
- "帮我给这个项目加多语言支持。"
- "检查 i18n 实现有没有问题。"
- "翻译 key 应该怎么组织？"
