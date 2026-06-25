#!/usr/bin/env python3
"""
补齐 SKILL_ROUTING_RUNTIME.json 中所有 skill 的 network_access 和 allowed_tools。

用法: python3 scripts/p0-fix-routing-config.py
"""

import json
import copy

JSON_PATH = "skills/SKILL_ROUTING_RUNTIME.json"

with open(JSON_PATH) as f:
    data = json.load(f)

skills = data["skills"]
keys = data["keys"]

SLUG = keys.index("slug")
NA = keys.index("network_access")
AT = keys.index("allowed_tools")

# ── 映射表: slug -> { "network_access": str|None, "allowed_tools": list|None } ──

CONFIG = {
    # === required (核心依赖网络) ===
    "deep-research": {
        "network_access": "required",
        "allowed_tools": ["WebSearch", "WebFetch", "Read", "Bash"],
    },
    "research-discovery": {
        "network_access": "required",
        "allowed_tools": ["WebSearch", "WebFetch", "Read", "Bash"],
    },
    "literature-verification": {
        "network_access": "required",
        "allowed_tools": ["Read", "WebFetch", "WebSearch", "Bash", "Grep"],
    },
    # === conditional (部分功能需网络) ===
    "research-execution": {
        "network_access": "conditional",
        "allowed_tools": ["Read", "Write", "Edit", "Bash", "Grep", "Glob"],
    },
    "citation-management": {
        "network_access": "conditional",
        "allowed_tools": ["Read", "Write", "Edit", "Bash", "Grep"],
    },
    "paper-writing": {
        "network_access": "conditional",
        "allowed_tools": ["Read", "Write", "Edit", "Bash", "Grep"],
    },
    "statistical-analysis": {
        "network_access": "conditional",
        "allowed_tools": ["Read", "Write", "Edit", "Bash", "Grep"],
    },
    "mcp-server-management": {
        "network_access": "conditional",
        "allowed_tools": ["Read", "Write", "Edit", "Bash", "Grep", "Glob"],
    },
    "algo-trading": {
        "network_access": "conditional",
        "allowed_tools": ["Read", "Write", "Edit", "Bash", "Grep", "Glob"],
    },
    "scientific-figure-plotting": {
        "network_access": "conditional",
        "allowed_tools": ["Read", "Write", "Edit", "Bash", "Grep", "Glob"],
    },
    "python-env-management": {
        "network_access": "conditional",
        "allowed_tools": ["Read", "Bash", "Grep"],
    },
    "autoresearch": {
        "network_access": "conditional",
        "allowed_tools": ["Read", "Write", "Edit", "Bash", "Grep"],
    },
    "agent-swarm-orchestration": {
        "network_access": "conditional",
        "allowed_tools": ["Agent", "Read", "Bash", "Grep"],
    },
    # === local (纯本地操作) ===
    "formal-verification": {
        "network_access": "local",
        "allowed_tools": ["Read", "Bash", "Grep"],
    },
    "statistical-verification": {
        "network_access": "local",
        "allowed_tools": ["Read", "Bash", "Grep"],
    },
    "prose-verification": {
        "network_access": "local",
        "allowed_tools": ["Read", "Bash", "Grep"],
    },
    "structure-verification": {
        "network_access": "local",
        "allowed_tools": ["Read", "Bash", "Grep"],
    },
    "reproducibility-verification": {
        "network_access": "local",
        "allowed_tools": ["Read", "Bash", "Grep"],
    },
    "experiment-reproducibility": {
        "network_access": "local",
        "allowed_tools": ["Read", "Write", "Edit", "Bash", "Grep", "Glob"],
    },
    "math-derivation": {
        "network_access": "local",
        "allowed_tools": ["Read", "Bash", "Grep"],
    },
    "plan-mode": {
        "network_access": "local",
        "allowed_tools": ["Read", "Write", "Edit", "Bash", "Grep"],
    },
    "infographic": {
        "network_access": "local",
        "allowed_tools": ["Read", "Write", "Edit", "Bash"],
    },
    "diagramming": {
        "network_access": "local",
        "allowed_tools": ["Read", "Write", "Edit", "Bash"],
    },
    "email-template": {
        "network_access": "local",
        "allowed_tools": ["Read", "Write", "Edit", "Bash"],
    },
    "research-knowledge-graph": {
        "network_access": "local",
        "allowed_tools": ["Read", "Bash", "Grep"],
    },
    "spreadsheets": {
        "network_access": "local",
        "allowed_tools": ["Read", "Write", "Edit", "Bash", "Grep"],
    },
    # === 已有 allowed_tools, 仅缺 network_access ===
    "code-review-deep": {
        "network_access": "local",
        "allowed_tools": None,  # keep existing
    },
    "deepinterview": {
        "network_access": "local",
        "allowed_tools": None,
    },
    "simplify": {
        "network_access": "local",
        "allowed_tools": None,
    },
    "tikz-paper-figure": {
        "network_access": "local",
        "allowed_tools": None,
    },
    "goalx": {
        "network_access": "local",
        "allowed_tools": None,
    },
}

# ── 应用修改 ──

before_na = sum(1 for s in skills if s[NA] is None)
before_at = sum(1 for s in skills if s[AT] is None)

for s in skills:
    slug = s[SLUG]
    cfg = CONFIG.get(slug)
    if cfg is None:
        continue
    if cfg["network_access"] is not None and s[NA] is None:
        s[NA] = cfg["network_access"]
    if cfg.get("allowed_tools") is not None and s[AT] is None:
        s[AT] = cfg["allowed_tools"]

after_na = sum(1 for s in skills if s[NA] is None)
after_at = sum(1 for s in skills if s[AT] is None)

# ── 写回 ──

with open(JSON_PATH, "w") as f:
    json.dump(data, f, indent=2, ensure_ascii=False)
    f.write("\n")

# ── 报告 ──

print(f"=== P0 修复报告 ===")
print(f"network_access:  {before_na}/44 missing → {after_na}/44 missing")
print(f"allowed_tools:   {before_at}/44 missing → {after_at}/44 missing")
print(f"文件已写入: {JSON_PATH}")

# 列出仍然缺失的
if after_na > 0:
    missing = [s[SLUG] for s in skills if s[NA] is None]
    print(f"仍缺 network_access: {missing}")
if after_at > 0:
    missing = [s[SLUG] for s in skills if s[AT] is None]
    print(f"仍缺 allowed_tools: {missing}")

if after_na == 0 and after_at == 0:
    print("✅ 全部补齐！")
else:
    print("⚠️ 尚有缺失未补齐")
