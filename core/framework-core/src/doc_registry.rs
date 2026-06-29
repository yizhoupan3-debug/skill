//! 文档路径注册表 — 仓库内所有文档相对路径的唯一真源。
//!
//! # 使用场景
//! - 测试代码中引用文档路径（避免硬编码字符串）
//! - `framework doctor` 验证文档存在性和目录规范
//! - 文档重构时只需修改此文件，所有引用方自动生效
//!
//! # 原则
//! - 路径为**仓库根相对路径**
//! - 需要绝对路径时调用 `resolve(root, key)` 或 `root.join(key)`
//! - 已删除的旧路径列入 `DEPRECATED`，不移除（防止误复用）

use std::path::{Path, PathBuf};

// ── 核心文档 ──

/// 一站式文档地图（架构参考真源）
pub const DOC_README: &str = "docs/README.md";

// ── 研发文档 ──

/// 运维中枢
pub const DOC_OPS_INDEX: &str = "docs/operations/index.md";

// ── 根级 ──

/// 跨宿主代理策略
pub const AGENTS_MD: &str = "AGENTS.md";
/// 仓库快速入门
pub const ROOT_README: &str = "README.md";

// ── 治理目录 ──

/// 审计报告目录
pub const DIR_REPORTS: &str = "docs/reports";
/// 计划文档目录
pub const DIR_PLANS: &str = "docs/plans";

// ── 已删除路径（注册以防止误复用）──

/// 已删除的文档路径集合
pub const DEPRECATED: &[&str] = &[
    "docs/spec.md",
    "docs/architecture.md",
    "docs/framework_naming_conventions.md",
    "docs/framework_profile_contract.md",
    "docs/generated.md",
    "docs/runtime-status.md",
    "docs/hosts/README.md",
    "docs/design-decisions.md",
    "docs/migration.md",
    "docs/contributing.md",
    "docs/hosts/handbook.md",
    "docs/adr/010-ideal-architecture-v10.md",
    "docs/hosts/_common.md",
    "docs/hosts/hook-hosts.md",
    "docs/hosts/opencode.md",
    "docs/research-harness.md",
    "docs/research/routing-contracts.md",
    "docs/research/harness.md",
    "docs/reports/2026-06-24-runtime-audit.md",
];

// ── 查询函数 ──

/// 将文档 key 解析为相对于 repo_root 的绝对路径。
pub fn resolve(root: &Path, key: &str) -> PathBuf {
    root.join(key)
}

/// 检查文档 key 对应的文件是否存在。
pub fn exists(root: &Path, key: &str) -> bool {
    resolve(root, key).is_file()
}

/// 所有活跃文档路径。
pub fn all_keys() -> &'static [&'static str] {
    &[DOC_README, DOC_OPS_INDEX, AGENTS_MD, ROOT_README]
}

/// 框架治理的文档目录。
pub fn all_dirs() -> &'static [&'static str] {
    &[DIR_REPORTS, DIR_PLANS]
}

// ── 测试辅助（仅 test/test-sync）──

#[cfg(any(test, feature = "test-sync"))]
/// 返回所有不存在的活跃文档路径（用于测试断言）。
pub fn missing_keys(root: &Path) -> Vec<String> {
    all_keys()
        .iter()
        .filter(|key| !exists(root, key))
        .map(|s| s.to_string())
        .collect()
}
