# Next.js Caching Matrix

## Data Caching Strategies

| Layer | Mechanism | Scope | Revalidation | Default |
|-------|-----------|-------|-------------|---------|
| **Request Memoization** | `React.cache` / dedupe `fetch` | Single render pass | Automatic (per request) | ON |
| **Data Cache** | `fetch` cache | Cross-request, cross-user | `revalidate` option | ON (GET only) |
| **Full Route Cache** | Pre-rendered HTML + RSC Payload | Cross-request, cross-user | `revalidate` option | Static routes only |
| **Router Cache** | Client-side RSC Payload | Single user session | Time-based / `router.refresh()` | ON |

## fetch() Caching Options

```tsx
// Default: cached indefinitely (static)
fetch(url);

// Revalidate every 60 seconds (ISR)
fetch(url, { next: { revalidate: 60 } });

// No caching (dynamic)
fetch(url, { cache: 'no-store' });

// Tag-based revalidation
fetch(url, { next: { tags: ['posts'] } });
// Then invalidate:
revalidateTag('posts');
```

## Route Segment Config

```tsx
// Force dynamic rendering
export const dynamic = 'force-dynamic';

// Force static rendering
export const dynamic = 'force-static';

// Revalidate every N seconds
export const revalidate = 60;

// Set runtime
export const runtime = 'edge'; // or 'nodejs'
```

## Revalidation Methods

| Method | Trigger | Scope |
|--------|---------|-------|
| **Time-based** | `revalidate: N` | After N seconds, next request gets fresh data |
| **On-demand (tag)** | `revalidateTag('tag')` | All fetches with matching tag |
| **On-demand (path)** | `revalidatePath('/path')` | Specific route |
| **Manual** | `router.refresh()` | Client-side Router Cache only |

## Decision Tree

```
Is the data user-specific?
├── Yes → cache: 'no-store' or cookies()/headers()
└── No → Is it time-sensitive?
    ├── Yes → next: { revalidate: N }
    └── No → Is it event-driven?
        ├── Yes → next: { tags: ['x'] } + revalidateTag
        └── No → Default static caching
```

## Common Pitfalls

| Pitfall | Fix |
|---------|-----|
| Using `cookies()` or `headers()` opts entire route to dynamic | Move to specific components |
| `searchParams` in page makes it dynamic | Use `generateStaticParams` if possible |
| Stale Router Cache after mutation | Call `router.refresh()` after Server Action |
| Over-caching personalized data | Use `cache: 'no-store'` for auth-dependent fetches |
| Missing `revalidatePath` after Server Action | Always revalidate affected paths/tags |
