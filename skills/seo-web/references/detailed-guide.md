# seo-web — Detailed Guide

> Extracted from SKILL.md to reduce token consumption at routing time.

## Task ownership and boundaries

This skill owns:
- HTML meta tags (title, description, viewport, charset)
- OpenGraph and Twitter Card meta tags
- JSON-LD structured data (Schema.org)
- sitemap.xml generation and configuration
- robots.txt configuration and meta robots directives
- canonical URLs and hreflang for internationalization
- heading hierarchy (single H1, proper nesting)
- URL structure and slug best practices
- crawlability and indexability analysis
- framework-specific SEO integration

This skill does not own:
- web performance / Core Web Vitals as primary focus → `$performance-expert`
- semantic HTML mechanics without SEO context → `$web-platform-basics`
- content strategy / copywriting
- link building or off-page SEO strategy

If the task shifts to adjacent skill territory, route to:
- `$performance-expert` for CWV optimization
- `$nextjs` for Next.js metadata API implementation
- `$web-platform-basics` for semantic HTML mechanics
- `$accessibility-auditor` for a11y concerns (overlap: heading hierarchy, semantic HTML)

## Core workflow

### 1. SEO Audit Checklist

#### Meta Tags
- [ ] `<title>` — unique, descriptive, 50–60 characters
- [ ] `<meta name="description">` — compelling, 150–160 characters
- [ ] `<meta name="viewport">` — `width=device-width, initial-scale=1`
- [ ] `<meta charset="utf-8">`
- [ ] `<link rel="canonical">` — self-referencing or pointing to canonical version
- [ ] `<meta name="robots">` — `index, follow` (or `noindex` for non-public pages)

#### OpenGraph / Social
- [ ] `og:title`, `og:description`, `og:image`, `og:url`, `og:type`
- [ ] `og:image` — recommended 1200×630px, < 8MB
- [ ] `twitter:card`, `twitter:title`, `twitter:description`, `twitter:image`
- [ ] Validate with: [Facebook Sharing Debugger](https://developers.facebook.com/tools/debug/), [Twitter Card Validator](https://cards-dev.twitter.com/validator)

#### Structured Data (JSON-LD)
```html
<script type="application/ld+json">
{
  "@context": "https://schema.org",
  "@type": "Article",
  "headline": "Article Title",
  "author": { "@type": "Person", "name": "Author Name" },
  "datePublished": "2024-01-15",
  "image": "https://example.com/image.jpg"
}
</script>
```
- Common types: Article, Product, FAQPage, BreadcrumbList, Organization, WebSite
- Validate with: [Google Rich Results Test](https://search.google.com/test/rich-results)

#### Sitemap & Robots
```xml
<!-- sitemap.xml -->
<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>https://example.com/</loc>
    <lastmod>2024-01-15</lastmod>
    <changefreq>weekly</changefreq>
    <priority>1.0</priority>
  </url>
</urlset>
```
```
# robots.txt
User-agent: *
Allow: /
Disallow: /admin/
Sitemap: https://example.com/sitemap.xml
```

#### URL Structure
- Lowercase, hyphen-separated slugs
- Meaningful paths (`/blog/seo-guide` not `/p?id=123`)
- Avoid excessive query parameters
- Trailing slash consistency

#### Heading Hierarchy
- Single `<h1>` per page
- Sequential nesting: H1 → H2 → H3 (no skipping levels)
- H1 should match or closely relate to `<title>`

### 2. Framework-Specific SEO

#### Next.js (App Router)
```typescript
// app/blog/[slug]/page.tsx
export async function generateMetadata({ params }): Promise<Metadata> {
  const post = await getPost(params.slug);
  return {
    title: post.title,
    description: post.excerpt,
    openGraph: { images: [post.coverImage] },
  };
}
```

#### Next.js Sitemap
```typescript
// app/sitemap.ts
export default async function sitemap(): Promise<MetadataSitemap> {
  const posts = await getAllPosts();
  return posts.map(post => ({
    url: `https://example.com/blog/${post.slug}`,
    lastModified: post.updatedAt,
  }));
}
```

#### Vue / Nuxt
- `@nuxtjs/seo` module or `useHead()` composable
- `nuxt.config.ts` → `seo` module configuration

#### React (SPA)
- `react-helmet-async` for dynamic meta tags
- Pre-rendering or SSR required for SEO-critical pages

## Output defaults

```markdown
## SEO Audit Summary
- Pages audited: N
- Critical: N | Major: N | Minor: N

## Findings
### [Severity] Finding Title
- **Category**: meta / structured data / sitemap / URL / heading
- **Page(s)**: affected URLs
- **Issue**: description
- **Fix**: concrete code change

## Priority Fix Plan
- ...
```

## Hard constraints

- Do not recommend keyword stuffing or manipulative SEO tactics.
- Always validate structured data against Schema.org spec.
- Do not generate misleading meta descriptions or titles.
- Always check that canonical URLs are correct to avoid duplicate content issues.
- Ensure OG images meet minimum size requirements (1200×630px recommended).
- When auditing, separate confirmed issues from best-practice suggestions.
- Prefer server-side rendering or pre-rendering for SEO-critical content.

## Trigger examples

- "Use $seo-web to audit this site's SEO and fix meta tags."
- "帮我添加 JSON-LD 结构化数据。"
- "检查这个 Next.js 项目的 SEO 配置。"
- "生成 sitemap.xml 和 robots.txt。"
