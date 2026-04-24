# Vercel React / Next.js Best Practices

Use this reference inside `nextjs` for rendering, hydration, and Vercel-aligned
application quality.

Rules:

- Prefer Server Components by default; add client boundaries only for real interactivity.
- Avoid client-side data waterfalls; colocate server data fetching with the server component that needs it.
- Model expected Server Action errors as return values.
- Use explicit caching and revalidation; do not rely on version-specific defaults.
- Keep Suspense/loading boundaries close to slow data.
- Validate with `next build` and targeted runtime checks.
