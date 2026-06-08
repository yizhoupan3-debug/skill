export const meta = {
  name: 'deep-research',
  description: 'Deep research harness — fan-out web searches, fetch sources, adversarially verify claims, synthesize a cited report.',
  whenToUse: 'When the user wants a deep, multi-source, fact-checked research report on any topic.',
  phases: [
    { title: 'Plan', detail: 'Decompose query into search vectors' },
    { title: 'Search', detail: 'Sweep web for diverse sources' },
    { title: 'Extract', detail: 'Read pages and extract claims/evidence' },
    { title: 'Verify', detail: 'Adversarially verify extracted claims' },
    { title: 'Synthesize', detail: 'Write cited report' },
  ],
}

if (!args) {
  throw new Error("Missing research topic in args. Usage: /workflow deep-research <topic>")
}

const topic = typeof args === 'string' ? args : JSON.stringify(args)
log(`Researching topic: ${topic}`)

phase('Plan')
log(`Planning search vectors...`)
const planPrompt = `
You are a research planning agent. The user wants to research:
"${topic}"

Decompose this into 3-5 distinct, specific search queries that will cover different angles of the topic.
Return JSON ONLY:
{
  "queries": ["query 1", "query 2", ...]
}
`

const planObj = await agent(planPrompt, {
  label: 'plan-searches',
  phase: 'Plan',
  model: 'haiku',
  schema: {
    type: "object",
    properties: {
      queries: { type: "array", items: { type: "string" } }
    },
    required: ["queries"]
  }
})

const queries = planObj.queries
log(`Generated ${queries.length} search vectors`)

phase('Search')
log(`Running searches...`)
const searchResults = await parallel(
  queries.map(q => () => agent(
    `Run a WebSearch for "${q}" and return the top 3 highly relevant URLs. You must use the WebSearch tool.
    Return JSON ONLY:
    { "urls": ["url1", "url2", "url3"] }`,
    {
      label: `search:${q.substring(0, 20)}`,
      phase: 'Search',
      model: 'haiku',
      schema: {
        type: "object",
        properties: {
          urls: { type: "array", items: { type: "string" } }
        },
        required: ["urls"]
      }
    }
  ))
)

// Deduplicate URLs
const urlSet = new Set()
searchResults.filter(Boolean).forEach(r => {
  if (r.urls) r.urls.forEach(url => urlSet.add(url))
})
const uniqueUrls = Array.from(urlSet).slice(0, 10) // Cap at 10 URLs
log(`Found ${uniqueUrls.length} unique sources to fetch`)

if (uniqueUrls.length === 0) {
  return { error: "No search results found." }
}

phase('Extract')
log(`Fetching and extracting claims...`)
const extractionResults = await pipeline(
  uniqueUrls,
  url => agent(
    `Fetch this URL using the WebFetch tool: ${url}
    Then extract the key facts, claims, and evidence relevant to: "${topic}"
    Return JSON ONLY:
    {
      "claims": [
        { "fact": "The claim text", "evidence": "Quote or specific context from page" }
      ]
    }`,
    {
      label: `extract:${new URL(url).hostname}`,
      phase: 'Extract',
      model: 'haiku',
      schema: {
        type: "object",
        properties: {
          claims: {
            type: "array",
            items: {
              type: "object",
              properties: {
                fact: { type: "string" },
                evidence: { type: "string" }
              },
              required: ["fact", "evidence"]
            }
          }
        },
        required: ["claims"]
      }
    }
  ).then(res => ({ url, claims: res?.claims || [] })).catch(() => ({ url, claims: [] }))
)

const allClaims = extractionResults.flatMap(r =>
  r.claims.map(c => ({ ...c, url: r.url }))
)
log(`Extracted ${allClaims.length} raw claims`)

phase('Verify')
log(`Adversarially verifying claims...`)

// Deduplicate claims conceptually before verification
const dedupPrompt = `
Here are claims extracted from multiple sources about: "${topic}"
Merge overlapping claims and filter out irrelevant ones.
Return a clean list of distinct claims, each with its supporting URLs.
Return JSON ONLY:
{
  "merged_claims": [
    { "claim": "Merged claim text", "urls": ["url1", "url2"] }
  ]
}

Raw claims:
${JSON.stringify(allClaims, null, 2)}
`

const dedupObj = await agent(dedupPrompt, {
  label: 'dedup-claims',
  phase: 'Verify',
  model: 'haiku',
  schema: {
    type: "object",
    properties: {
      merged_claims: {
        type: "array",
        items: {
          type: "object",
          properties: {
            claim: { type: "string" },
            urls: { type: "array", items: { type: "string" } }
          },
          required: ["claim", "urls"]
        }
      }
    },
    required: ["merged_claims"]
  }
})

const mergedClaims = dedupObj.merged_claims || []
log(`Consolidated into ${mergedClaims.length} distinct claims for verification`)

const verifiedClaims = await parallel(
  mergedClaims.map(c => () => agent(
    `Adversarially verify this claim based on general knowledge and internal consistency.
    Claim: "${c.claim}"
    Sources: ${c.urls.join(", ")}
    Is this claim factually sound, logically coherent, and generally accepted? Or is it highly contested, dubious, or subjective?
    Return JSON ONLY:
    {
      "is_valid": true/false,
      "nuance": "Important context, caveats, or corrections"
    }`,
    {
      label: `verify:${c.claim.substring(0, 30)}`,
      phase: 'Verify',
      model: 'haiku',
      schema: {
        type: "object",
        properties: {
          is_valid: { type: "boolean" },
          nuance: { type: "string" }
        },
        required: ["is_valid", "nuance"]
      }
    }
  ).then(res => ({ ...c, ...res })))
)

const finalClaims = verifiedClaims.filter(Boolean)
const validClaims = finalClaims.filter(c => c.is_valid)
log(`${validClaims.length}/${finalClaims.length} claims passed verification`)

phase('Synthesize')
log(`Synthesizing final report...`)

const synthPrompt = `
Write a comprehensive, well-structured research report on: "${topic}"

Use the following verified claims and context. You MUST cite your sources inline using markdown links (e.g., [Source](url)).
Do NOT include claims that failed verification unless discussing them as misconceptions.

Verified Claims:
${JSON.stringify(validClaims, null, 2)}

Refuted/Contested Claims (for context/debunking if relevant):
${JSON.stringify(finalClaims.filter(c => !c.is_valid), null, 2)}

The report should be in simplified Chinese (面向用户的可见输出使用简体中文), written in a professional, academic style.
Include:
1. Executive Summary
2. Detailed Findings (structured by themes)
3. Nuances & Caveats
4. References (list of URLs)
`

const report = await agent(synthPrompt, {
  label: 'write-report',
  phase: 'Synthesize',
  model: 'sonnet'
})

return { report }