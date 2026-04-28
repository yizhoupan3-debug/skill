---
name: web-scraping
description: |
  Plan and implement web scraping and structured data extraction workflows.
  Use when the user asks to scrape a website, extract data from web pages,
  crawl multiple pages, build a spider, or says “抓取网页数据”, “爬虫”, “网页信息提取”,
  “批量抓取”, or “web crawler”. Covers static and dynamic scraping, pagination,
  anti-bot tactics, and JSON/CSV/database output. This skill owns extraction;
  use built-in browser/browser-use capability only when live interaction is required.
metadata:
  version: "1.0.0"
  platforms: [codex]
  tags:
    - web-scraping
    - data-extraction
    - crawler
    - cheerio
    - beautifulsoup
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 抓取网页数据
  - 爬虫
  - 网页信息提取
  - 批量抓取
  - web crawler
  - extract data from web pages
  - crawl multiple pages
  - build a spider
  - says "抓取网页数据
  - web scraping
---

# web-scraping

This skill owns web data extraction workflows: choosing the right scraping
approach, implementing extraction logic, handling pagination and anti-bot
measures, and structuring output data.

## When to use

- The user wants to extract data from one or more web pages
- The task involves building a crawler, spider, or scraping pipeline
- The user needs structured data from websites (product listings, prices, articles, contacts)
- The task involves pagination, infinite scroll, or multi-page traversal
- Best for requests like:
  - "帮我抓这个网站的数据"
  - "写一个爬虫抓取产品列表"
  - "批量提取这些页面的信息"
  - "Scrape this site and output a CSV"

## Do not use

- The task is browser automation for testing or UI interaction without data extraction → use the built-in browser/browser-use capability
- The task is API integration / calling a documented REST/GraphQL API → use `$api-integration-debugging`
- The task is data cleaning/transformation of already-extracted data → use `$data-wrangling`
- The task is building a search engine or indexing system → broader architecture

## Task ownership and boundaries

This skill owns:
- scraping strategy selection (static vs dynamic)
- extraction logic (selectors, XPath, JSON-LD, regex)
- pagination and traversal patterns
- rate limiting and polite crawling
- anti-bot bypass strategies (headers, delays, proxy rotation)
- output formatting and persistence

This skill does not own:
- browser automation for non-extraction purposes → built-in browser/browser-use capability
- API client implementation → `$api-integration-debugging`
- data cleaning of already-extracted datasets → `$data-wrangling`
- legal/compliance advice on scraping (flag concerns but do not provide legal counsel)
- **Dual-Dimension Audit (Pre: Selectors/Flow, Post: Extraction-Fidelity/Anti-bot Results)** → runtime verification gate

If the task shifts to adjacent skill territory, route to:
- built-in browser/browser-use capability for browser automation beyond extraction
- `$data-wrangling` for post-extraction data cleaning
- `$api-integration-debugging` if the site offers a documented API

## Required workflow

1. Analyze the target: static HTML, JS-rendered, login-required, API-backed?
2. Choose approach: static fetch vs browser automation vs API-first.
3. Implement extraction with proper selectors.
4. Handle pagination, rate limits, and error recovery.
5. Format and validate output data.
6. Flag ethical/legal considerations.

## Core workflow

### 1. Target Analysis

Before writing any scraping code, determine:

- **Rendering**: Is the content in the initial HTML response, or JS-rendered?
  - Quick check: `curl -s <url> | grep <target-text>`
  - If not in HTML → needs Playwright or headless browser
- **Data source**: Does the site have a public API or JSON endpoint?
  - Check Network tab for XHR/fetch requests → often easier to scrape the API directly
  - Check for `<script type="application/ld+json">` structured data
- **Authentication**: Does the content require login?
  - Cookie-based → need session management
  - OAuth → may need API approach instead
- **Anti-bot**: Does the site use Cloudflare, reCAPTCHA, rate limiting?
  - Respect robots.txt; check `/robots.txt` first
  - Plan for delays, user-agent rotation, proxy if needed

### 2. Approach Selection

| Scenario | Approach | Tools |
|----------|----------|-------|
| Static HTML, no JS rendering | HTTP fetch + parse | Node: `fetch` + `cheerio`; Python: `httpx` + `BeautifulSoup` / `lxml` |
| JS-rendered content | Headless browser | Use built-in browser/browser-use capability when interaction is required |
| API available | Direct API calls | `fetch` / `httpx` with proper headers |
| Large-scale crawl | Crawler framework | Python: `Scrapy`; Node: `crawlee` |

### 3. Extraction Patterns

#### CSS Selectors (preferred for most cases)
```javascript
// Node.js with cheerio
const $ = cheerio.load(html);
const items = $('.product-card').map((i, el) => ({
  name: $(el).find('h2').text().trim(),
  price: $(el).find('.price').text().trim(),
  url: $(el).find('a').attr('href'),
})).get();
```

#### JSON-LD / Structured Data
```javascript
// Extract structured data from page
const ldJson = $('script[type="application/ld+json"]')
  .map((i, el) => JSON.parse($(el).html()))
  .get();
```

#### XPath (when CSS selectors are insufficient)
```python
# Python with lxml
from lxml import html
tree = html.fromstring(page_content)
titles = tree.xpath('//div[@class="product"]//h2/text()')
```

### 4. Pagination Handling

- **URL-based**: increment page number or offset parameter
- **Next button**: extract next page URL from link element
- **Infinite scroll**: use Playwright to scroll and wait for new content
- **Cursor-based API**: follow `next_cursor` or `after` parameter

```javascript
// URL-based pagination
for (let page = 1; page <= maxPages; page++) {
  const html = await fetch(`${baseUrl}?page=${page}`).then(r => r.text());
  // extract and accumulate data
  await delay(1000 + Math.random() * 1000); // polite delay
}
```

### 5. Rate Limiting & Politeness

- **Always**: check and respect `robots.txt`
- **Delay**: 1-3 seconds between requests minimum; randomize
- **User-Agent**: set a descriptive user-agent string
- **Concurrent requests**: limit to 2-5 parallel requests
- **Error handling**: exponential backoff on 429/503 responses
- **Session management**: rotate cookies/sessions if needed

### 6. Output Formatting

- JSON: `JSON.stringify(data, null, 2)` → file
- CSV: use `csv-stringify` (Node) or `csv` module (Python)
- Database: batch insert with transaction for consistency
- Always validate: check for null/empty fields, deduplicate

## Hard constraints

- Always check `robots.txt` before scraping and flag any disallowed paths.
- Always add reasonable delays between requests (minimum 1 second).
- Do not store or transmit personal data without explicit user instruction.
- Flag potential legal/ethical concerns (ToS violations, personal data) but do not refuse the task.
- Prefer API endpoints over HTML scraping when available.
- Always handle errors gracefully: retry on transient failures, skip and log on persistent ones.
- Validate extracted data before outputting: check for empty fields, duplicates, malformed entries.
- **Superior Quality Audit**: For large-scale or high-fidelity scraping, apply the runtime verification gate to verify against [Superior Quality Bar](runtime verification criteria).

## Trigger examples

- "Use $web-scraping to extract product data from this e-commerce site."
- "帮我写一个爬虫，抓取这个网站的文章列表。"
- "Scrape this page and save the results as CSV."
- "批量提取这些页面的价格和标题。"
- "强制进行爬虫深度审计 / 检查字段提取完整性与防封结果。"
- "Use the runtime verification gate to audit this scraper for extraction-fidelity idealism."
