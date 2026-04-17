# Domain-Specific Logic Checklists

This file provides per-domain checklist variants for `paper-logic`.
The main SKILL.md covers the general audit framework. Use the domain-specific
items below when the paper falls into a recognized subfield.

---

## Computer Vision (CV)

### Novelty positioning
- Is the architecture genuinely new, or a re-combination of known blocks?
- Is the method compared to current SOTA on standard benchmarks (ImageNet, COCO, ADE20K)?
- Does the paper show performance on multiple datasets, not just the easiest one?

### Experiment design
- Are data augmentation strategies disclosed and controlled?
- Are training schedules (epochs, LR schedule, warmup) fully specified?
- Are FLOPs / throughput / latency reported alongside accuracy?
- Are pre-training data sources disclosed for foundation models?
- Is transfer learning properly controlled (same backbone, same pre-training)?

### Common reviewer attacks
- "This is just [ResNet/ViT/ConvNeXt] + [minor modification]"
- "Only tested on ImageNet; what about distribution shift?"
- "No comparison with [latest concurrent work on arXiv]"

---

## Natural Language Processing (NLP)

### Novelty positioning
- Does the paper differentiate from prompt engineering vs genuine method innovation?
- Are LLM-based methods compared fairly to fine-tuned baselines?
- Is the evaluation contamination risk discussed (benchmark leakage)?

### Experiment design
- Are multiple LM sizes tested (generalization across scale)?
- Are human evaluation protocols described (inter-annotator agreement)?
- Is the evaluation set truly held-out from training data?
- Are token-level vs document-level metrics distinguished?

### Common reviewer attacks
- "This could be achieved with a better prompt"
- "Not clear if the improvement is from the method or the base model"
- "Human evaluation is insufficiently described"

---

## Reinforcement Learning (RL)

### Novelty positioning
- Is the environment truly novel, or a minor variant of existing benchmarks?
- Does the paper compare against currently competitive algorithms (not just PPO/DQN)?

### Experiment design
- Are learning curves shown (not just final performance)?
- Are at least 10 random seeds used (RL has high variance)?
- Is wall-clock time reported alongside sample efficiency?
- Are hyperparameter sensitivity analyses included?
- Is the environment stochasticity properly controlled?

### Common reviewer attacks
- "Results only on toy environments"
- "No wall-clock time comparison"
- "Hyperparameter sensitivity not explored"

---

## Theoretical Machine Learning

### Method rigor
- Are all assumptions stated explicitly (convexity, smoothness, bounded gradients)?
- Are proofs self-contained in the paper or appendix?
- Do theorem conditions match experimental settings?
- Are lower bounds discussed when claiming optimality?

### Experiment design
- Do experiments verify theoretical predictions (not just show the method works)?
- Are synthetic experiments included to isolate theoretical contributions?
- Are constants in bounds discussed (not just asymptotic behavior)?

### Common reviewer attacks
- "The assumptions are too restrictive for practical use"
- "The gap between theory and practice is not adequately discussed"
- "Similar results exist in [adjacent field]"

---

## Physics-Informed / Scientific ML

### Theoretical depth (specialty checks)
- Are embedded priors (ODE/PDE constraints, symmetries) mathematically derived?
- Is balance between physics and data-driven terms justified?
- Are applicability boundaries of theoretical assumptions discussed?
- Is there "theory washing" (physics component contributes negligibly)?

### Experiment design
- Are results shown across multiple operating conditions (steady/transient/noisy)?
- Is cross-domain generalization tested?
- Are conservation law violations quantified?
- Are results compared against classical numerical methods?

### Common reviewer attacks
- "The physics term doesn't actually matter (ablation shows negligible effect)"
- "Not tested outside training distribution"
- "No comparison with established numerical solvers"

---

## Biomedical / Clinical AI

### Novelty positioning
- Is clinical relevance established (not just ML metric improvement)?
- Are ethical considerations and IRB approval mentioned?
- Is the data source clearly described (prospective vs retrospective)?

### Experiment design
- Is external validation included (different institution, different scanner)?
- Are subgroup analyses performed (demographics, disease severity)?
- Are clinically meaningful metrics reported (sensitivity, specificity at operating points)?
- Is comparison with clinical standard of care included?
- Is data leakage at the patient level audited?

### Common reviewer attacks
- "No external validation"
- "Not clinically deployable at this stage"
- "Missing subgroup analysis for underrepresented populations"
