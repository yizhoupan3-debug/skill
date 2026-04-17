# Prompt Engineer — Authoring Workflow

Use this reference when the main `SKILL.md` is not enough and you need the
detailed prompt-construction rules, audit checklist, or output formats.

## Intake

Determine:

- rewrite existing prompt vs create new prompt
- prompt consumer: user / agent / subagent / tool
- target use case
- desired output format

## Structure selection

### External prompts

Use the lightest structure that works:

- `RTF`
- `RACE`
- `STAR`
- `RISEN`
- `RODES`

### Internal prompts

Pick the matching pattern:

- explorer prompt
- worker prompt
- reviewer prompt
- tool-use prompt
- system prompt
- multi-turn orchestration prompt

See `internal-prompt-patterns.md` for templates.

## Internal prompt 6-element audit

Every agent-internal prompt should make these explicit:

1. **Goal**
2. **Boundary**
3. **Context**
4. **Output contract**
5. **Acceptance**
6. **Coordination**

If any are vague, strengthen before finalizing.

## Output formats

### Default

````markdown
## Optimized Prompt
```text
...
```

## Notes
- ...
````

### Internal prompt variant

````markdown
## Optimized Prompt
```text
...
```

## 6-Element Audit
| Element | Status | Detail |
|---|---|---|
| Goal | ✅ | ... |
| Boundary | ✅ | ... |
| Context | ✅ | ... |
| Output | ✅ | ... |
| Acceptance | ✅ | ... |
| Coordination | ⬜ N/A | ... |

## Notes
- ...
````

## Internal prompt iron rules

- subagent prompts must have an output contract
- system prompts must have a role anchor
- delegation prompts must not rely on implicit context
- tool-use prompts must include error handling
- never send a subagent prompt that only says “do X”

## Trigger examples

### External

- “帮我优化这个 prompt”
- “给我写一个 AI 提示词”
- “Create a strong prompt for asking an AI to design a study plan.”

### Internal

- “帮我把这个 subagent 任务 prompt 写得更清楚。”
- “优化一下 delegation prompt，让 subagent 输出更稳定。”
- “给这个 agent 写一个 system prompt。”
- “这个 spawn_agent 的任务描述太模糊，帮我加强。”
