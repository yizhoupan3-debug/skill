---
name: seo-web
description: |
  Audit and optimize technical SEO for web apps: meta tags, structured data
  (JSON-LD/OpenGraph/Twitter Cards), sitemap, robots.txt, canonical URLs, hreflang,
  headings, semantic HTML, crawlability, and framework SEO (Next.js metadata API, Nuxt SEO
  module). Use when the user asks about SEO optimization, search visibility, meta
  descriptions, structured data, sitemap, robots.txt, OpenGraph, or phrases like 'SEO 优化',
  '搜索引擎优化', 'OG 标签', '搜索排名'.
metadata:
  version: "1.0.0"
  platforms: [codex, antigravity, claude-code]
  tags:
    - seo
    - meta-tags
    - structured-data
    - json-ld
    - opengraph
    - sitemap
    - robots
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - SEO 优化
  - 搜索引擎优化
  - OG 标签
  - 搜索排名
  - search visibility
  - meta descriptions
  - structured data
  - sitemap
  - robots
  - seo
---

# seo-web

This skill owns technical SEO: ensuring web pages are correctly structured,
annotated, and configured for optimal search engine discovery and ranking.

## When to use

### As primary owner
- The user wants a technical SEO audit
- The user wants to add or fix meta tags, structured data, sitemaps, or robots.txt
- The task focuses on search engine visibility and discoverability
- Best for requests like:
  - "帮我做 SEO 优化"
  - "检查一下 meta 标签和 OG 标签"
  - "生成 sitemap.xml"
  - "这个页面的结构化数据对不对"

### As overlay on another owner
- The user is building a page/app with a framework skill as owner, and SEO is a secondary concern
- Pair with `$nextjs` (generateMetadata), `$react` (react-helmet), `$vue` (Nuxt SEO)

## Do not use

- The task is web performance optimization → use `$performance-expert` (though CWV affects SEO, perf is the primary skill)
- The task is content writing or copywriting → SEO provides structural guidance, not content creation
- The task is paid search / SEM / advertising campaigns → out of scope
- The task is pure CSS/layout without SEO concern → use `$css-pro`

## Reference

For detailed workflow, examples, and implementation guidance, see [references/detailed-guide.md](./references/detailed-guide.md).
