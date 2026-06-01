# Paper Skill Severity Spec (Shared)

Paper review and revision modes (`paper-reviewer`, inline revision, logic mode,
notation sweep) use this severity system.

## Severity Levels

| Level | Label | Meaning | Reviewer Impact |
|-------|-------|---------|-----------------|
| **P0** | 一票否决 | Fatal: data integrity, academic honesty, hard theoretical errors, plagiarism | Immediate reject |
| **A** | 核心硬伤 | Core flaw in logic, method, or evidence that undermines the main contribution | Likely reject or major revision |
| **B** | 需补充 | Missing data, experiments, baselines, or statistical validation needed | Weakens paper, fixable |
| **Warning** | 隐晦警告 | Subtle omission, unstated boundary, or likely reader misread — based on real material | Does not alone veto; must be listed in exhaustive mode |
| **C** | 打磨 | Writing polish, style, notation cosmetics, minor inconsistencies | Does not affect acceptance decision |

## Usage Rules

1. **P0 stops everything**: if any P0 is found, it must be reported immediately
   and no "ready" verdict can be given
2. **A requires action**: cannot be dismissed as "minor" or "could be improved"
3. **B is evidence-gated**: items that need new experiments or data, not just text edits
4. **Warning must be listed in exhaustive mode**: may co-tag with B or C (`B + Warning`)
   when a fixable gap also has an unstated boundary risk
5. **C is optional for acceptance**: nice-to-have but not blocking
6. **`audit_depth: exhaustive`**: do **not** truncate findings to "top 3" or "top blockers";
   list all material issues by dimension (severity-sorted within each dimension)
7. **`audit_depth: compact`**: verdict + top blockers summary is allowed

## Cross-Skill Consistency

- `paper-reviewer` discovers and classifies issues using this scheme
- `paper-reviewer` logic mode uses this scheme for logic audit findings
- `paper-reviewer` notation sweep uses this scheme for notation issues
- `paper-workbench` inline revision receives issues tagged with this scheme and fixes by priority order
- `paper-writing` output uses severity when multiple text issues are found

## Mapping to Common Alternative Schemes

| This Spec | Reviewer Score (1-10) | OpenReview | Severity Keywords |
|-----------|----------------------|------------|-------------------|
| P0 | 1-2 | Strong Reject | fatal, integrity, plagiarism |
| A | 3-4 | Reject / Borderline | core flaw, missing contribution |
| B | 5-6 | Weak Accept | needs data, missing baseline |
| Warning | — | Minor concern / clarity | subtle, boundary, misread risk |
| C | 7+ | Accept | polish, cosmetic |
