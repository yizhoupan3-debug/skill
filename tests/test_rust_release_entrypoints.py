from __future__ import annotations

import os
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from scripts import host_integration_rs, router_rs_runner, rust_binary_runner, sync_skills


def test_host_integration_rs_uses_release_binary_when_present(tmp_path: Path, monkeypatch) -> None:
    crate_root = tmp_path / "host-integration-rs"
    release_bin = crate_root / "target" / "release" / "host-integration-rs"
    captured: dict[str, object] = {}

    def fake_ensure_rust_binary(**kwargs):
        captured.update(kwargs)
        return release_bin

    monkeypatch.setattr(host_integration_rs, "CRATE_ROOT", crate_root)
    monkeypatch.setattr(host_integration_rs, "PROJECT_ROOT", tmp_path)
    monkeypatch.setattr(host_integration_rs, "ensure_rust_binary", fake_ensure_rust_binary)

    assert host_integration_rs._ensure_binary() == release_bin
    assert captured["crate_root"] == crate_root
    assert captured["binary_name"] == "host-integration-rs"
    assert captured["release"] is True
    assert captured["allow_stale_fallback"] is False
    assert captured["allow_cross_profile_fallback"] is False
    assert captured["cwd"] == tmp_path


def test_router_rs_runner_delegates_to_shared_rust_binary_runner(tmp_path: Path, monkeypatch) -> None:
    crate_root = tmp_path / "router-rs"
    captured: dict[str, object] = {}

    def fake_ensure_rust_binary(**kwargs):
        captured.update(kwargs)
        return tmp_path / "router-rs-bin"

    monkeypatch.setattr(router_rs_runner, "CRATE_ROOT", crate_root)
    monkeypatch.setattr(router_rs_runner, "PROJECT_ROOT", tmp_path)
    monkeypatch.setattr(router_rs_runner, "ensure_rust_binary", fake_ensure_rust_binary)

    assert router_rs_runner._ensure_binary() == (tmp_path / "router-rs-bin")
    assert captured["crate_root"] == crate_root
    assert captured["allow_stale_fallback"] is False
    assert captured["allow_cross_profile_fallback"] is False
    assert captured["release"] is False
    assert captured["binary_name"] == "router-rs"
    assert captured["cwd"] == tmp_path


def test_ensure_rust_binary_uses_fresh_existing_binary_without_build(
    tmp_path: Path, monkeypatch
) -> None:
    crate_root = tmp_path / "router-rs"
    src_dir = crate_root / "src"
    debug_bin = crate_root / "target" / "debug" / "router-rs"
    src_dir.mkdir(parents=True)
    debug_bin.parent.mkdir(parents=True)
    manifest_path = crate_root / "Cargo.toml"
    lock_path = crate_root / "Cargo.lock"
    manifest_path.write_text("[package]\nname='router-rs'\nversion='0.1.0'\n", encoding="utf-8")
    lock_path.write_text("", encoding="utf-8")
    (src_dir / "main.rs").write_text("fn main() {}\n", encoding="utf-8")
    debug_bin.write_text("debug", encoding="utf-8")

    src_mtime = 1_700_000_100
    bin_mtime = 1_700_000_200
    os.utime(manifest_path, (src_mtime, src_mtime))
    os.utime(lock_path, (src_mtime, src_mtime))
    os.utime(src_dir / "main.rs", (src_mtime, src_mtime))
    os.utime(debug_bin, (bin_mtime, bin_mtime))

    def fail_run(*args, **kwargs):
        raise AssertionError("cargo build should not run when a fresh binary already exists")

    monkeypatch.setattr(rust_binary_runner.subprocess, "run", fail_run)

    assert rust_binary_runner.ensure_rust_binary(
        crate_root=crate_root,
        binary_name="router-rs",
        release=False,
        cwd=tmp_path,
    ) == debug_bin


def test_ensure_rust_binary_can_fallback_to_stale_binary_when_build_fails(
    tmp_path: Path, monkeypatch
) -> None:
    crate_root = tmp_path / "router-rs"
    src_dir = crate_root / "src"
    debug_bin = crate_root / "target" / "debug" / "router-rs"
    src_dir.mkdir(parents=True)
    debug_bin.parent.mkdir(parents=True)
    manifest_path = crate_root / "Cargo.toml"
    lock_path = crate_root / "Cargo.lock"
    manifest_path.write_text("[package]\nname='router-rs'\nversion='0.1.0'\n", encoding="utf-8")
    lock_path.write_text("", encoding="utf-8")
    (src_dir / "main.rs").write_text("fn main() {}\n", encoding="utf-8")
    debug_bin.write_text("debug", encoding="utf-8")

    bin_mtime = 1_700_000_100
    src_mtime = 1_700_000_200
    os.utime(debug_bin, (bin_mtime, bin_mtime))
    os.utime(manifest_path, (src_mtime, src_mtime))
    os.utime(lock_path, (src_mtime, src_mtime))
    os.utime(src_dir / "main.rs", (src_mtime, src_mtime))

    def fail_build(*args, **kwargs):
        raise rust_binary_runner.subprocess.CalledProcessError(
            returncode=1,
            cmd=args[0],
            stderr="build failed",
        )

    monkeypatch.setattr(rust_binary_runner.subprocess, "run", fail_build)

    assert rust_binary_runner.ensure_rust_binary(
        crate_root=crate_root,
        binary_name="router-rs",
        release=False,
        allow_stale_fallback=True,
        cwd=tmp_path,
    ) == debug_bin


def test_ensure_rust_binary_does_not_cross_profiles_when_release_is_strict(
    tmp_path: Path, monkeypatch
) -> None:
    crate_root = tmp_path / "router-rs"
    src_dir = crate_root / "src"
    debug_bin = crate_root / "target" / "debug" / "router-rs"
    src_dir.mkdir(parents=True)
    debug_bin.parent.mkdir(parents=True)
    manifest_path = crate_root / "Cargo.toml"
    lock_path = crate_root / "Cargo.lock"
    manifest_path.write_text("[package]\nname='router-rs'\nversion='0.1.0'\n", encoding="utf-8")
    lock_path.write_text("", encoding="utf-8")
    (src_dir / "main.rs").write_text("fn main() {}\n", encoding="utf-8")
    debug_bin.write_text("debug", encoding="utf-8")

    src_mtime = 1_700_000_200
    debug_mtime = 1_700_000_300
    os.utime(manifest_path, (src_mtime, src_mtime))
    os.utime(lock_path, (src_mtime, src_mtime))
    os.utime(src_dir / "main.rs", (src_mtime, src_mtime))
    os.utime(debug_bin, (debug_mtime, debug_mtime))

    def fail_build(*args, **kwargs):
        raise rust_binary_runner.subprocess.CalledProcessError(
            returncode=1,
            cmd=args[0],
            stderr="release build failed",
        )

    monkeypatch.setattr(rust_binary_runner.subprocess, "run", fail_build)

    try:
        rust_binary_runner.ensure_rust_binary(
            crate_root=crate_root,
            binary_name="router-rs",
            release=True,
            allow_stale_fallback=True,
            allow_cross_profile_fallback=False,
            cwd=tmp_path,
        )
    except RuntimeError as exc:
        assert "release build failed" in str(exc)
    else:
        raise AssertionError("strict release resolution should not fall back to debug binaries")


def test_sync_skills_ignores_debug_skill_compiler_binary(
    tmp_path: Path, monkeypatch
) -> None:
    crate_root = tmp_path / "skill-compiler-rs"
    release_bin = tmp_path / "target" / "release" / "skill-compiler-rs"
    debug_bin = tmp_path / "target" / "debug" / "skill-compiler-rs"
    manifest_path = crate_root / "Cargo.toml"
    src_main = crate_root / "src" / "main.rs"
    src_main.parent.mkdir(parents=True)
    debug_bin.parent.mkdir(parents=True)
    debug_bin.write_text("debug", encoding="utf-8")
    manifest_path.write_text("[package]\nname='skill-compiler-rs'\nversion='0.1.0'\n", encoding="utf-8")
    src_main.write_text("fn main() {}\n", encoding="utf-8")

    monkeypatch.setattr(sync_skills, "SKILL_COMPILER_RS_RELEASE_BIN", release_bin)
    monkeypatch.setattr(sync_skills, "SKILL_COMPILER_RS_DEBUG_BIN", debug_bin)
    monkeypatch.setattr(sync_skills, "SKILL_COMPILER_RS_DIR", crate_root)

    assert sync_skills.resolve_skill_compiler_binary() is None

    release_bin.parent.mkdir(parents=True, exist_ok=True)
    release_bin.write_text("release", encoding="utf-8")
    src_mtime = 1_700_000_100
    release_mtime = 1_700_000_200
    manifest_mtime = 1_700_000_050
    os.utime(manifest_path, (manifest_mtime, manifest_mtime))
    os.utime(src_main, (src_mtime, src_mtime))
    os.utime(release_bin, (release_mtime, release_mtime))

    assert sync_skills.resolve_skill_compiler_binary() == release_bin


def test_sync_skills_ignores_stale_release_skill_compiler_binary(
    tmp_path: Path, monkeypatch
) -> None:
    crate_root = tmp_path / "skill-compiler-rs"
    release_bin = tmp_path / "target" / "release" / "skill-compiler-rs"
    manifest_path = crate_root / "Cargo.toml"
    src_main = crate_root / "src" / "main.rs"
    src_main.parent.mkdir(parents=True)
    release_bin.parent.mkdir(parents=True)
    manifest_path.write_text("[package]\nname='skill-compiler-rs'\nversion='0.1.0'\n", encoding="utf-8")
    src_main.write_text("fn main() {}\n", encoding="utf-8")
    release_bin.write_text("release", encoding="utf-8")

    monkeypatch.setattr(sync_skills, "SKILL_COMPILER_RS_RELEASE_BIN", release_bin)
    monkeypatch.setattr(sync_skills, "SKILL_COMPILER_RS_DIR", crate_root)

    release_mtime = 1_700_000_100
    src_mtime = 1_700_000_200
    manifest_mtime = 1_700_000_150
    os.utime(release_bin, (release_mtime, release_mtime))
    os.utime(manifest_path, (manifest_mtime, manifest_mtime))
    os.utime(src_main, (src_mtime, src_mtime))

    assert sync_skills.resolve_skill_compiler_binary() is None
