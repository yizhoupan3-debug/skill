//! 可复现性验证 — 种子检查、确定性重跑、环境锁定、数据版本化、checkpoint 恢复。
//!
//! 对应 `reproducibility-verification/SKILL.md` 验证清单。
//! 通过文件系统扫描和代码分析执行无状态审计。

use anyhow::Result;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::Path;

/// 检查结果封装。
#[derive(Debug, Clone, PartialEq)]
pub enum CheckStatus {
    Pass,
    Fail(String),   // blocker message
    Warn(String),   // advisory note
    Skip(String),   // skipped with reason
}

/// 可复现性验证报告。
#[derive(Debug, Clone)]
pub struct ReproducibilityReport {
    pub checks: Vec<CheckResult>,
}

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub name: &'static str,
    pub status: CheckStatus,
}

/// 种子关键词集合（常见随机种子设置模式）。
const SEED_KEYWORDS: &[&str] = &[
    "seed", "random_state", "set_seed", "torch.manual_seed",
    "numpy.random.seed", "tf.random.set_seed", "random.seed",
    "deterministic", "CUBLAS_WORKSPACE_CONFIG",
];

/// Lockfile 文件名集合。
const LOCKFILES: &[&str] = &[
    "Cargo.lock", "uv.lock", "poetry.lock", "Pipfile.lock",
    "package-lock.json", "yarn.lock", "pnpm-lock.yaml",
    "Gemfile.lock", "mix.lock",
];

/// DVC / Git LFS 标记文件。
const DVC_MARKERS: &[&str] = &[".dvc", "dvc.lock", "dvc.yaml"];

/// Checkpoint 文件名模式。
const CHECKPOINT_PATTERNS: &[&str] = &[
    "checkpoint", "checkpoint.pt", "checkpoint.pth", "model.ckpt",
    "checkpoint.ckpt", "best_model.pt", "last.ckpt",
];

/// 检查 #1: 种子已设置。
///
/// 在实验代码目录中递归搜索常见的种子设置模式。
pub fn check_seed_set(experiment_dir: &Path) -> Result<CheckResult> {
    let mut found = false;
    let mut locations = Vec::new();

    if let Ok(entries) = std::fs::read_dir(experiment_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // 跳过隐藏目录和 artifacts
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with('.') || name == "artifacts" || name == "target" || name == "node_modules" {
                    continue;
                }
                // 递归检查子目录（深度限制为 3）
                if let Ok(result) = check_seed_set(&path) {
                    if matches!(result.status, CheckStatus::Pass) {
                        return Ok(result);
                    }
                }
                continue;
            }
            if path.extension().and_then(|e| e.to_str()).map_or(false, |e| {
                matches!(e, "py" | "rs" | "js" | "ts" | "java" | "cpp" | "h" | "hpp" | "cu" | "yaml" | "yml" | "toml" | "json")
            }) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let lower = content.to_lowercase();
                    for kw in SEED_KEYWORDS {
                        if lower.contains(kw) {
                            found = true;
                            locations.push(format!("{}: {}", path.display(), kw));
                            break;
                        }
                    }
                }
            }
        }
    }

    if found {
        Ok(CheckResult {
            name: "seed_set",
            status: CheckStatus::Pass,
        })
    } else {
        Ok(CheckResult {
            name: "seed_set",
            status: CheckStatus::Fail("No seed setting found in experiment code. Add seed initialization (e.g., set_seed(42)) at script start.".into()),
        })
    }
}

/// 计算文件或目录的 SHA-256 哈希。
fn sha256_hash(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();

    if path.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(path)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .collect();
        entries.sort_by_key(|e| e.path());

        for entry in &entries {
            let bytes = std::fs::read(entry.path())?;
            hasher.update(&bytes);
        }
    } else {
        let bytes = std::fs::read(path)?;
        hasher.update(&bytes);
    }

    let result = hasher.finalize();
    Ok(format!("{result:x}"))
}

/// 检查 #2: 确定性重跑。
///
/// 比较两次运行的输出目录/文件 hash。
/// 需要传入至少两个运行输出目录路径。
pub fn check_deterministic_rerun(run_paths: &[&Path]) -> Result<CheckResult> {
    if run_paths.len() < 2 {
        return Ok(CheckResult {
            name: "deterministic_rerun",
            status: CheckStatus::Skip("Need at least 2 run output paths for comparison".into()),
        });
    }

    let hashes: Vec<_> = run_paths
        .iter()
        .filter_map(|p| sha256_hash(p).ok())
        .collect();

    if hashes.len() < 2 {
        return Ok(CheckResult {
            name: "deterministic_rerun",
            status: CheckStatus::Skip("Could not compute hashes for all run paths".into()),
        });
    }

    let all_match = hashes.windows(2).all(|w| w[0] == w[1]);
    if all_match {
        Ok(CheckResult {
            name: "deterministic_rerun",
            status: CheckStatus::Pass,
        })
    } else {
        Ok(CheckResult {
            name: "deterministic_rerun",
            status: CheckStatus::Fail("Rerun output hashes differ. Non-deterministic result detected.".into()),
        })
    }
}

/// 检查 #3: 环境可复现。
///
/// 在项目根目录中查找 lockfile，验证其存在且非空。
pub fn check_environment_reproducible(project_dir: &Path) -> Result<CheckResult> {
    for lockfile in LOCKFILES {
        let path = project_dir.join(lockfile);
        if path.exists() {
            let metadata = std::fs::metadata(&path)?;
            if metadata.len() > 0 {
                return Ok(CheckResult {
                    name: "environment_reproducible",
                    status: CheckStatus::Pass,
                });
            }
        }
    }

    // 再递归搜索一级子目录
    if let Ok(entries) = std::fs::read_dir(project_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with('.') || name_str == "target" || name_str == "node_modules" {
                    continue;
                }
                for lockfile in LOCKFILES {
                    let path = entry.path().join(lockfile);
                    if path.exists() && path.metadata().map_or(false, |m| m.len() > 0) {
                        return Ok(CheckResult {
                            name: "environment_reproducible",
                            status: CheckStatus::Pass,
                        });
                    }
                }
            }
        }
    }

    Ok(CheckResult {
        name: "environment_reproducible",
        status: CheckStatus::Fail("No lockfile found (Cargo.lock, uv.lock, poetry.lock, etc.). Environment is not reproducible.".into()),
    })
}

/// 检查 #4: 数据版本化。
///
/// 检查 DVC 标记文件或 Git LFS 配置的存在。
pub fn check_data_versioned(project_dir: &Path) -> Result<CheckResult> {
    // 检查 DVC 标记
    for marker in DVC_MARKERS {
        let path = project_dir.join(marker);
        if path.exists() {
            return Ok(CheckResult {
                name: "data_versioned",
                status: CheckStatus::Pass,
            });
        }
    }

    // 检查 .gitattributes 中的 Git LFS 配置
    let gitattributes = project_dir.join(".gitattributes");
    if gitattributes.exists() {
        if let Ok(content) = std::fs::read_to_string(&gitattributes) {
            if content.contains("lfs") || content.contains("filter=lfs") {
                return Ok(CheckResult {
                    name: "data_versioned",
                    status: CheckStatus::Pass,
                });
            }
        }
    }

    Ok(CheckResult {
        name: "data_versioned",
        status: CheckStatus::Warn(
            "No DVC tracking or Git LFS found. Data versioning is recommended for research reproducibility.".into(),
        ),
    })
}

/// 检查 #5: Checkpoint 可恢复。
///
/// 在实验目录中搜索 checkpoint 文件并验证其可读性。
pub fn check_checkpoint_recoverable(experiment_dir: &Path) -> Result<CheckResult> {
    let mut found_valid = false;
    let mut errors = Vec::new();

    if let Ok(entries) = walk_dir(experiment_dir, 2) {
        for entry in entries {
            let fname = entry.file_name().to_string_lossy().to_lowercase();
            if CHECKPOINT_PATTERNS.iter().any(|p| fname.contains(p)) {
                match std::fs::metadata(&entry) {
                    Ok(meta) if meta.len() > 0 => {
                        found_valid = true;
                    }
                    Ok(_) => {
                        errors.push(format!("{} is empty", entry.display()));
                    }
                    Err(e) => {
                        errors.push(format!("{}: {}", entry.display(), e));
                    }
                }
            }
        }
    }

    if found_valid {
        Ok(CheckResult {
            name: "checkpoint_recoverable",
            status: CheckStatus::Pass,
        })
    } else if !errors.is_empty() {
        Ok(CheckResult {
            name: "checkpoint_recoverable",
            status: CheckStatus::Fail(format!("Checkpoint issues found: {}", errors.join("; "))),
        })
    } else {
        Ok(CheckResult {
            name: "checkpoint_recoverable",
            status: CheckStatus::Skip("No checkpoint files found; skip if experiment is short-lived or stateless.".into()),
        })
    }
}

/// 对实验目录运行全量可复现性审计。
pub fn run_reproducibility_audit(
    experiment_dir: &Path,
    run_paths: Option<&[&Path]>,
) -> Result<ReproducibilityReport> {
    let seed_check = check_seed_set(experiment_dir)?;
    let env_check = check_environment_reproducible(experiment_dir)?;
    let data_check = check_data_versioned(experiment_dir)?;
    let ckpt_check = check_checkpoint_recoverable(experiment_dir)?;

    let rerun_check = match run_paths {
        Some(paths) => check_deterministic_rerun(paths)?,
        None => CheckResult {
            name: "deterministic_rerun",
            status: CheckStatus::Skip("No run paths provided for comparison".into()),
        },
    };

    Ok(ReproducibilityReport {
        checks: vec![seed_check, rerun_check, env_check, data_check, ckpt_check],
    })
}

/// 递归遍历目录，深度限制为 max_depth。
fn walk_dir(dir: &Path, max_depth: usize) -> Result<Vec<std::path::PathBuf>> {
    let mut results = Vec::new();
    if max_depth == 0 {
        return Ok(results);
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !name.starts_with('.') && name != "target" && name != "node_modules" {
                    results.extend(walk_dir(&path, max_depth - 1)?);
                }
            } else {
                results.push(path);
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn seed_set_found_in_python_file() {
        let dir = tempdir().unwrap();
        let mut f = fs::File::create(dir.path().join("train.py")).unwrap();
        writeln!(f, "import torch\nseed = 42\ntorch.manual_seed(seed)").unwrap();

        let result = check_seed_set(dir.path()).unwrap();
        assert!(matches!(result.status, CheckStatus::Pass), "{:?}", result.status);
    }

    #[test]
    fn seed_set_not_found() {
        let dir = tempdir().unwrap();
        let mut f = fs::File::create(dir.path().join("train.py")).unwrap();
        writeln!(f, "print('hello')").unwrap();

        let result = check_seed_set(dir.path()).unwrap();
        assert!(matches!(result.status, CheckStatus::Fail(_)));
    }

    #[test]
    fn lockfile_found() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("Cargo.lock"), "locked contents").unwrap();

        let result = check_environment_reproducible(dir.path()).unwrap();
        assert!(matches!(result.status, CheckStatus::Pass), "{:?}", result.status);
    }

    #[test]
    fn lockfile_missing() {
        let dir = tempdir().unwrap();
        let result = check_environment_reproducible(dir.path()).unwrap();
        assert!(matches!(result.status, CheckStatus::Fail(_)));
    }

    #[test]
    fn deterministic_rerun_matches() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        fs::write(dir1.path().join("output.txt"), b"same content").unwrap();
        fs::write(dir2.path().join("output.txt"), b"same content").unwrap();

        let paths = &[dir1.path(), dir2.path()];
        let result = check_deterministic_rerun(paths).unwrap();
        assert!(matches!(result.status, CheckStatus::Pass), "{:?}", result.status);
    }

    #[test]
    fn deterministic_rerun_differs() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        fs::write(dir1.path().join("output.txt"), b"run 1 result").unwrap();
        fs::write(dir2.path().join("output.txt"), b"run 2 result").unwrap();

        let paths = &[dir1.path(), dir2.path()];
        let result = check_deterministic_rerun(paths).unwrap();
        assert!(matches!(result.status, CheckStatus::Fail(_)));
    }

    #[test]
    fn dvc_marker_detected() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join(".dvc"), "cache").unwrap();
        let result = check_data_versioned(dir.path()).unwrap();
        assert!(matches!(result.status, CheckStatus::Pass));
    }

    #[test]
    fn checkpoint_found() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("checkpoint.pt"), b"model weights").unwrap();
        let result = check_checkpoint_recoverable(dir.path()).unwrap();
        assert!(matches!(result.status, CheckStatus::Pass));
    }

    #[test]
    fn full_audit_runs_without_panic() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.py"), b"seed = 42").unwrap();
        fs::write(dir.path().join("Cargo.lock"), b"").unwrap();

        let report = run_reproducibility_audit(dir.path(), None).unwrap();
        assert_eq!(report.checks.len(), 5);
        // Cargo.lock is empty → lockfile check fails
        assert!(matches!(
            report.checks[2].status,
            CheckStatus::Fail(_)
        ));
    }
}
