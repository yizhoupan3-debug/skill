import { agent, pipeline, phase, log } from "workflow"
import { chunkedParallel } from './workflow-helpers.js'
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

const recoveryTrace = { searches: [], extractionErrors: [], fetchFailures: [], warnings: [] }

function safeHostname(url) {
  try {
    return new URL(url).hostname
  } catch {
    return null
  }
}
function normalizeUrl(url) {
  if (!url) return ''
  try { new URL(url); return url } catch { return 'https://' + url }
}

phase('Plan')
log(`Planning search vectors...`)
const planObj = await agent(`
You are a research planning agent. The user wants to research:
"${topic}"

Decompose this into 3-5 distinct, specific search queries that will cover different angles of the topic.
Return JSON ONLY:
{
  "queries": ["query 1", "query 2", ...]
}
`, {
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
recoveryTrace.searches = queries.map(q => ({ query: q, yieldedUrls: 0 }))

phase('Search')
log(`Running searches...`)
const searchResults = await chunkedParallel(
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
  ).then(res => {
    const found = res?.urls?.length || 0
    // Update recovery trace for the matching query
    for (const s of recoveryTrace.searches) {
      if (s.query === q) s.yieldedUrls = found
    }
    return res
  }))
)

// Deduplicate URLs
const urlSet = new Set()
searchResults.filter(Boolean).forEach(r => {
  if (r.urls) r.urls.forEach(url => {
    try { new URL(url); urlSet.add(url) } catch {
      // silently skip malformed URLs from search results
    }
  })
})
const uniqueUrls = Array.from(urlSet).slice(0, 10)
log(`Found ${uniqueUrls.length} unique sources to fetch`)

if (uniqueUrls.length === 0) {
  return { error: "No search results found.", recoveryTrace }
}
if (uniqueUrls.length < 3) {
  recoveryTrace.warnings.push("Fewer than 3 unique sources — coverage may be thin")
}

phase('Extract')
log(`Fetching and extracting claims...`)
const extractionErrors = []
const extractionResults = await pipeline(
  uniqueUrls,
  url => {
    const hostname = safeHostname(url) || url.replace(/[^a-zA-Z0-9]/g, '').substring(0, 25) || 'unknown'
    return agent(
      `Fetch this URL using the WebFetch tool: ${normalizeUrl(url)}
    Then extract the key facts, claims, and evidence relevant to: "${topic}"
    Return JSON ONLY:
    {
      "claims": [
        { "fact": "The claim text", "evidence": "Quote or specific context from page" }
      ]
    }`,
      {
        label: `extract:${hostname}`,
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
    ).then(res => ({ url, claims: res?.claims || [] }))
     .catch(err => {
       extractionErrors.push({ url, error: extractErrorSummary(err) })
       return { url, claims: [] }
     })
  }
)

// Collect extraction trace
for (const r of extractionResults) {
  if (r.claims.length === 0 && extractionErrors.find(e => e.url === r.url)) {
    recoveryTrace.extractionErrors.push(extractionErrors.find(e => e.url === r.url))
  }
}
const totalFetched = extractionResults.filter(r => r.claims.length > 0).length
recoveryTrace.fetchFailures.push({ total: uniqueUrls.length, succeeded: totalFetched, failed: uniqueUrls.length - totalFetched })

const allClaims = extractionResults.flatMap(r =>
  r.claims.map(c => ({ ...c, url: r.url }))
)
log(`Extracted ${allClaims.length} raw claims from ${totalFetched} sources`)

if (allClaims.length === 0) {
  recoveryTrace.warnings.push("No claims extracted — sources may be paywalled or unresponsive")
  return { error: "No claims could be extracted from any source.", recoveryTrace }
}

phase('Verify')
log(`Cross-referencing and verifying claims across sources...`)

// Dedup with sonnet for semantic accuracy
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
  model: 'sonnet',
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

const verifiedClaims = await chunkedParallel(
  mergedClaims.map(c => () => {
    // Collect evidence from all sources for this claim
    const sourceEvidence = c.urls.map(u => {
      const src = extractionResults.find(r => r.url === u)
      const ev = src?.claims?.find(cc => cc.fact === c.claim)
      return `- [${u}]: ${ev?.evidence || '(extracted without specific evidence)'}`
    }).join('\n')

    return agent(
      `You are an adversarial fact-checker. Cross-reference this claim against ALL its source evidence.

    Topic: "${topic}"

    Claim: "${c.claim}"

    Evidence from each source:
    ${sourceEvidence}

    Tasks:
    1. Do the sources AGREE or CONTRADICT each other on this claim?
    2. Does the evidence actually support the claim (not just restate it)?
    3. Is the claim logically coherent and generally accepted, or contested/dubious?
    4. Are there known counterarguments or caveats the evidence misses?

    Return JSON ONLY:
    {
      "verdict": "verified" or "contested" or "refuted",
      "sources_agree": true or false,
      "evidence_supports_claim": true or false,
      "contradictions": "description of any cross-source contradictions, or null",
      "confidence": "high" or "medium" or "low",
      "nuance": "Important caveats, corrections, or context. Keep concise but specific."
    }`,
      {
        label: `verify:${c.claim.substring(0, 30)}`,
        phase: 'Verify',
        model: 'sonnet',
        schema: {
          type: "object",
          properties: {
            verdict: { type: "string", enum: ["verified", "contested", "refuted"] },
            sources_agree: { type: "boolean" },
            evidence_supports_claim: { type: "boolean" },
            contradictions: { type: ["string", "null"] },
            confidence: { type: "string", enum: ["high", "medium", "low"] },
            nuance: { type: "string" }
          },
          required: ["verdict", "sources_agree", "evidence_supports_claim", "confidence", "nuance"]
        }
      }
    ).then(res => ({ ...c, ...res }))
  })
)

const finalClaims = verifiedClaims.filter(Boolean)
const validClaims = finalClaims.filter(c => c.verdict === 'verified')
const contestedClaims = finalClaims.filter(c => c.verdict === 'contested')
const refutedClaims = finalClaims.filter(c => c.verdict === 'refuted')
log(`${validClaims.length} verified, ${contestedClaims.length} contested, ${refutedClaims.length} refuted`)

if (validClaims.length === 0) {
  recoveryTrace.warnings.push("No claims passed adversarial verification — report will be thin or caveat-heavy")
}

phase('Synthesize')
log(`Synthesizing final report...`)

const synthPrompt = `
Write a comprehensive, well-structured research report on: "${topic}"

Use the following claims and context. You MUST cite your sources inline using markdown links (e.g., [Source](url)).
Do NOT include refuted claims in the main body; contested claims go in the Caveats section.

Verified Claims:
${JSON.stringify(validClaims, null, 2)}

Contested Claims (discuss under Nuances & Caveats):
${JSON.stringify(contestedClaims, null, 2)}

Recovery notes (for the methods section):
${JSON.stringify(recoveryTrace, null, 2)}

The report should be in simplified Chinese (面向用户的可见输出使用简体中文), written in a professional, academic style.
Include:
1. Executive Summary
2. Detailed Findings (structured by themes, not by source)
3. Nuances & Caveats (contested claims, limitations, open questions)
4. References (list of all URLs cited, with brief descriptions)
5. Recovery trace (Appendix: what searches ran, which yielded results, what was excluded and why)
`

const report = await agent(synthPrompt, {
  label: 'write-report',
  phase: 'Synthesize',
  model: 'sonnet'
})

return { report, recoveryTrace }


// ---- helpers ----

function extractErrorSummary(err) {
  const msg = err?.message || String(err)
  if (msg.includes('timeout') || msg.includes('Timeout')) return 'fetch_timeout'
  if (msg.includes('404') || msg.includes('not found')) return 'not_found_404'
  if (msg.includes('403') || msg.includes('forbidden')) return 'access_denied_403'
  if (msg.includes('paywall') || msg.includes('Paywall')) return 'paywall_blocked'
  if (msg.includes('fetch') || msg.includes('Fetch')) return 'fetch_error'
  return msg.substring(0, 80)
}
