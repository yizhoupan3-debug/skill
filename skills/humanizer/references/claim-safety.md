# Claim Safety for Detector-Related Requests

Use this note only when the user explicitly asks about AI scores, AIGC percentages, Turnitin, or other detectors.
Do not repeat this note in ordinary humanizer deliveries that are simply asking for better text.

## Safe wording

- Say `may reduce obvious machine-like signals`, not `will pass the detector`
- Say `tool-dependent and contextual`, not `objective` or `guaranteed`
- Say `heuristic feedback`, not `ground truth`
- Say `sentence judgments are review priorities`, not `sentence truth scores`
- Say `quality-first editing`, not `detector evasion`
- Say `the result varies by tool, language, length, and domain`

## Unsafe wording

- `100% human-like`
- `guaranteed to pass`
- `bypass detection`
- `fool Turnitin`
- `detector-proof`

## Editing rule

If a detector claim is not directly supported by the source text or by a cited source, treat it as a caution rather than a promise.

## Source notes

- Turnitin describes AI writing detection as a signaling tool and explicitly says false positives are possible, including a sentence-level false positive rate around 4% in one explanation of its model. See [Turnitin AI writing detection update](https://www.turnitin.com/blog/ai-writing-detection-what-academic-leaders-need-to-know-as-technology-matures) and [Turnitin false positive rate article](https://www.turnitin.com/blog/understanding-the-false-positive-rate-for-sentences-of-our-ai-writing-detection-capability).
- Turnitin’s guide says text in the 1% to 19% range gets an asterisk instead of a score to avoid potential false positives. See [Turnitin AI writing detection model guide](https://guides.turnitin.com/hc/en-us/articles/28294949544717-AI-writing-detection-model).
- Recent ACL and LREC work reports that adversarial perturbations and paraphrasing can materially change detector behavior, and that some detectors remain vulnerable to small wording changes. See [ACL 2024 on adversarial perturbations](https://aclanthology.org/2024.acl-long.327/), [LREC-COLING 2024 on humanizing machine-generated content](https://aclanthology.org/2024.lrec-main.739/), and [ACL 2025 on paraphrase inversion](https://aclanthology.org/2025.findings-acl.227.pdf).

## Preferred framing

- `may reduce obvious machine-like signals` is safer than `will pass`
- `tool-dependent and contextual` is safer than `guaranteed`
- `heuristic feedback` is safer than `proof`
- `sentence-level judgment is a triage aid` is safer than `sentence-level certainty`
- `quality-first editing` is safer than `detector evasion`
- `the result varies by tool, language, length, and domain` is safer than a universal claim
