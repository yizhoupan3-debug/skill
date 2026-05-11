# Cursor 跨工作区：操作核对清单（可复制）

本清单对应 README「其它仓库一键接入」「建议自检命令序列」。**其它机器上的目标仓库**须由操作者在本地执行；CI 与本代理无法代替。

**相关**：科研类 skill / hook 真源与 `REVIEW_GATE` 分层速查见 [`research_skills_hooks_survey.md`](research_skills_hooks_survey.md)；宿主接入叙事见 [`host_adapter_contract.md`](../host_adapter_contract.md)。

## 0. 框架根 `FW` 与 `router-rs`（本仓库已自检）

```bash
export FW=/Users/joe/Documents/skill   # 换成你的框架检出根
command -v router-rs && router-rs --help | head -n 1
# 若未安装：cargo install --path "$FW/scripts/router-rs"
```

## 1. 对每个需在 Cursor 全能力使用的目标仓库根（operator-run）

```bash
cd /abs/path/to/your-other-repo
"$FW/scripts/cursor-bootstrap-framework.sh" --framework-root "$FW" --with-cursor-rules --with-configs
python3 -m json.tool .cursor/hooks.json > /dev/null
test -L skills && test -L AGENTS.md && echo "symlinks ok"
# 可选：与模板对照（README L191）
# cmp .cursor/hooks.json "$FW/configs/framework/cursor-hooks.workspace-template.json" && echo "hooks match workspace template"
```

## 2. Cursor 打开方式（operator-run）

在 Cursor 中「打开文件夹」选**目标项目根**（含 `.cursor/hooks.json`），勿只打开子目录。

## 3. Hook 管道自检（目标根目录）

```bash
cd /abs/path/to/your-other-repo
printf '{}' | router-rs cursor hook --event=SessionStart --repo-root "$(pwd)"
```

## 4.（可选）shell profile：`SKILL_FRAMEWORK_ROOT` / `ROUTER_RS_BIN`

```bash
# ~/.zshrc 等；示例：
# export SKILL_FRAMEWORK_ROOT=/abs/path/to/skill
# export ROUTER_RS_BIN=/abs/path/to/router-rs   # 若可执行名非默认
SKILL_FRAMEWORK_ROOT="$FW" router-rs framework maint verify-cursor-hooks
# 注意：该校验的是框架仓 $FW 内 embedded hooks，不是目标仓模板。
```

## 5. Hooks 模板 vs 本仓库 embedded（事件键）

在本仓库根执行：

```bash
jq -r '.hooks|keys[]' .cursor/hooks.json | sort > /tmp/h_emb.txt
jq -r '.hooks|keys[]' configs/framework/cursor-hooks.workspace-template.json | sort > /tmp/h_tpl.txt
diff /tmp/h_emb.txt /tmp/h_tpl.txt && echo "hook event keys match"
```

**结果**：除 command 中 `router-rs` 路径形态外，事件键集合一致（均为：afterFileEdit、afterShellExecution、beforeShellExecution、beforeSubmitPrompt、postToolUse、preCompact、sessionEnd、sessionStart、stop、subagentStart、subagentStop）。

## 6. Bootstrap 脚本语法

```bash
bash -n scripts/cursor-bootstrap-framework.sh && echo ok
```

## 7. 临时目录 bootstrap + hook smoke（本仓库运行记录）

```bash
TMP=$(mktemp -d)
FW=/Users/joe/Documents/skill
cd "$TMP"
"$FW/scripts/cursor-bootstrap-framework.sh" --framework-root "$FW"
R="$FW/scripts/router-rs/target/release/router-rs"
printf '{}' | "$R" cursor hook --event=SessionStart --repo-root "$TMP" >/dev/null; echo SessionStart=$?
printf '{}' | "$R" cursor hook --event=stop --repo-root "$TMP" >/dev/null; echo stop=$?
printf '{}' | "$R" cursor hook --event=beforeSubmitPrompt --repo-root "$TMP" >/dev/null; echo beforeSubmitPrompt=$?
rm -rf "$TMP"
```

**记录**：SessionStart=0，stop=0，beforeSubmitPrompt=0；stdout 为 JSON。

## 8. Runtime：`SKILL_PLUGIN_CATALOG` 中断言 `cursor`（bootstrap 后）

目标仓经软链读取框架生成物时：

```bash
python3 - <<'PY'
import json, sys
with open("skills/SKILL_PLUGIN_CATALOG.json") as f:
    d = json.load(f)
pl = d["skills"]["autopilot"]["host_support"]["platforms"]
assert "cursor" in pl and "claude-code" in pl, pl
print("ok", pl)
PY
```

## 9. `/gitx plan` 收口

按 `skills/gitx/SKILL.md`，在**宿主对话**中执行 **`/gitx plan`** 对照计划；本代理环境无法代为执行该宿主命令。
