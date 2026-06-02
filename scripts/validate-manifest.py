#!/usr/bin/env python3
"""manifest 完整性校验脚本 (P1-003)

校验规则:
  1. host_platforms 无重复 (manifest + routing)
  2. hot routing skill_path 文件存在性
  3. 冷热一致性: kind=cold 不应出现在 routing; kind=skill 应在 routing
  4. L0 gate skill 应有 routing_priority (priority) 字段
  5. manifest 与 plugin catalog slug 集合一致性
  6. plugin catalog host_support.platforms 无重复
  7. plugin catalog skill_path 文件存在性
  8. trigger_hints 重叠检测 (INFO, 不阻断)

用法:
  python3 scripts/validate-manifest.py          # 校验
  python3 scripts/validate-manifest.py --fix    # 校验 + 自动修复规则 1 去重

退出码: 0 = 全绿, 1 = 有失败
"""

import argparse
import json
import os
import sys
from collections import Counter
from pathlib import Path

# ---------------------------------------------------------------------------
# Paths (relative to project root)
# ---------------------------------------------------------------------------
MANIFEST_PATH = "skills/SKILL_MANIFEST.json"
ROUTING_PATH = "skills/SKILL_ROUTING_RUNTIME.json"
CATALOG_PATH = "skills/SKILL_PLUGIN_CATALOG.json"


def resolve_project_root() -> Path:
    """从脚本位置向上查找项目根目录 (含 .claude/ 目录)."""
    p = Path(__file__).resolve().parent.parent
    if (p / ".claude").is_dir():
        return p
    # fallback: cwd
    return Path.cwd()


def load_json(path: Path) -> dict:
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def build_index(keys: list[str]) -> dict[str, int]:
    """将 keys 数组转为 {field_name: position_index} 映射."""
    return {k: i for i, k in enumerate(keys)}


# ---------------------------------------------------------------------------
# Rule implementations
# ---------------------------------------------------------------------------

def rule_host_platforms_dedup(manifest: dict, routing: dict, fix: bool = False) -> tuple[bool, list[str]]:
    """规则 1: host_platforms 无重复."""
    issues: list[str] = []
    fixed_count = 0

    # Check manifest
    m_keys = build_index(manifest["keys"])
    m_hp_idx = m_keys["host_platforms"]
    m_slug_idx = m_keys["slug"]
    for skill in manifest["skills"]:
        hp = skill[m_hp_idx]
        if not isinstance(hp, list):
            issues.append(f"  manifest/{skill[m_slug_idx]}: host_platforms 不是数组")
            continue
        if len(hp) != len(set(hp)):
            duped = [k for k, v in Counter(hp).items() if v > 1]
            issues.append(f"  manifest/{skill[m_slug_idx]}: 重复平台 {duped}")
            if fix:
                skill[m_hp_idx] = list(dict.fromkeys(hp))  # 保序去重
                fixed_count += 1

    # Check routing
    r_keys = build_index(routing["keys"])
    r_hp_idx = r_keys["host_platforms"]
    r_slug_idx = r_keys["slug"]
    for skill in routing["skills"]:
        hp = skill[r_hp_idx]
        if not isinstance(hp, list):
            issues.append(f"  routing/{skill[r_slug_idx]}: host_platforms 不是数组")
            continue
        if len(hp) != len(set(hp)):
            duped = [k for k, v in Counter(hp).items() if v > 1]
            issues.append(f"  routing/{skill[r_slug_idx]}: 重复平台 {duped}")
            if fix:
                skill[r_hp_idx] = list(dict.fromkeys(hp))
                fixed_count += 1

    if fix and fixed_count > 0:
        # Write back
        with open(MANIFEST_PATH, "w", encoding="utf-8") as f:
            json.dump(manifest, f, indent=2, ensure_ascii=False)
            f.write("\n")
        with open(ROUTING_PATH, "w", encoding="utf-8") as f:
            json.dump(routing, f, indent=2, ensure_ascii=False)
            f.write("\n")
        issues = [f"  --fix: 已修复 {fixed_count} 条重复 (原始 {len(issues)} 条问题)"]

    passed = len(issues) == 0 or (fix and fixed_count > 0)
    return passed, issues


def rule_routing_path_exists(routing: dict, project_root: Path) -> tuple[bool, list[str]]:
    """规则 2: routing skill_path 指向的文件必须存在."""
    issues: list[str] = []
    r_keys = build_index(routing["keys"])
    r_sp_idx = r_keys["skill_path"]
    r_slug_idx = r_keys["slug"]

    for skill in routing["skills"]:
        sp = skill[r_sp_idx]
        full = project_root / sp
        if not full.is_file():
            issues.append(f"  {skill[r_slug_idx]}: {sp} -> 文件不存在")

    return len(issues) == 0, issues


def rule_cold_hot_consistency(manifest: dict, routing: dict) -> tuple[bool, list[str]]:
    """规则 3: kind=cold 不应出现在 routing; kind=skill 应在 routing."""
    issues: list[str] = []

    m_keys = build_index(manifest["keys"])
    m_slug_idx = m_keys["slug"]
    m_kind_idx = m_keys["kind"]

    r_keys = build_index(routing["keys"])
    r_slug_idx = r_keys["slug"]
    routing_slugs = {s[r_slug_idx] for s in routing["skills"]}

    for skill in manifest["skills"]:
        slug = skill[m_slug_idx]
        kind = skill[m_kind_idx]
        if kind == "cold" and slug in routing_slugs:
            issues.append(f"  {slug}: kind=cold 但出现在 routing 中")
        elif kind == "skill" and slug not in routing_slugs:
            issues.append(f"  {slug}: kind=skill 但不在 routing 中")

    return len(issues) == 0, issues


def rule_l0_gate_priority(manifest: dict) -> tuple[bool, list[str]]:
    """规则 4: L0 gate skill 应有 priority 字段."""
    issues: list[str] = []
    m_keys = build_index(manifest["keys"])
    m_slug_idx = m_keys["slug"]
    m_layer_idx = m_keys["layer"]
    m_gate_idx = m_keys.get("gate")
    m_priority_idx = m_keys.get("priority")

    if m_gate_idx is None or m_priority_idx is None:
        return True, ["  manifest 缺少 gate 或 priority 字段定义, 跳过"]

    for skill in manifest["skills"]:
        if skill[m_layer_idx] == "L0" and skill[m_gate_idx] is not None:
            priority = skill[m_priority_idx]
            if priority is None or priority == "":
                issues.append(
                    f"  {skill[m_slug_idx]}: L0 gate (gate={skill[m_gate_idx]}) 缺少 priority"
                )

    return len(issues) == 0, issues


def rule_manifest_catalog_slug_consistency(
    manifest: dict, catalog: dict
) -> tuple[bool, list[str]]:
    """规则 5: manifest 与 plugin catalog slug 集合一致."""
    issues: list[str] = []
    m_keys = build_index(manifest["keys"])
    m_slug_idx = m_keys["slug"]

    manifest_slugs = {s[m_slug_idx] for s in manifest["skills"]}
    catalog_slugs = set(catalog["skills"].keys())

    only_manifest = sorted(manifest_slugs - catalog_slugs)
    only_catalog = sorted(catalog_slugs - manifest_slugs)

    for slug in only_manifest:
        issues.append(f"  {slug}: 仅在 manifest 中, catalog 缺失")
    for slug in only_catalog:
        issues.append(f"  {slug}: 仅在 catalog 中, manifest 缺失")

    return len(issues) == 0, issues


def rule_catalog_platforms_dedup(catalog: dict) -> tuple[bool, list[str]]:
    """规则 6: plugin catalog host_support.platforms 无重复."""
    issues: list[str] = []
    for slug, entry in catalog.get("skills", {}).items():
        platforms = entry.get("host_support", {}).get("platforms", [])
        if not isinstance(platforms, list):
            issues.append(f"  {slug}: host_support.platforms 不是数组")
            continue
        if len(platforms) != len(set(platforms)):
            duped = [k for k, v in Counter(platforms).items() if v > 1]
            issues.append(f"  {slug}: 重复平台 {duped}")
    return len(issues) == 0, issues


def rule_catalog_path_exists(catalog: dict, project_root: Path) -> tuple[bool, list[str]]:
    """规则 7: plugin catalog skill_path 指向的文件必须存在."""
    issues: list[str] = []
    for slug, entry in catalog.get("skills", {}).items():
        sp = entry.get("skill_path", "")
        if sp and not (project_root / sp).is_file():
            issues.append(f"  {slug}: {sp} -> 文件不存在")
    return len(issues) == 0, issues


def rule_trigger_hints_overlap(manifest: dict) -> tuple[bool, list[str]]:
    """规则 8 (INFO): 检测不同 skill 之间 trigger_hints 的重叠.

    一些重叠是合理的（如 paper-reviewer 和 paper-workbench 共享审稿相关 hint），
    因此本规则仅报告重叠项，不设为 FAIL。
    """
    issues: list[str] = []
    m_keys = build_index(manifest["keys"])
    m_slug_idx = m_keys["slug"]
    m_th_idx = m_keys.get("trigger_hints")

    if m_th_idx is None:
        return True, ["  manifest 缺少 trigger_hints 字段定义, 跳过"]

    # 构建 hint -> [slug, ...] 映射（忽略大小写）
    hint_to_slugs: dict[str, list[str]] = {}
    for skill in manifest["skills"]:
        slug = skill[m_slug_idx]
        hints = skill[m_th_idx]
        if not isinstance(hints, list):
            continue
        for hint in hints:
            key = hint.lower().strip()
            if key:
                hint_to_slugs.setdefault(key, []).append(slug)

    # 找出被 2 个以上 skill 共享的 hint
    for key in sorted(hint_to_slugs):
        slugs = hint_to_slugs[key]
        if len(slugs) > 1:
            # 还原原始大小写（取第一个出现的）
            issues.append(
                f'  "{key}" 共享于: {", ".join(sorted(set(slugs)))}'
            )

    # INFO 规则始终返回 True
    return True, issues


# ---------------------------------------------------------------------------
# Runner
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="manifest 完整性校验 (P1-003)")
    parser.add_argument("--fix", action="store_true", help="自动修复规则 1 (host_platforms 去重)")
    args = parser.parse_args()

    project_root = resolve_project_root()
    os.chdir(project_root)

    # Load data
    try:
        manifest = load_json(MANIFEST_PATH)
        routing = load_json(ROUTING_PATH)
        catalog = load_json(CATALOG_PATH)
    except FileNotFoundError as e:
        print(f"FAIL  文件加载失败: {e}", file=sys.stderr)
        sys.exit(1)

    rules = [
        ("R1", "host_platforms 无重复",
         lambda: rule_host_platforms_dedup(manifest, routing, fix=args.fix)),
        ("R2", "hot routing skill_path 存在性",
         lambda: rule_routing_path_exists(routing, project_root)),
        ("R3", "冷热一致性",
         lambda: rule_cold_hot_consistency(manifest, routing)),
        ("R4", "L0 gate priority 字段",
         lambda: rule_l0_gate_priority(manifest)),
        ("R5", "manifest <-> catalog slug 一致性",
         lambda: rule_manifest_catalog_slug_consistency(manifest, catalog)),
        ("R6", "catalog host_support.platforms 无重复",
         lambda: rule_catalog_platforms_dedup(catalog)),
        ("R7", "catalog skill_path 存在性",
         lambda: rule_catalog_path_exists(catalog, project_root)),
        ("R8", "trigger_hints 重叠检测 (INFO)",
         lambda: rule_trigger_hints_overlap(manifest)),
    ]

    all_passed = True
    results: list[tuple[str, str, bool, list[str]]] = []

    for code, desc, check_fn in rules:
        passed, details = check_fn()
        results.append((code, desc, passed, details))
        if not passed:
            all_passed = False

    # Report
    print("=" * 60)
    print("manifest 完整性校验报告")
    print(f"  manifest:  {MANIFEST_PATH}  ({len(manifest['skills'])} skills)")
    print(f"  routing:   {ROUTING_PATH}  ({len(routing['skills'])} skills)")
    print(f"  catalog:   {CATALOG_PATH}  ({len(catalog['skills'])} skills)")
    if args.fix:
        print("  mode:      --fix (自动修复)")
    print("=" * 60)

    # INFO-level rules (always pass, but show details under INFO label)
    INFO_RULES = {"R8"}

    for code, desc, passed, details in results:
        if code in INFO_RULES:
            status = "INFO"
        else:
            status = "PASS" if passed else "FAIL"
        print(f"\n[{status}] {code}: {desc}")
        for line in details:
            print(line)

    print()
    if all_passed:
        print("RESULT: ALL PASS")
    else:
        fail_count = sum(1 for _, _, p, _ in results if not p)
        print(f"RESULT: {fail_count} rule(s) FAILED")

    sys.exit(0 if all_passed else 1)


if __name__ == "__main__":
    main()
