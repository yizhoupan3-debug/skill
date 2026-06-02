#!/usr/bin/env python3
"""
Execute all cross-validated recommendations from SKILL_ECOSYSTEM_AUDIT_FINAL.md
P0 (critical) + P1 (important) changes.
"""

import json
import os
import re
import shutil
import sys
from pathlib import Path

ROOT = Path("/Users/joe/Developer/skill")
os.chdir(ROOT)

# ═══════════════════════════════════════════════════
# P0-2 + P0-6: Remove 6 expired + 1 to-archive from MANIFEST
# ═══════════════════════════════════════════════════
REMOVE_SLUGS = {
    "ppt-beamer",              # broken route (archived)
    "source-slide-formats",    # broken route (archived)
    "token-optimization",      # broken route (archived)
    "jupyter-notebook",        # archived, still in MANIFEST
    "latex-compile-acceleration",  # archived, still in MANIFEST
    "mac-memory-management",   # archived, still in MANIFEST
    "adversarial-loop",        # hook injection removed, dead
}

def patch_manifest():
    path = ROOT / "skills/SKILL_MANIFEST.json"
    data = json.loads(path.read_text())
    keys = data["keys"]
    slug_idx = keys.index("slug")

    before = len(data["skills"])
    data["skills"] = [s for s in data["skills"] if s[slug_idx] not in REMOVE_SLUGS]
    after = len(data["skills"])
    print(f"[P0-2/P0-6] MANIFEST: {before} -> {after} skills (removed {before - after})")

    # P1-3: Unify routing_layer declarations
    # gitx: L0 -> L2 (SKILL.md says L2)
    # deepinterview: L0 -> L1 (SKILL.md says L1)
    layer_idx = keys.index("layer")
    for s in data["skills"]:
        if s[slug_idx] == "gitx":
            old = s[layer_idx]
            s[layer_idx] = "L2"
            print(f"[P1-3] gitx layer: {old} -> L2")
        elif s[slug_idx] == "deepinterview":
            old = s[layer_idx]
            s[layer_idx] = "L1"
            print(f"[P1-3] deepinterview layer: {old} -> L1")

    # P1-4: Sync update triggers from SKILL.md
    trigger_idx = keys.index("trigger_hints")
    for s in data["skills"]:
        if s[slug_idx] == "update":
            old_triggers = s[trigger_idx]
            s[trigger_idx] = [
                "/update", "一口气更新", "更新关键文档", "科研文档更新",
                "刷新文档", "扫描文档", "git 跟踪文件", "git tracking",
                "死代码清理", "旧文档清理", "stale files", "dead code",
                "registry 更新", "同步投影"
            ]
            print(f"[P1-4] update triggers: {len(old_triggers)} -> {len(s[trigger_idx])}")

    # P1-5: Resolve gh-address-comments vs gh-fix-ci trigger conflicts
    for s in data["skills"]:
        if s[slug_idx] == "gh-address-comments":
            old = list(s[trigger_idx])
            s[trigger_idx] = [
                "/gh-address-comments", "address comments", "PR review summary",
                "pull request summary", "PR comments", "review comments",
                "PR 评论回复", "reviewer 意见处理", "review feedback",
                "address PR feedback"
            ]
            print(f"[P1-5] gh-address-comments triggers: {len(old)} -> {len(s[trigger_idx])}")
        elif s[slug_idx] == "gh-fix-ci":
            old = list(s[trigger_idx])
            s[trigger_idx] = [
                "/gh-fix-ci", "fix ci", "ci failed", "ci failure",
                "github actions", "CI 失败排查", "CI 修复", "workflow 失败",
                "fix build", "green ci", "ci broken"
            ]
            print(f"[P1-5] gh-fix-ci triggers: {len(old)} -> {len(s[trigger_idx])}")

    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
    print(f"[P0-2] MANIFEST written: {path}")

# ═══════════════════════════════════════════════════
# P0-1: Fix broken links in AGENTS.md and paper-workbench
# ═══════════════════════════════════════════════════
def fix_broken_links():
    # Fix AGENTS.md
    agents_path = ROOT / "AGENTS.md"
    if agents_path.exists():
        content = agents_path.read_text()
        # Replace paper-writing reference with paper-workbench local reference
        orig = content
        content = content.replace(
            "skills/paper-writing/references/prose-quality-gate.md",
            "skills/paper-workbench/references/prose-quality-gate.md"
        )
        if content != orig:
            agents_path.write_text(content)
            print("[P0-1] AGENTS.md: fixed paper-writing -> paper-workbench reference")
        else:
            print("[P0-1] AGENTS.md: no paper-writing references found (may already be fixed)")

    # Fix paper-workbench/SKILL.md
    pwb_path = ROOT / "skills/paper-workbench/SKILL.md"
    if pwb_path.exists():
        content = pwb_path.read_text()
        orig = content
        # Replace ../paper-writing/ references with local references/
        content = content.replace("../paper-writing/references/prose-quality-gate.md",
                                   "references/prose-quality-gate.md")
        content = content.replace("../paper-writing/", "references/")
        # Replace $paper-reviewer inline references with @lane:reviewer
        content = content.replace("$paper-reviewer", "@lane:reviewer")
        # Replace $paper-writing inline references with @lane:writer
        content = content.replace("$paper-writing", "@lane:writer")
        if content != orig:
            pwb_path.write_text(content)
            print("[P0-1/P1-1] paper-workbench/SKILL.md: fixed broken links and archived refs")
        else:
            print("[P0-1] paper-workbench/SKILL.md: no broken links found")

# ═══════════════════════════════════════════════════
# P0-4: Fix Rust routing test hardcodes
# ═══════════════════════════════════════════════════
def fix_routing_tests():
    test_path = ROOT / "core/router-rs/tests/main_tests/routing_tests.rs"
    if not test_path.exists():
        print("[P0-4] routing_tests.rs not found, skipping")
        return
    content = test_path.read_text()
    orig = content
    # Replace paper-reviewer with paper-workbench in test assertions
    content = content.replace('"paper-reviewer"', '"paper-workbench"')
    # Also fix any spreadsheets references if they're archived
    if content != orig:
        test_path.write_text(content)
        print("[P0-4] routing_tests.rs: replaced paper-reviewer -> paper-workbench")
    else:
        print("[P0-4] routing_tests.rs: no paper-reviewer references found")

# ═══════════════════════════════════════════════════
# P0-5: Update FRAMEWORK_SURFACE_POLICY.json
# ═══════════════════════════════════════════════════
def fix_surface_policy():
    path = ROOT / "configs/framework/FRAMEWORK_SURFACE_POLICY.json"
    if not path.exists():
        print("[P0-5] FRAMEWORK_SURFACE_POLICY.json not found, skipping")
        return
    data = json.loads(path.read_text())
    orig = json.dumps(data)
    if "hot_first_turn_owners" in data:
        before = len(data["hot_first_turn_owners"])
        data["hot_first_turn_owners"] = [
            s for s in data["hot_first_turn_owners"]
            if s not in REMOVE_SLUGS and s != "paper-writing"
        ]
        after = len(data["hot_first_turn_owners"])
        if before != after:
            print(f"[P0-5] hot_first_turn_owners: {before} -> {after} (removed archived)")
    if json.dumps(data) != orig:
        path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
        print(f"[P0-5] FRAMEWORK_SURFACE_POLICY.json updated")
    else:
        print("[P0-5] FRAMEWORK_SURFACE_POLICY.json: no changes needed")

# ═══════════════════════════════════════════════════
# P0-6: Move adversarial-loop to .archive-cold
# ═══════════════════════════════════════════════════
def archive_adversarial_loop():
    src = ROOT / "skills/adversarial-loop"
    dst = ROOT / "skills/.archive-cold/adversarial-loop"
    if src.exists() and src.is_dir():
        if dst.exists():
            shutil.rmtree(dst)
        shutil.move(str(src), str(dst))
        print("[P0-6] Moved adversarial-loop -> .archive-cold/")
    elif dst.exists():
        print("[P0-6] adversarial-loop already in .archive-cold/")
    else:
        print("[P0-6] adversarial-loop not found, skipping")

# ═══════════════════════════════════════════════════
# P1-1: Clean archived skill references in remaining skills
# ═══════════════════════════════════════════════════
REFERENCE_REPLACEMENTS = {
    # skill file -> [(old_pattern, new_pattern)]
    "skills/math-derivation/SKILL.md": [
        ("$paper-reviewer", "@lane:reviewer"),
        ("$latex-compile-acceleration", "（内联处理 LaTeX 编译）"),
    ],
    "skills/statistical-analysis/SKILL.md": [
        ("$paper-reviewer", "@lane:reviewer"),
        ("$paper-writing", "@lane:writer"),
        ("$mac-memory-management", ""),
    ],
    "skills/citation-management/SKILL.md": [
        ("$paper-reviewer", "@lane:reviewer"),
        ("$paper-writing", "@lane:writer"),
    ],
    "skills/experiment-reproducibility/SKILL.md": [
        ("$paper-writing", "@lane:writer"),
        ("$mac-memory-management", ""),
    ],
    "skills/python-env-management/SKILL.md": [
        ("$mac-memory-management", ""),
        ("$jupyter-notebook", ""),
    ],
    "skills/design-md/SKILL.md": [
        ("$ppt-beamer", "（已归档，内联处理）"),
        ("$source-slide-formats", "（已归档，内联处理）"),
    ],
    "skills/agent-swarm-orchestration/SKILL.md": [
        ("adversarial-loop/SKILL.md", ".archive-cold/adversarial-loop/SKILL.md"),
    ],
}

def clean_archived_refs():
    for rel_path, replacements in REFERENCE_REPLACEMENTS.items():
        path = ROOT / rel_path
        if not path.exists():
            print(f"[P1-1] {rel_path}: not found, skipping")
            continue
        content = path.read_text()
        orig = content
        for old, new in replacements:
            count = content.count(old)
            if count > 0:
                content = content.replace(old, new)
                print(f"[P1-1] {rel_path}: replaced '{old}' x{count} -> '{new}'")
        if content != orig:
            path.write_text(content)

# ═══════════════════════════════════════════════════
# P1-2: Clean SKILL_ROUTING_LAYERS.md
# ═══════════════════════════════════════════════════
def clean_routing_layers():
    path = ROOT / "skills/SKILL_ROUTING_LAYERS.md"
    if not path.exists():
        print("[P1-2] SKILL_ROUTING_LAYERS.md not found, skipping")
        return
    content = path.read_text()
    orig = content
    # Remove references to archived slugs
    archived_slugs = [
        "adversarial-loop", "paper-writing", "paper-reviewer", "paper-reviser",
        "jupyter-notebook", "ppt-beamer", "source-slide-formats",
        "token-optimization", "latex-compile-acceleration", "mac-memory-management"
    ]
    for slug in archived_slugs:
        # Remove lines referencing these slugs
        lines = content.split("\n")
        lines = [l for l in lines if slug not in l]
        content = "\n".join(lines)
    if content != orig:
        path.write_text(content)
        print("[P1-2] SKILL_ROUTING_LAYERS.md: cleaned archived slug references")
    else:
        print("[P1-2] SKILL_ROUTING_LAYERS.md: no archived references found")

# ═══════════════════════════════════════════════════
# P1-6: Remove empty config files
# ═══════════════════════════════════════════════════
def remove_empty_configs():
    empty_files = [
        "skills/SKILL_HEALTH_MANIFEST.json",
        "skills/SKILL_APPROVAL_POLICY.json",
        "skills/SKILL_ROUTING_RUNTIME_EXPLAIN.json",
    ]
    for rel_path in empty_files:
        path = ROOT / rel_path
        if path.exists():
            data = json.loads(path.read_text())
            # Check if it's effectively empty
            values = [v for k, v in data.items() if k not in ("schema_version", "source_of_truth", "generated_at")]
            if all(not v or v == {} for v in values):
                path.unlink()
                print(f"[P1-6] Removed empty config: {rel_path}")
            else:
                print(f"[P1-6] {rel_path}: has content, keeping")
        else:
            print(f"[P1-6] {rel_path}: not found")

# ═══════════════════════════════════════════════════
# P1-7: Add exit criteria to 6 high-frequency skills
# ═══════════════════════════════════════════════════
EXIT_CRITERIA = {
    "skills/paper-workbench/SKILL.md": """
## Exit Criteria

- verdict 已输出（accept / revise / reject + claim-evidence ladder 完整）
- edit_scope 已门控（仅限 declared scope 内的文件）
- 用户已确认下一步动作（提交修改 / 补充实验 / 放弃）
""",
    "skills/gh-address-comments/SKILL.md": """
## Exit Criteria

- 所有 reviewer comments 已逐条回复（resolved 或 replied）
- PR 状态已更新（comment 已 post 或 commit 已 push）
- 用户确认回复策略（直接修改 / 解释说明 / 标记 wontfix）
""",
    "skills/gh-fix-ci/SKILL.md": """
## Exit Criteria

- CI 状态从红变绿（或已确认为 flaky/infra issue）
- 失败原因已根因分析并记录
- fix 已 push 并通过 CI 验证
""",
    "skills/slides/SKILL.md": """
## Exit Criteria

- deck.plan.json 已生成（含 slide 列表 + 布局决策）
- 美学门控已通过（一致性 / 可读性 / 品牌匹配）
- 用户确认输出格式（LaTeX Beamer / HTML / Markdown）
""",
    "skills/mcp-server-management/SKILL.md": """
## Exit Criteria

- MCP 服务器可启动（进程存活 + 无 crash）
- 工具可调用（至少一个 tool 返回正常响应）
- 配置已持久化到 settings.json
""",
    "skills/skill-framework-developer/SKILL.md": """
## Exit Criteria

- MANIFEST/RUNTIME 已同步（framework skills validate 通过）
- 路由测试通过（cargo test --test routing_tests）
- 新增/修改的 skill 可被路由命中
""",
}

def add_exit_criteria():
    for rel_path, criteria in EXIT_CRITERIA.items():
        path = ROOT / rel_path
        if not path.exists():
            print(f"[P1-7] {rel_path}: not found, skipping")
            continue
        content = path.read_text()
        if "Exit Criteria" in content or "退出标准" in content:
            print(f"[P1-7] {rel_path}: already has exit criteria")
            continue
        # Append before the last line or at the end
        content = content.rstrip() + "\n" + criteria
        path.write_text(content)
        print(f"[P1-7] {rel_path}: added exit criteria")

# ═══════════════════════════════════════════════════
# P1-8: Mark old optimization plan as superseded
# ═══════════════════════════════════════════════════
def mark_superseded():
    path = ROOT / "docs/SKILL_OPTIMIZATION_PLAN.md"
    if not path.exists():
        print("[P1-8] docs/SKILL_OPTIMIZATION_PLAN.md not found, skipping")
        return
    content = path.read_text()
    if "superseded" in content.lower():
        print("[P1-8] Already marked as superseded")
        return
    marker = (
        "> **SUPERSEDED** — This plan has been superseded by "
        "`artifacts/current/SKILL_ECOSYSTEM_AUDIT_FINAL.md` (2026-06-03). "
        "Refer to the new audit report for the current optimization roadmap.\n\n"
    )
    path.write_text(marker + content)
    print("[P1-8] Marked docs/SKILL_OPTIMIZATION_PLAN.md as superseded")

# ═══════════════════════════════════════════════════
# P2-2: Trigger word governance (targeted fixes)
# ═══════════════════════════════════════════════════
def fix_trigger_governance():
    path = ROOT / "skills/SKILL_MANIFEST.json"
    data = json.loads(path.read_text())
    keys = data["keys"]
    slug_idx = keys.index("slug")
    trigger_idx = keys.index("trigger_hints")

    for s in data["skills"]:
        slug = s[slug_idx]
        triggers = list(s[trigger_idx])

        if slug == "doc":
            # Remove overly broad 'word'
            if "word" in triggers:
                triggers.remove("word")
                triggers.extend(["word document", "docx"])
                print(f"[P2-2] doc: removed 'word', added 'word document', 'docx'")
        elif slug == "slides":
            # Remove internal framework term
            if "artifact tool" in triggers:
                triggers.remove("artifact tool")
                print(f"[P2-2] slides: removed 'artifact tool'")
        elif slug == "pdf":
            # Add natural language triggers
            for t in ["编辑 PDF", "合并 PDF", "PDF 转文字"]:
                if t not in triggers:
                    triggers.append(t)
            print(f"[P2-2] pdf: added Chinese natural language triggers")
        elif slug == "deepinterview":
            for t in ["深度采访", "先问清楚", "澄清需求"]:
                if t not in triggers:
                    triggers.append(t)
            print(f"[P2-2] deepinterview: added Chinese triggers")

        s[trigger_idx] = triggers

    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n")
    print("[P2-2] Trigger governance applied to MANIFEST")


# ═══════════════════════════════════════════════════
# Main execution
# ═══════════════════════════════════════════════════
if __name__ == "__main__":
    print("=" * 60)
    print("SKILL ECOSYSTEM AUDIT — EXECUTING ALL RECOMMENDATIONS")
    print("=" * 60)

    # P0 phase
    print("\n--- P0: Critical Fixes ---")
    fix_broken_links()       # P0-1
    patch_manifest()         # P0-2, P0-6 (manifest), P1-3, P1-4, P1-5
    fix_routing_tests()      # P0-4
    fix_surface_policy()     # P0-5
    archive_adversarial_loop()  # P0-6 (filesystem)

    # P1 phase
    print("\n--- P1: Important Fixes ---")
    clean_archived_refs()    # P1-1
    clean_routing_layers()   # P1-2
    remove_empty_configs()   # P1-6
    add_exit_criteria()      # P1-7
    mark_superseded()        # P1-8

    # P2 phase (trigger governance)
    print("\n--- P2: Improvements ---")
    fix_trigger_governance() # P2-2

    print("\n" + "=" * 60)
    print("ALL CHANGES APPLIED SUCCESSFULLY")
    print("=" * 60)
