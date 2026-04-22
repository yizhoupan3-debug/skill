from __future__ import annotations

import sys
from pathlib import Path
import pytest

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts import host_integration_rs, sync_skills


def test_host_integration_rs_uses_release_binary_when_present(tmp_path: Path, monkeypatch) -> None:
    crate_root = tmp_path / "host-integration-rs"
    src_dir = crate_root / "src"
    release_bin = crate_root / "target" / "release" / "host-integration-rs"
    src_dir.mkdir(parents=True)
    release_bin.parent.mkdir(parents=True)
    manifest_path = crate_root / "Cargo.toml"
    lock_path = crate_root / "Cargo.lock"
    manifest_path.write_text("[package]\nname='host-integration-rs'\nversion='0.1.0'\n", encoding="utf-8")
    lock_path.write_text("", encoding="utf-8")
    (src_dir / "main.rs").write_text("fn main() {}\n", encoding="utf-8")
    release_bin.write_text("release", encoding="utf-8")

    monkeypatch.setattr(host_integration_rs, "CRATE_ROOT", crate_root)
    monkeypatch.setattr(host_integration_rs, "MANIFEST_PATH", manifest_path)
    monkeypatch.setattr(host_integration_rs, "RELEASE_BINARY_PATH", release_bin)

    def fail_run(*args, **kwargs):
        raise AssertionError("subprocess.run should not be called while resolving the binary")

    monkeypatch.setattr(host_integration_rs.subprocess, "run", fail_run)

    assert host_integration_rs._ensure_binary() == release_bin


def test_host_integration_rs_requires_prebuilt_release_binary_when_missing(
    tmp_path: Path, monkeypatch
) -> None:
    crate_root = tmp_path / "host-integration-rs"
    src_dir = crate_root / "src"
    release_bin = crate_root / "target" / "release" / "host-integration-rs"
    src_dir.mkdir(parents=True)
    manifest_path = crate_root / "Cargo.toml"
    lock_path = crate_root / "Cargo.lock"
    manifest_path.write_text("[package]\nname='host-integration-rs'\nversion='0.1.0'\n", encoding="utf-8")
    lock_path.write_text("", encoding="utf-8")
    (src_dir / "main.rs").write_text("fn main() {}\n", encoding="utf-8")

    monkeypatch.setattr(host_integration_rs, "CRATE_ROOT", crate_root)
    monkeypatch.setattr(host_integration_rs, "MANIFEST_PATH", manifest_path)
    monkeypatch.setattr(host_integration_rs, "RELEASE_BINARY_PATH", release_bin)

    with pytest.raises(RuntimeError, match="requires a prebuilt release binary"):
        host_integration_rs._ensure_binary()


def test_sync_skills_ignores_debug_skill_compiler_binary(
    tmp_path: Path, monkeypatch
) -> None:
    release_bin = tmp_path / "target" / "release" / "skill-compiler-rs"
    debug_bin = tmp_path / "target" / "debug" / "skill-compiler-rs"
    debug_bin.parent.mkdir(parents=True)
    debug_bin.write_text("debug", encoding="utf-8")

    monkeypatch.setattr(sync_skills, "SKILL_COMPILER_RS_RELEASE_BIN", release_bin)
    monkeypatch.setattr(sync_skills, "SKILL_COMPILER_RS_DEBUG_BIN", debug_bin)

    assert sync_skills.resolve_skill_compiler_binary() is None

    release_bin.parent.mkdir(parents=True, exist_ok=True)
    release_bin.write_text("release", encoding="utf-8")

    assert sync_skills.resolve_skill_compiler_binary() == release_bin
