# GitHub Actions High-Frequency Patterns

## Reusable Workflows

### Caller workflow

```yaml
name: CI
on: push
jobs:
  test:
    uses: ./.github/workflows/reusable-test.yml
    with:
      node-version: 20
    secrets: inherit  # or explicit secret passing
```

### Reusable workflow definition

```yaml
# .github/workflows/reusable-test.yml
name: Reusable Test
on:
  workflow_call:
    inputs:
      node-version:
        type: number
        default: 20
    secrets:
      NPM_TOKEN:
        required: false

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: ${{ inputs.node-version }}
      - run: npm ci
      - run: npm test
```

**Key rules:**
- Reusable workflows must be in `.github/workflows/`
- Use `workflow_call` trigger
- Max 4 levels of nesting
- Caller can pass `secrets: inherit` or name them explicitly

---

## Composite Actions

```yaml
# .github/actions/setup-project/action.yml
name: Setup Project
description: Install deps and build
inputs:
  node-version:
    default: '20'
runs:
  using: composite
  steps:
    - uses: actions/setup-node@v4
      with:
        node-version: ${{ inputs.node-version }}
        cache: npm
    - run: npm ci
      shell: bash
    - run: npm run build
      shell: bash
```

Usage:
```yaml
- uses: ./.github/actions/setup-project
  with:
    node-version: 20
```

**Key rules:**
- Must specify `shell:` for every `run:` step
- Can live anywhere in repo (not just `.github/workflows/`)
- Great for DRY-ing up repeated setup steps

---

## OIDC Authentication (Keyless Auth)

### AWS

```yaml
permissions:
  id-token: write
  contents: read

steps:
  - uses: aws-actions/configure-aws-credentials@v4
    with:
      role-to-assume: arn:aws:iam::123456789:role/github-actions
      aws-region: us-east-1
```

### GCP

```yaml
permissions:
  id-token: write
  contents: read

steps:
  - uses: google-github-actions/auth@v2
    with:
      workload_identity_provider: projects/123/locations/global/workloadIdentityPools/...
      service_account: github-actions@project.iam.gserviceaccount.com
```

**Key rules:**
- Always prefer OIDC over long-lived secrets
- Requires `id-token: write` permission
- Configure trust relationship on the cloud provider side

---

## Environment Protection Rules

```yaml
jobs:
  deploy-staging:
    runs-on: ubuntu-latest
    environment: staging
    steps:
      - run: echo "deploying to staging"

  deploy-prod:
    needs: deploy-staging
    runs-on: ubuntu-latest
    environment:
      name: production
      url: https://example.com
    steps:
      - run: echo "deploying to production"
```

Configure in repo Settings → Environments:
- Required reviewers
- Wait timer
- Branch restrictions
- Environment secrets

---

## Cache Strategies

### npm/pnpm cache

```yaml
- uses: actions/setup-node@v4
  with:
    node-version: 20
    cache: npm  # or 'pnpm' or 'yarn'
```

### Custom cache

```yaml
- uses: actions/cache@v4
  with:
    path: |
      ~/.cache/pip
      node_modules/.cache
    key: ${{ runner.os }}-deps-${{ hashFiles('**/package-lock.json') }}
    restore-keys: |
      ${{ runner.os }}-deps-
```

### Build cache (Turborepo, Nx)

```yaml
- uses: actions/cache@v4
  with:
    path: node_modules/.cache/turbo
    key: turbo-${{ github.sha }}
    restore-keys: turbo-
```

**Key rules:**
- Always use `hashFiles()` for cache keys
- Provide `restore-keys` for partial matching
- Cache immutable deps (lockfile-based), not build output

---

## Matrix Builds

### Basic matrix

```yaml
strategy:
  matrix:
    node-version: [18, 20, 22]
    os: [ubuntu-latest, macos-latest]
  fail-fast: false  # continue other jobs if one fails
```

### Include/exclude

```yaml
strategy:
  matrix:
    os: [ubuntu-latest, windows-latest]
    node: [18, 20]
    include:
      - os: ubuntu-latest
        node: 22
    exclude:
      - os: windows-latest
        node: 18
```

### Dynamic matrix

```yaml
jobs:
  detect:
    outputs:
      packages: ${{ steps.find.outputs.packages }}
    steps:
      - id: find
        run: echo "packages=$(ls packages | jq -R -s -c 'split("\n")[:-1]')" >> $GITHUB_OUTPUT

  test:
    needs: detect
    strategy:
      matrix:
        package: ${{ fromJSON(needs.detect.outputs.packages) }}
```

---

## Concurrency Control

```yaml
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true  # cancel previous runs on same branch
```

For deploy workflows where you DON'T want cancellation:
```yaml
concurrency:
  group: deploy-${{ github.ref }}
  cancel-in-progress: false
```

---

## Self-Hosted Runners

```yaml
jobs:
  build:
    runs-on: [self-hosted, linux, x64]
    steps:
      - uses: actions/checkout@v4
      - run: make build
```

**Key rules:**
- Use labels to target specific runner capabilities
- Self-hosted runners persist state between jobs — clean up explicitly
- Consider security: self-hosted runners execute any workflow code

---

## workflow_dispatch (Manual Triggers)

```yaml
on:
  workflow_dispatch:
    inputs:
      environment:
        description: Target environment
        required: true
        type: choice
        options: [staging, production]
      dry-run:
        description: Dry run mode
        type: boolean
        default: true

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - run: echo "Deploying to ${{ inputs.environment }}, dry-run=${{ inputs.dry-run }}"
```

---

## Artifact Handoff Between Jobs

```yaml
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - run: npm run build
      - uses: actions/upload-artifact@v4
        with:
          name: build-output
          path: dist/

  deploy:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/download-artifact@v4
        with:
          name: build-output
          path: dist/
      - run: ./deploy.sh dist/
```
