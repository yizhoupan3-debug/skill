# Section Budget Norms & Conference Page Limits

Reference data for `paper-length-tuner` skill.

## Major Conference Page Limits

| Venue | Submission Pages | Camera-Ready | References | Appendix |
|---|---|---|---|---|
| **AAAI** | 7 (technical) | 7 + 2 purchasable | Excluded | Excluded |
| **ICML** | 8 (main body) | 9 (main body) | Unlimited, excluded | Unlimited, same PDF |
| **NeurIPS** | 9 (incl fig/table) | 10 (incl fig/table) | Excluded | Separate supplementary |
| **CVPR** | 8 (excl references) | 8 + 1 extra | Excluded | Separate supplementary |
| **ICLR** | 8-10 (varies by year) | +1 page | Excluded | Separate supplementary |
| **ACL** | 8 (long) / 4 (short) | 9 (long) / 5 (short) | Excluded | Unlimited appendix |
| **ECCV** | 14 (incl references) | 14 | Included | Separate supplementary |
| **KDD** | 9 (research) / 9 (applied) | 9 | Excluded | Not standardized |

> [!NOTE]
> Always verify the latest CFP — limits change year to year.

## Words-per-Page Estimates

Typical two-column conference templates (10pt, standard margins):

| Template Style | Words/Page (text only) | With Figures |
|---|---|---|
| IEEE two-column | ~800-900 | ~600-700 |
| ACM two-column | ~750-850 | ~550-650 |
| NeurIPS single-column | ~500-600 | ~400-500 |
| AAAI two-column | ~800-900 | ~600-700 |

## Typical Section Proportion Norms

### Conference Paper (8-page template)

| Section | Norm % of Total | Typical Words (8pp, ~5000w) | Notes |
|---|---|---|---|
| Abstract | 2-3% | 100-150 | Fixed by venue; often ≤150 words |
| Introduction | 12-18% | 600-900 | Front-load contribution statement |
| Related Work | 8-12% | 400-600 | Can be merged into intro for tight limits |
| Method | 22-30% | 1100-1500 | Core contribution usually lives here |
| Experiments | 25-35% | 1250-1750 | Tables + figures reduce text needed |
| Discussion | 5-10% | 250-500 | Sometimes merged with experiments |
| Conclusion | 3-5% | 150-250 | Brief; no new content |

### Journal Paper (~12-20 pages)

Proportions are similar but with more room for depth:

| Section | Norm % of Total |
|---|---|
| Abstract | 1-2% |
| Introduction | 10-15% |
| Related Work / Background | 12-18% |
| Method | 20-25% |
| Experiments / Results | 25-30% |
| Discussion | 8-12% |
| Conclusion | 3-5% |

### Thesis / Dissertation

| Section | Norm % of Total |
|---|---|
| Introduction | 10-15% |
| Literature Review | 20-30% |
| Methodology | 15-20% |
| Results | 20-25% |
| Discussion | 20-25% |
| Conclusion | 5-10% |

## Reviewer Reading Behavior (Time Allocation)

From empirical studies and Neel Nanda's model:

| Section | % Reviewers Who Read | Time Allocation |
|---|---|---|
| Abstract | 100% | ~25% of total review time |
| Introduction | 90%+ (often skimmed) | ~25% of total review time |
| Figures / Tables | Examined before methods | ~25% of total review time |
| Methods + Experiments + Discussion + Conclusion | Variable | ~25% of total review time |
| Appendix | Rarely consulted | Minimal |

**Implication for cutting/expansion**: invest most effort in Abstract, Introduction,
and Figures. Methods and Results matter for acceptance but are read less thoroughly
on first pass.

## Red Flags in Section Length

### Over-length Signals

- Related work exceeds 15% → likely too much survey, not enough positioning
- Introduction exceeds 20% → probably repeating method details
- Conclusion exceeds 5% → may be restating results instead of synthesizing
- Any single paragraph exceeds 250 words → likely needs splitting

### Under-length Signals

- Experiments below 20% → insufficient evidence for claims
- Method below 15% → may lack reproducibility detail
- Discussion missing entirely → reviewer will flag shallowness
- Introduction below 8% → inadequate motivation and context
