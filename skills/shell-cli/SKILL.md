---
name: shell-cli
description: |
  Produce safe, portable shell commands, pipelines, and scripts that handle
  quoting, glob-safety, and GNU/BSD differences correctly. Delivers dry-
  runable solutions with preview before destructive operations. Use when the
  user asks for shell commands, bash/zsh scripts, batch file processing, CLI
  automation, or phrases like "写个 shell 脚本", "一行命令处理", "批量改文件",
  "命令行自动化".
metadata:
  version: "1.1.0"
  platforms: [codex]
  tags:
    - shell
    - bash
    - zsh
    - cli
    - automation
risk: medium
source: local
routing_layer: L2
routing_owner: owner
routing_gate: none
session_start: n/a
trigger_hints:
  - 写个 shell 脚本
  - 一行命令处理
  - 批量改文件
  - 命令行自动化
  - shell commands
  - bash
  - zsh scripts
  - batch file processing
  - CLI automation
  - shell
---

# shell-cli

This skill owns shell-first terminal work: commands, pipelines, scripts, and CLI ergonomics that should not be forced into a Python or Git skill.

## When to use

- The user wants a shell command, one-liner, or reusable shell script
- The task involves bash, zsh, POSIX shell, shell startup files, aliases, or functions
- The task involves Unix pipelines with tools such as `find`, `grep`, `sed`, `awk`, `xargs`, `jq`, `cut`, `sort`, or `uniq`
- The user wants batch file processing, bulk rename, text extraction, environment setup, or terminal automation
- Best for requests like:
  - "写个 shell 脚本批量处理这些文件"
  - "帮我用 bash 一行命令筛出重复项"
  - "zsh 配置为什么没生效"
  - "给我一个可直接跑的命令行方案"

## Do not use

- The main task is Git history, branching, remotes, or publishing workflows → use `$git-workflow`
- The best implementation should clearly be a Python program rather than shell automation → use `$python-pro`
- The main task is browser automation rather than terminal automation → use `$playwright`
- The task is primarily root-cause debugging of a broader software failure rather than shell work → use `$systematic-debugging`

## Task ownership and boundaries

This skill owns:
- shell commands and one-liners
- bash/zsh scripts and shell startup configuration
- Unix text-processing pipelines
- file-system batch operations driven from the terminal
- CLI tool composition and safe command sequencing

This skill does not own:
- Git collaboration workflows
- Python application design
- browser/UI automation
- non-shell system administration beyond the scoped request

If the task shifts to adjacent skill territory, route to:
- `$git-workflow`
- `$python-pro`
- `$playwright`
- `$systematic-debugging`

## Required workflow

1. Confirm the task shape:
   - object: files, text streams, env vars, configs, directories, CLI output
   - action: extract, transform, batch edit, automate, debug, configure
   - constraints: shell type, OS compatibility, safety requirements, input size
   - deliverable: one-liner, script, config snippet, explanation, or fix
2. Prefer the smallest safe solution that fits the job.
3. For destructive or bulk operations, design a dry-run or preview step first.
4. Quote paths and handle spaces safely.
5. Validate the command on representative input.

## Core workflow

### 1. Intake
- Determine whether the user needs a one-liner, reusable script, shell config help, or command debugging.
- Check shell/runtime assumptions such as `bash`, `zsh`, POSIX `sh`, macOS vs GNU tools, and required external CLIs.

### 2. Execution
- Choose the minimal toolchain that solves the task cleanly.
- Prefer readable pipelines over clever but fragile command golf.
- When editing files in bulk, show selection first, write step second, and verification third.
- When portability matters, call out GNU/BSD differences explicitly.

### 3. Validation / recheck
- Re-run the command against sample input or a narrowed file set.
- Confirm exit behavior, quoting, glob handling, and recursion scope.
- If the command relies on non-default tools, say so explicitly.

## Output defaults

Default output should contain:
- the final command or script
- what it does and any assumptions
- how to preview or verify before broad execution

Recommended structure:

````markdown
## Shell Solution
- Goal: ...
- Form: one-liner / script / config change

## Command or Script
```bash
...
```

## Notes
- Assumptions: ...
- Verify with: ...
- Risks: ...
````

## Hard constraints

- Do not suggest broad destructive commands without a scoped preview path.
- In this repository, follow [`RTK.md`](/Users/joe/Documents/skill/RTK.md) for repo-local RTK usage rules.
- When generating commands that produce large output (e.g., `git status`, `cargo test`, `npm test`), PREFER the `rtk` form when `RTK.md` says compression is appropriate, unless raw output is explicitly requested.
- Do not leave paths unquoted when spaces or special characters are plausible.
- Do not assume GNU-specific flags on macOS without saying so.
- Prefer explicit file selection over recursive wildcard commands when the blast radius matters.
- If safety depends on a dry run, include it.

## Trigger examples

- "Use $shell-cli to write a safe batch-rename script."
- "帮我用 zsh/bash 批量改文件名。"
- "写一个命令把目录里的 JSON 都提取某个字段。"
