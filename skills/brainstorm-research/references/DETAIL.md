# Brainstorm Research — Detailed Reference

## File-Backed Discipline (adapted from planning-with-files)

When in file-backed exploration mode, use the filesystem as external working memory.

### Read before decide
Before changing the shortlist, reframing the problem, or switching from divergence to convergence, re-read the local brainstorm files.

### Write after every 2 research actions
After every 2 meaningful research actions (searches, paper reads, note reviews, screenshots, web opens, PDF inspections), update the local files.

At minimum:
- new evidence or constraints → `direction_map.md`
- changed shortlist or evaluation logic → `brainstorm_plan.md`
- what you did this round and what changed → `iteration_log.md`

### Never lose pruning logic
If you reject or merge a direction, log:
- what changed
- why
- whether it was rejected, merged, or parked
- what evidence or constraint caused that decision

### Make resumption cheap
A future session should be able to answer these from the local files:
- What is the seed idea?
- What options were generated?
- Which ones are still alive?
- Why were others cut?
- What should be done next?

## Expansion Axes

Force divergence across several research axes instead of staying on one track. Use the axes that fit the input:

- question type: mechanism, prediction, explanation, evaluation, optimization, benchmarking, causal inference
- contribution shape: new task, new dataset, new method, new metric, new theoretical framing, new system, new empirical finding
- method family: experiment, simulation, field study, survey, observational study, qualitative study, modeling, benchmark analysis
- evidence source: public dataset, lab data, literature synthesis, interviews, logs, sensors, web data, synthetic data
- scope level: narrow tractable study, medium publishable slice, long-horizon agenda
- novelty level: safe extension, solid combination, differentiated reframing, contrarian hypothesis
- validation style: ablation, baseline comparison, robustness test, error analysis, user study, cross-domain generalization
- resource bias: solo-doable, compute-light, data-light, theory-heavy, engineering-heavy, collaboration-heavy
- output target: workshop paper, conference paper, journal article, thesis chapter, pilot study, grant seed
- reframe angle: change population, modality, task definition, assumption, unit of analysis, or move from method to application / application to method

Include at least one direction that changes the framing of the research problem, not just the method.

## Direction Packaging

Present directions as discrete bets the user could actually choose between.

For each direction, include:
- short name
- core idea or research question in 1-2 sentences
- novelty axis or reframing axis
- why it is meaningfully different
- required evidence, dataset, or experiment
- upside
- main risk or tradeoff
- cheapest strong next step
- status: `candidate`, `shortlisted`, `parked`, `rejected`, or `merged`

## Convergence Protocol

If the user wants help choosing, do not just recommend one option. First narrow the space explicitly:
- shortlist 2-4 directions
- log why they survived
- log why the others were cut or parked
- identify the cheapest discriminating next experiment / literature check / data check

Store the convergence rationale in both:
- `brainstorm_plan.md` for current direction and open questions
- `iteration_log.md` for what changed in this round

## Local File Responsibilities

### `brainstorm_plan.md`
Owns the stable frame:
- seed restatement
- assumptions
- constraints
- target output
- expansion axes
- current phase
- shortlist criteria
- open questions
- decision summary

### `direction_map.md`
Owns the option space:
- grouped directions
- per-direction comparison fields
- evidence hooks
- feasibility / novelty / risk notes
- current status of each direction

### `iteration_log.md`
Owns the timeline:
- what was explored this round
- directions added, merged, or rejected
- sources consulted
- what changed in the shortlist
- what to do next

## Quality Bar

- Produce options that are orthogonal enough that choosing one excludes or deprioritizes another.
- Avoid filler like "just add deep learning", "use LLM", or "collect more data" unless the move is specific and defensible.
- Avoid collapsing into one polished answer plus a few weak extras.
- Include both incremental variants and non-obvious reframes.
- Favor concrete hypotheses, datasets, task settings, or experiment hooks over abstract labels.
- Keep momentum high. The user came for option space, not a lecture.
- Avoid pretending something is novel without specifying the novelty axis.
- In file-backed mode, the local files should be strong enough that another future session can resume without reconstructing the whole conversation.

## Trigger Examples

- "我有个科研 idea，先帮我发散一下。"
- "这个研究方向还很早期，给我多几个论文路子。"
- "别急着写方案，先用 brainstorm-research 发散研究思路。"
- "给我 10 个完全不同的研究做法。"
- "我只有一个模糊课题，帮我拆成多个可能方向。"
- "别只 brainstorm，顺便把过程整理成本地文件。"
- "把筛选逻辑和 shortlist 也记下来，后面我要继续迭代。"
- "I have a rough research idea. Give me many directions before we commit to a study design."
- "Turn this brainstorm into a local research workspace I can continue later."
