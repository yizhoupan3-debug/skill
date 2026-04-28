# Structured Ideation Frameworks

10 cognitive frameworks for generating research ideas, adapted from Orchestra-Research best practices.

## 1. Problem-First vs Solution-First Thinking

**Problem-First** (pain point → method):
- Start with a concrete failure, bottleneck, or unmet need
- Risk: may converge on incremental fixes

**Solution-First** (new capability → application):
- Start with a new tool or insight seeking application
- Risk: "hammer looking for a nail"

**Self-Check**:
- [ ] Can I name a specific person/community who needs this?
- [ ] Is the problem actually unsolved?
- [ ] If solution-first, does the solution create new capability?

## 2. The Abstraction Ladder

| Direction | Action | Outcome |
|-----------|--------|---------|
| **Move Up** | Turn a specific result into a broader principle | Framework papers |
| **Move Down** | Test a general paradigm under concrete constraints | Empirical papers |
| **Move Sideways** | Apply same abstraction level to adjacent domain | Transfer papers |

**Example**:
- Current: "Improving retrieval accuracy for RAG systems"
- Up: "What makes context selection effective for any augmented generation?"
- Down: "How does retrieval accuracy degrade under adversarial perturbation?"
- Sideways: "Database query optimization uses similar relevance ranking—borrow?"

## 3. Tension and Contradiction Hunting

| Tension Pair | Research Opportunity |
|---|---|
| Performance ↔ Efficiency | Match SOTA with 10x less compute? |
| Privacy ↔ Utility | Close the accuracy gap with federated/encrypted methods? |
| Generality ↔ Specialization | When does fine-tuning beat prompting, and why? |
| Safety ↔ Capability | Can alignment improve rather than tax capability? |
| Interpretability ↔ Performance | Do mechanistic insights enable better architecture? |
| Scale ↔ Accessibility | Can small models replicate emergent behaviors? |

**Workflow**: Pick area → list desiderata → identify trade-off pairs → ask: fundamental or artifact of current methods? → reconciliation IS your contribution.

## 4. Cross-Pollination (Analogy Transfer)

| Source Field | Transferable Concepts |
|---|---|
| Neuroscience | Attention, memory consolidation, hierarchical processing |
| Physics | Energy-based models, phase transitions, renormalization |
| Economics | Mechanism design, auction theory, incentive alignment |
| Ecology | Population dynamics, niche competition, co-evolution |
| Linguistics | Compositionality, pragmatics, grammatical induction |
| Control Theory | Feedback loops, stability, adaptive regulation |

**Requirements**: structural fidelity, non-obvious connection, testable predictions.

## 5. The "What Changed?" Principle

Revisit old problems under new conditions:

| Change Type | Example | Research Implication |
|---|---|---|
| Compute | GPUs 10x faster | Previously dismissed methods become feasible |
| Scale | Trillion-token datasets | Statistical arguments may now hold |
| Regulation | EU AI Act, GDPR | Demand for compliant alternatives |
| Tooling | New frameworks, APIs | Reduced implementation barrier |
| Failure | High-profile system failures | Exposed gaps in existing approaches |

**Workflow**: Pick abandoned approach (3-10 yr old) → list rejection assumptions → ask: still true today? → if not, re-run.

## 6. Failure Analysis and Boundary Probing

**Boundaries to probe**: distributional (OOD), scale (10x/0.1x), adversarial, compositional, temporal (concept drift).

**Workflow**: Select widely-used method → identify implicit evaluation assumptions → systematically violate each → document where/how it breaks → diagnose root cause → propose fix.

## 7. The Simplicity Test

- Can you explain the idea in one sentence to a non-expert?
- Can you remove any component without losing the core contribution?
- Is the simplest version of the idea still interesting?

## 8. Stakeholder Rotation

View the problem from different perspectives: end user, system admin, regulator, adversary, developer, researcher in adjacent field. Each perspective reveals different requirements and constraints.

## 9. Composition and Decomposition

- **Composition**: Combine two simple, well-understood techniques in a novel way
- **Decomposition**: Break a complex system into components and improve one in isolation
- Ask: which component is the bottleneck? What if we replaced just that part?

## 10. The "Explain It to Someone" Test

Explain your idea to an imaginary colleague. Where do you handwave? Where do you say "it just works"? Those gaps are either weaknesses to fix or opportunities to investigate.

---

## Framework Selection Guide

| Situation | Best Framework |
|---|---|
| Have a problem, need solutions | Problem-First (#1) |
| Have a technique, need applications | Solution-First (#1), Cross-Pollination (#4) |
| Field feels stuck | What Changed (#5), Tension Hunting (#3) |
| Want to find novelty | Abstraction Ladder (#2), Boundary Probing (#6) |
| Want rigor check on existing idea | Simplicity Test (#7), Explain It (#10) |
| Want breadth | Cross-Pollination (#4), Stakeholder Rotation (#8) |
