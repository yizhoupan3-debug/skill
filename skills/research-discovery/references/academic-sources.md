---
name: academic-sources
description: |
  Verified-open external academic data sources for agent research harness.
  Five sources tested 2026-05-31 via router-rs-framework web_fetch; all returned
  HTTP 200 with usable structured data. Use these as the retrieval backbone for
  `external_research` lane queries.
metadata:
  type: reference
  verified: 2026-05-31
  verified_by: router-rs-framework web_fetch
---

# Academic Sources — Agent Research Harness

Five open-access academic data sources verified as reachable
via `router-rs-framework` `web_fetch`. No API keys required for basic use.

## Source Index

| # | Source | Protocol | Auth | Best For | Rate Limit Notes |
|---|--------|----------|------|----------|------------------|
| 1 | **arXiv** | Atom/XML API | None | Preprints, full-text PDFs, CS/Physics/Math | Polite: ≤1 req/3s |
| 2 | **OpenAlex** | REST JSON | None (polite pool w/ email) | Massive cross-discipline metadata, OA status | Free; `mailto` param for higher limit |
| 3 | **CrossRef** | REST JSON | None | DOI resolution, citation metadata, publisher info | Polite: ≤50 req/s without key |
| 4 | **PubMed E-utilities** | XML | None (key recommended) | Biomedical, life sciences, clinical | 3 req/s without key, 10 req/s with key |
| 5 | **DOAJ** | REST JSON | None | Open-access journal discovery, OA article metadata | Reasonable use |

---

## 1. arXiv API

**Endpoint:** `https://export.arxiv.org/api/query`
**Docs:** https://info.arxiv.org/help/api/index.html

### Query Pattern

```
GET https://export.arxiv.org/api/query?search_query={QUERY}&start={OFFSET}&max_results={LIMIT}
```

### Search Query Syntax

| Field | Prefix | Example |
|-------|--------|---------|
| All fields | `all:` | `all:transformer` |
| Title | `ti:` | `ti:attention is all you need` |
| Author | `au:` | `au:Hinton` |
| Abstract | `ab:` | `ab:diffusion model` |
| Category | `cat:` | `cat:cs.LG` |
| Date range | `submittedDate:[YYYYMMDDHHMI TO ...]` | `submittedDate:[202401010000 TO 202412312359]` |

Boolean: `AND`, `OR`, `ANDNOT` (no spaces around operators).

### Response Fields (per `<entry>`)

- `id` — abs URL (e.g. `http://arxiv.org/abs/2201.00978v1`)
- `title` — paper title
- `summary` — abstract
- `category` — primary arXiv category (e.g. `cs.CV`)
- `published` / `updated` — timestamps
- `link[title="pdf"]` — direct PDF URL

### Example: Fetch latest 3 CV papers on diffusion

```
GET https://export.arxiv.org/api/query?search_query=cat:cs.CV+AND+ab:diffusion&sortBy=submittedDate&sortOrder=descending&max_results=3
```

### Full-text PDF

Construct PDF URL from abs URL: replace `/abs/` with `/pdf/`.
Example: `http://arxiv.org/pdf/2201.00978v1`

---

## 2. OpenAlex API

**Endpoint:** `https://api.openalex.org`
**Docs:** https://docs.openalex.org

### Query Pattern

```
GET https://api.openalex.org/works?search={QUERY}&per_page={LIMIT}&page={PAGE}
```

Append `&mailto={EMAIL}` to join the polite pool (higher rate limit, credited
in metadata).

### Key Parameters

| Param | Purpose | Example |
|-------|---------|---------|
| `search` | Full-text relevance search | `search=vision transformer` |
| `filter` | Structured filters | `filter=publication_year:2024,type:article,is_oa:true` |
| `sort` | Sort field + direction | `sort=cited_by_count:desc` |
| `select` | Limit returned fields | `select=id,title,doi,publication_year,cited_by_count` |
| `per_page` | Results per page (max 200) | `per_page=10` |
| `page` | Page number | `page=1` |

### Useful Filters

```
publication_year:2024          # by year
is_oa:true                     # open access only
authorships.author.id:A50...   # specific author
locations.source.id:S123...    # specific venue
cited_by_count:>100            # highly cited
type:article | type:review     # article type
```

### Response Shape

```json
{
  "meta": { "count": 1031583, "page": 1, "per_page": 1 },
  "results": [{
    "id": "https://openalex.org/W...",
    "doi": "https://doi.org/10.xxx",
    "title": "...",
    "publication_year": 2024,
    "open_access": { "is_oa": true, "oa_url": "..." },
    "authorships": [...],
    "cited_by_count": 42
  }]
}
```

### Entity Lookups

- Author: `GET /authors/{id}`
- Venue/Source: `GET /sources/{id}`
- Concept/Topic: `GET /topics/{id}`

---

## 3. CrossRef API

**Endpoint:** `https://api.crossref.org`
**Docs:** https://api.crossref.org

### Query Pattern

```
GET https://api.crossref.org/works?query={QUERY}&rows={LIMIT}&offset={OFFSET}
```

### Key Parameters

| Param | Purpose | Example |
|-------|---------|---------|
| `query` | Relevance search | `query=attention mechanism` |
| `query.author` | Author search | `query.author=Geoffrey Hinton` |
| `query.title` | Title search | `query.title=BERT` |
| `query.bibliographic` | Full bibliographic search | `query.bibliographic=deep learning` |
| `rows` | Results per page | `rows=10` |
| `offset` | Pagination offset | `offset=0` |
| `filter` | Field filters | `filter=from-pub-date:2024,until-pub-date:2024` |
| `sort` | Sort field | `sort=is-referenced-by-count&order=desc` |
| `select` | Limit fields | `select=DOI,title,author,published-print,is-referenced-by-count` |

### DOI Lookup

```
GET https://api.crossref.org/works/{DOI}
```

### Response Shape

```json
{
  "status": "ok",
  "message": {
    "total-results": 96471,
    "items": [{
      "DOI": "10.xxx",
      "title": ["..."],
      "author": [{"given": "A.", "family": "B."}],
      "is-referenced-by-count": 42,
      "container-title": ["Journal Name"],
      "issued": { "date-parts": [[2024]] }
    }]
  }
}
```

### Useful for

- Resolving DOIs to full metadata
- Checking citation counts
- Finding publisher and venue information
- Verifying reference completeness in `citation-management`

---

## 4. PubMed E-utilities

**Endpoint:** `https://eutils.ncbi.nlm.nih.gov/entrez/eutils/`
**Docs:** https://www.ncbi.nlm.nih.gov/books/NBK25501/

### Two-Step Query Pattern

**Step 1 — Search (get IDs):**
```
GET https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi?db=pubmed&term={QUERY}&retmax={LIMIT}&retmode=json
```

**Step 2 — Fetch details (by IDs):**
```
GET https://eutils.ncbi.nlm.nih.gov/entrez/eutils/efetch.fcgi?db=pubmed&id={ID1},{ID2},...&rettype=abstract&retmode=xml
```

Alternative Step 2 (lighter, JSON):
```
GET https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esummary.fcgi?db=pubmed&id={ID}&retmode=json
```

### Search Query Syntax

| Field | Tag | Example |
|-------|-----|---------|
| All fields | `[All Fields]` | `transformer[All Fields]` |
| Title | `[Title]` | `BERT[Title]` |
| Author | `[Author]` | `Hinton G[Author]` |
| MeSH term | `[MeSH Terms]` | `deep learning[MeSH Terms]` |
| Date | `[Date - Publication]` | `2024[Date - Publication]` |
| Journal | `[Journal]` | `Nature[Journal]` |

Boolean: `AND`, `OR`, `NOT`.

### Useful for

- Biomedical, clinical, life sciences literature
- MeSH-controlled vocabulary search
- Linking PubMed IDs to full-text via PMC

---

## 5. DOAJ API

**Endpoint:** `https://doaj.org/api`
**Docs:** https://doaj.org/api/docs

### Search Journals

```
GET https://doaj.org/api/search/journals/{QUERY}?page=1&pageSize=10
```

### Search Articles

```
GET https://doaj.org/api/search/articles/{QUERY}?page=1&pageSize=10
```

### Journal by ID

```
GET https://doaj.org/api/journals/{ISSN}
```

### Response Shape (articles)

```json
{
  "total": 12345,
  "page": 1,
  "pageSize": 10,
  "results": [{
    "id": "...",
    "title": "...",
    "bibjson": {
      "title": "...",
      "abstract": "...",
      "year": "2024",
      "journal": { "title": "...", "publisher": "..." },
      "link": [{ "type": "fulltext", "url": "..." }]
    }
  }]
}
```

### Useful for

- Finding OA journals in a field
- Discovering OA article versions of paywalled papers
- Verifying journal DOAJ inclusion for `citation-management`

---

## Agent Usage Patterns

### Pattern A: Broad discovery (start here)

Use **OpenAlex** for cross-discipline broad queries; it has the largest
deduplicated corpus and supports structured filters.

```
OpenAlex: search={topic} + filter=is_oa:true,publication_year:{year},sort=cited_by_count:desc
```

### Pattern B: Preprint-first (fast, full-text)

Use **arXiv** when the target is CS/Physics/Math preprints with immediate
PDF access.

```
arXiv: search_query=cat:{category}+AND+ab:{keywords}&sortBy=submittedDate&sortOrder=descending
```

### Pattern C: DOI verification and citation metadata

Use **CrossRef** to resolve DOIs, check citation counts, and verify
reference metadata.

```
CrossRef: query={title_or_author}&sort=is-referenced-by-count&order=desc
```

### Pattern D: Biomedical/life sciences

Use **PubMed E-utilities** for clinical, biomedical, and life science queries
with MeSH-controlled vocabulary.

```
PubMed Step 1: esearch?db=pubmed&term={query}+AND+{year}[Date]
PubMed Step 2: efetch?db=pubmed&id={ids}&rettype=abstract
```

### Pattern E: Open-access journal/article discovery

Use **DOAJ** to find OA journals and OA article versions.

```
DOAJ: /api/search/articles/{query}
```

### Multi-source fan-out (recommended for deep research)

For thorough literature coverage, run **OpenAlex** (breadth) + **arXiv**
(preprints) + **PubMed** (if biomedical) in parallel, then deduplicate
by DOI/title before synthesis.

### Pattern F: Math background / unknown property (STEM §G)

For **theory landscape** and **theorem applicability** (not manuscript review),
run at least **two** of arXiv + OpenAlex + CrossRef; record queries in
`retrieval_fanout_plan` and executed lines in `retrieval_trace.queries_used`.

```
arXiv: cat:math.AP+AND+ab:{phenomenon keywords}   # or math.PR, math.OC, stat.ML, etc.
arXiv: cat:math.AP+AND+ti:{standard theorem name}
OpenAlex: search={concept} + filter=topics.id:{field},from_publication_date:{year}
CrossRef: query.bibliographic={survey or textbook title}&rows=5
```

Synthesis must populate `theory_background.theorem_applicability` (applies_when /
fails_when) and `analogy_candidates.breaks_when` — see
[math-background-inquiry.md](../../../docs/references/rfv-loop/math-background-inquiry.md).

---

## Error Handling

| Source | Common Errors | Mitigation |
|--------|---------------|------------|
| arXiv | Empty results for narrow queries | Broaden search terms, try adjacent categories |
| OpenAlex | 429 Too Many Requests | Add `mailto` param for polite pool; back off 2s |
| CrossRef | 429 with burst queries | Space requests; use `mailto` header |
| PubMed | 429 (3 req/s limit) | Add API key for 10 req/s; space requests |
| DOAJ | Slow responses on large queries | Reduce `pageSize`; paginate |

All sources return structured errors in their response body; parse
`status`, `message`, or `error` fields accordingly.

---

## Alternative: `web_search` Tool

For general research queries (not discipline-specific API calls), the framework MCP
provides `web_search` which aggregates multiple free search engines:

- **SearXNG** (self-hosted, configure via `SEARXNG_URL` env var) — aggregates 70+ engines
- **StackOverflow** + **GitHub** — for technical queries (`topic=tech`)
- **HN Algolia** — for news/trends (`topic=news`)
- **Wikipedia** — for knowledge/background (`topic=knowledge`)
- **arXiv** — for academic queries (`topic=academic`)
- **Brave Search** — optional, configure via `BRAVE_API_KEY`

Usage: `web_search(query, topic?)` — returns `{title, url, snippet}` per result.
Complementary to the structured APIs above; use `web_search` for discovery,
then `web_fetch` for deep reading of specific sources.
