# Branching Strategies Comparison

## Quick Decision Tree

```
Team size / release cadence?
├─ Solo or small team, continuous deploy → Trunk-Based
├─ Small-medium team, PR-based → GitHub Flow
├─ Large team, scheduled releases → Gitflow
└─ Open source with maintained versions → Gitflow variant
```

---

## Trunk-Based Development

**How it works:** Everyone commits to `main` (or very short-lived branches).

```
main ─────●────●────●────●────●──── (always deployable)
            \──●──/   (feature branch: < 1 day)
```

**Rules:**
- Feature branches live < 1-2 days
- Use feature flags for incomplete work
- CI/CD must be fast and reliable
- No long-lived branches

**Best for:** High-trust teams, continuous delivery, microservices.

**Not suited for:** Teams without CI automation, regulated release cycles.

---

## GitHub Flow

**How it works:** `main` is always deployable; features go through PRs.

```
main ─────●─────────────●─────────●──── (deployable)
            \──●──●──●──/ (PR)
                  \──●──●──/ (PR)
```

**Rules:**
- Branch from `main` for every feature/fix
- Open a PR for review
- Merge to `main` after approval + CI pass
- Deploy from `main`

**Best for:** Small-to-medium teams, web apps, SaaS with continuous deploy.

**Not suited for:** Products needing multiple supported versions.

---

## Gitflow

**How it works:** Structured branches for features, releases, and hotfixes.

```
main ──────────────●──────────────●──── (tagged releases)
                  / \            / \
develop ──●──●──●────●──●──●──●────●── (integration)
            \──●──/     (feature)
                \──●──/ (release/1.0)
                              \──●──/ (hotfix)
```

**Branches:**
- `main` — production releases only (tagged)
- `develop` — integration branch
- `feature/*` — branch from develop, merge back to develop
- `release/*` — stabilization before release
- `hotfix/*` — emergency fix from main, merge to main + develop

**Best for:** Versioned software, mobile apps, enterprise products.

**Not suited for:** Teams wanting continuous delivery without release ceremonies.

---

## Recommendation Matrix

| Criterion | Trunk-Based | GitHub Flow | Gitflow |
|-----------|:-----------:|:-----------:|:-------:|
| Team size < 5 | ✅ | ✅ | ⚠️ overhead |
| Team size 5-15 | ✅ | ✅ | ✅ |
| Team size > 15 | ✅ (with flags) | ⚠️ | ✅ |
| Continuous deploy | ✅ | ✅ | ❌ |
| Scheduled releases | ⚠️ | ⚠️ | ✅ |
| Multiple prod versions | ❌ | ❌ | ✅ |
| Junior-heavy team | ⚠️ risk | ✅ PR safety net | ✅ guardrails |
| CI/CD maturity needed | High | Medium | Low |
