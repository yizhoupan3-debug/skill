# semantic-release Setup Guide

## Quick Start

```bash
npm install --save-dev semantic-release @semantic-release/changelog @semantic-release/git
```

## Configuration (`.releaserc.json`)

```json
{
  "branches": ["main"],
  "plugins": [
    "@semantic-release/commit-analyzer",
    "@semantic-release/release-notes-generator",
    ["@semantic-release/changelog", { "changelogFile": "CHANGELOG.md" }],
    ["@semantic-release/npm", { "npmPublish": true }],
    ["@semantic-release/git", {
      "assets": ["CHANGELOG.md", "package.json"],
      "message": "chore(release): ${nextRelease.version} [skip ci]"
    }],
    "@semantic-release/github"
  ]
}
```

## GitHub Actions Integration

```yaml
name: Release
on:
  push:
    branches: [main]
permissions:
  contents: write
  issues: write
  pull-requests: write
  id-token: write
jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - run: npm ci
      - run: npx semantic-release
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          NPM_TOKEN: ${{ secrets.NPM_TOKEN }}
```

## Commit Types → Version Bumps

| Commit type | Version bump |
|-------------|-------------|
| `fix:` | PATCH |
| `feat:` | MINOR |
| `feat!:` or `BREAKING CHANGE:` | MAJOR |
| `chore:`, `docs:`, `style:` | No release |

## Monorepo Setup

Use `semantic-release-monorepo` or `multi-semantic-release`:

```bash
npm install --save-dev semantic-release-monorepo
```

Per-package `.releaserc`:
```json
{
  "extends": "semantic-release-monorepo"
}
```

## Dry Run

```bash
npx semantic-release --dry-run
```
