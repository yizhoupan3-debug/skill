# Conventional Commits Reference

## Format

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

## Types

| Type | Purpose | Version bump |
|------|---------|-------------|
| `feat` | New feature | MINOR |
| `fix` | Bug fix | PATCH |
| `docs` | Documentation only | — |
| `style` | Formatting, no code change | — |
| `refactor` | Code change, no feature/fix | — |
| `perf` | Performance improvement | PATCH |
| `test` | Adding/correcting tests | — |
| `build` | Build system, dependencies | — |
| `ci` | CI configuration | — |
| `chore` | Other changes | — |
| `revert` | Revert a previous commit | varies |

## Breaking Changes

Option 1 — in footer:
```
feat: allow provided config object to extend other configs

BREAKING CHANGE: `extends` key in config file is now used for extending other configs.
```

Option 2 — with `!`:
```
feat!: remove deprecated login endpoint
```

## Scope Examples

```
feat(auth): add OAuth2 support
fix(api): correct pagination offset
docs(readme): update installation steps
```

## Enforcement Tooling

### commitlint

```bash
npm install --save-dev @commitlint/cli @commitlint/config-conventional
```

`commitlint.config.js`:
```js
module.exports = { extends: ['@commitlint/config-conventional'] };
```

### Husky (Git hooks)

```bash
npm install --save-dev husky
npx husky init
echo "npx --no -- commitlint --edit \$1" > .husky/commit-msg
```

### commitizen (Interactive commit helper)

```bash
npm install --save-dev commitizen cz-conventional-changelog
```

`package.json`:
```json
{
  "config": {
    "commitizen": {
      "path": "cz-conventional-changelog"
    }
  }
}
```

Use: `npx cz` instead of `git commit`

## Validation in CI

```yaml
# GitHub Actions
- name: Validate PR title
  uses: amannn/action-semantic-pull-request@v5
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```
