# Notation Audit — Detailed Check Tables

## Abbreviation Audit

| # | Check | Pass Criteria |
|---|---|---|
| A1 | First-use expansion | Every abbreviation is written as "Full Name (ABBR)" at its first occurrence in the body text |
| A2 | Abstract independence | Abbreviations used in abstract are re-defined at first use in the body |
| A3 | Consistency | The same concept uses the same abbreviation throughout; no synonym drift |
| A4 | No orphan expansions | Every expanded form has subsequent abbreviated uses; remove expansion if ABBR is never reused |
| A5 | Standard vs custom | Well-known abbreviations (e.g., CNN, GPU, API) noted; custom ones flagged for mandatory expansion |
| A6 | Multilingual handling | In Chinese papers, English abbreviations still require full English expansion at first use; bilingual abbreviation conventions are respected |

## Symbol Audit

| # | Check | Pass Criteria |
|---|---|---|
| S1 | Uniqueness | Each symbol has exactly one meaning throughout the paper |
| S2 | First-definition | Every symbol is defined at or before its first use (inline or in a "where …" block) |
| S3 | Font consistency | Bold/italic/calligraphic/blackboard-bold usage is consistent for the same symbol class (vectors bold, matrices uppercase, sets calligraphic, etc.). See `notation-conventions.md` for standard mappings |
| S4 | Subscript/superscript | Index notation is consistent (e.g., always $x_i$ not sometimes $x_i$ sometimes $x^{(i)}$ for the same meaning) |
| S5 | Pseudocode alignment | Variable names in pseudocode/algorithm blocks match equation symbols exactly |
| S6 | Notation table sync | If a notation table exists, every entry matches body usage and vice versa |
| S7 | Theorem-environment coherence | Symbols introduced in theorem/lemma/definition/corollary environments are consistent with surrounding text and proofs |

## Formula Audit

| # | Check | Pass Criteria |
|---|---|---|
| F1 | Numbering continuity | Equation numbers are sequential with no gaps or duplicates |
| F2 | Cross-reference accuracy | Every "Eq. (N)" reference points to the correct equation |
| F3 | Connector text | Each displayed formula has a lead-in sentence and, when needed, a "where" block defining new variables |
| F4 | Punctuation | Displayed equations have correct trailing punctuation (comma, period) consistent with sentence structure. See `notation-conventions.md` for rules |
| F5 | Multi-line alignment | Aligned/split equations use consistent alignment points |
| F6 | Inline vs display | The same formula is not presented as inline math in one place and display math in another without justification; important formulas that are referenced should be displayed and numbered |

## Unit and Dimension Audit

| # | Check | Pass Criteria |
|---|---|---|
| U1 | Unit presence | Every physical quantity has an explicit unit |
| U2 | Format consistency | Units follow a single convention (SI preferred); consistent use of upright font for units. See `notation-conventions.md` |
| U3 | Dimensional consistency | Both sides of every equation have the same dimensions |
| U4 | Magnitude plausibility | Reported values are within plausible physical ranges |
| U5 | Figure/table alignment | Units in figure axes, table headers, and captions match those used in body text for the same quantities |
