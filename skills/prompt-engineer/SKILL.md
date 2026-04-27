---
name: prompt-engineer
description: |
  Transform vague instructions into structured prompts with explicit role, constraints, and output format.
  Produces copy-ready prompts using patterns like RTF/RISEN/RODES, and internal agent prompts (subagent tasks, system prompts, tool-use prompts) with verified 6-element completeness. Use when the user wants to improve a prompt, create one from a vague idea, or design agent-to-agent communication.
metadata:
  version: "3.0.0"
  platforms: [codex]
  category: automation
  tags:
    - prompt-engineering
    - prompt-rewrite
    - ai-prompts
    - structured-prompting
    - internal-prompt
    - agent-prompt
    - system-prompt
    - tool-use-prompt
    - delegation-prompt
risk: low
source: local
routing_layer: L1
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - prompt engineering
  - 提示词优化
  - structured prompt
  - agent prompt
  - RTF/RISEN/RODES
  - create one from a vague idea
  - design agent-to-agent communication
  - prompt rewrite
  - ai prompts
  - structured prompting
---

# Prompt Engineer

This skill owns prompt transformation: turning vague, weak, or underspecified
input into a clearer and more controllable prompt. Covers both **external
prompts** (user → AI) and **internal prompts** (agent → subagent, agent → tool,
system prompt design).

## When to use

### External prompt scenarios (user → AI)

- The user wants to improve, rewrite, or optimize a prompt
- The user has a vague idea and wants a strong AI prompt generated from it
- The user asks how to ask an AI for better output
- The user wants more structure, clarity, constraints, or output formatting
- Best for requests like:
  - "帮我优化这个 prompt"
  - "给我写一个 AI 提示词"
  - "我该怎么问 AI 才能得到更好的结果"

### Internal prompt scenarios (agent → subagent / tool)

- An agent needs to write a clear task prompt for a subagent
- The user wants to improve how delegation prompts are written
- System prompt design or iteration is needed
- Tool-use prompts need better parameter/constraint specification
- Multi-turn orchestration prompts need structuring
- The user is crafting `spawn_agent` / `send_input` task descriptions
- Best for requests like:
  - "帮我把这个 subagent 任务写得更清楚"
  - "优化一下这个 delegation prompt"
  - "写一个 system prompt 给这个 agent"
  - "这个 tool-use prompt 输出不稳定，帮我改"
  - "agent 给 subagent 的指令太模糊了，帮我加强"

## Do not use

- The task is UI / page-generation prompt strengthening with design-system blocks and page structure -> use `$design-workflow`
- The task is establishing a file-backed design workflow that happens to include prompts -> use `$design-md` first, then `$design-workflow`
- The task is general skill-authoring rather than prompt writing
- The user wants direct task execution, not a prompt to use elsewhere
- The task is domain implementation or debugging rather than prompt design
- The task is deciding **whether** to delegate → use `$subagent-delegation`
- The task is agent architecture or multi-agent system design → use `$agent-swarm-orchestration`
- The task is generic prose editing unrelated to prompts

## Task ownership and boundaries

This skill owns:
- prompt rewriting and generation
- adding structure, constraints, and output format
- choosing an appropriate prompting pattern
- **agent-to-subagent prompt crafting and quality gating**
- **system prompt design and iteration**
- **tool-use prompt structuring**
- **delegation prompt clarity enforcement**
- **multi-turn orchestration prompt design**

This skill does not own:
- executing the domain task instead of writing the prompt
- deciding whether delegation should happen → `$subagent-delegation`
- agent architecture design → `$agent-swarm-orchestration`
- Codex skill-routing diagnosis
- generic prose editing unrelated to prompts

If the task shifts to adjacent skill territory, route to:
- `$skill-framework-developer` for skill-document writing
- `$subagent-delegation` for delegation strategy decisions
- `$agent-swarm-orchestration` for multi-agent system architecture

## Cross-skill integration

With `$subagent-delegation`, this skill improves the **prompt quality** while
delegation still owns **whether / what / when** to delegate.

With `$checklist-planner`, this skill can strengthen prompt specs inside a checklist
without owning the checklist structure itself.

## Required workflow

1. Identify the user's real objective.
2. Determine prompt consumer: human, agent, subagent, or tool.
3. Select the simplest prompt structure that improves clarity.
4. Add role, context, constraints, and output format as needed.
5. For internal prompts: validate against the 6-Element Checklist.
6. Deliver a clean prompt the user can copy directly.

## Output defaults

- Default output: final prompt + optional short usage note
- For internal prompts, also return a compact 6-element audit summary
- Use the copy-ready markdown structures in the references below

## Hard constraints

- Do not explain framework names unless the user asks for the explanation.
- Do not generate a generic prompt when the user gave enough specifics.
- Do not silently change the user's core objective.
- Ask only minimal clarification when truly necessary.
- Make the final prompt directly copyable.

## Supporting references

- [internal-prompt-patterns.md](file:///Users/joe/Documents/skill/skills/prompt-engineer/references/internal-prompt-patterns.md) — reusable internal prompt templates and quality checklist
- [references/authoring-workflow.md](references/authoring-workflow.md) — structure selection, 6-element audit, output formats, iron rules, trigger examples
