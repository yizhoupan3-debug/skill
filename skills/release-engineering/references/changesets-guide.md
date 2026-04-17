# Changesets Monorepo Release Guide

## Quick Start

```bash
npm install --save-dev @changesets/cli
npx changeset init
```

## Workflow

### 1. Developer creates a changeset

```bash
npx changeset
# Interactive prompt: select packages, bump type, summary
```

This creates `.changeset/<unique-id>.md`:
```markdown
---
"@my-org/package-a": minor
"@my-org/package-b": patch
---

Added new feature X to package-a, fixed bug Y in package-b.
```

### 2. CI generates version bumps and changelogs

```bash
npx changeset version
```

- Consumes all pending changesets
- Bumps `package.json` versions
- Updates per-package `CHANGELOG.md`

### 3. CI publishes

```bash
npx changeset publish
```

## GitHub Actions with Changesets Bot

```yaml
name: Release
on:
  push:
    branches: [main]
jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - run: npm ci
      - name: Create Release PR or Publish
        uses: changesets/action@v1
        with:
          publish: npx changeset publish
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          NPM_TOKEN: ${{ secrets.NPM_TOKEN }}
```

## Key Differences from semantic-release

| Aspect | Changesets | semantic-release |
|--------|-----------|------------------|
| Changelog curation | Human-written summaries | Auto-generated from commits |
| Monorepo support | First-class | Requires plugins |
| Release trigger | Merge "Version Packages" PR | Push to main |
| Commit convention | Not required | Required (conventional commits) |
| Control level | More manual, more control | More automated |
