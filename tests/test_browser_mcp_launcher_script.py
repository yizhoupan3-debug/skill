from __future__ import annotations

import json
import os
import shutil
import sqlite3
import subprocess
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]
START_SCRIPT = PROJECT_ROOT / "tools" / "browser-mcp" / "scripts" / "start_browser_mcp.sh"
RESOLVER_SCRIPT = (
    PROJECT_ROOT / "tools" / "browser-mcp" / "scripts" / "resolve_runtime_attach_artifact.mjs"
)
REAL_NODE_BIN = shutil.which("node")
assert REAL_NODE_BIN is not None
SOURCE_FILES = [
    "index.ts",
    "runtime.ts",
    "server.ts",
    "types.ts",
    "errors.ts",
]
DIST_FILES = [
    "index.js",
    "runtime.js",
    "server.js",
    "types.js",
    "errors.js",
]


def _write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def _copy_launcher_scripts(repo_root: Path) -> Path:
    script_root = repo_root / "tools" / "browser-mcp" / "scripts"
    script_root.mkdir(parents=True, exist_ok=True)
    shutil.copy2(START_SCRIPT, script_root / "start_browser_mcp.sh")
    shutil.copy2(RESOLVER_SCRIPT, script_root / "resolve_runtime_attach_artifact.mjs")
    (script_root / "start_browser_mcp.sh").chmod(0o755)
    (script_root / "resolve_runtime_attach_artifact.mjs").chmod(0o755)
    return script_root


def _seed_browser_package(repo_root: Path) -> Path:
    return _seed_browser_package_state(repo_root)


def _seed_browser_package_state(
    repo_root: Path,
    *,
    include_node_modules: bool = True,
    src_newer_than_dist: bool = False,
) -> Path:
    package_root = repo_root / "tools" / "browser-mcp"
    if include_node_modules:
        (package_root / "node_modules").mkdir(parents=True, exist_ok=True)
    src_root = package_root / "src"
    dist_root = package_root / "dist"

    for name in SOURCE_FILES:
        _write_text(src_root / name, f"// {name}\n")
    for name in DIST_FILES:
        _write_text(dist_root / name, f"// built {name}\n")

    older = 1_700_000_000
    newer = older + 10
    src_ts = newer if src_newer_than_dist else older
    dist_ts = older if src_newer_than_dist else newer
    for name in SOURCE_FILES:
        os.utime(src_root / name, (src_ts, src_ts))
    for name in DIST_FILES:
        os.utime(dist_root / name, (dist_ts, dist_ts))
    return package_root


def _install_fake_node(bin_dir: Path, output_path: Path) -> None:
    _write_text(
        bin_dir / "node",
        "\n".join(
            [
                "#!/bin/sh",
                "SCRIPT_NAME=${1##*/}",
                "if [ \"$#\" -gt 0 ] && [ \"$SCRIPT_NAME\" = \"resolve_runtime_attach_artifact.mjs\" ]; then",
                "  exec \"$REAL_NODE_BIN\" \"$@\"",
                "fi",
                "python3 - \"$@\" <<'PY'",
                "import json, os, sys",
                "from pathlib import Path",
                "Path(os.environ['FAKE_NODE_OUTPUT']).write_text(",
                "    json.dumps({'argv': sys.argv[1:], 'cwd': os.getcwd()}, ensure_ascii=False),",
                "    encoding='utf-8',",
                ")",
                "PY",
            ]
        )
        + "\n",
    )
    (bin_dir / "node").chmod(0o755)
    output_path.parent.mkdir(parents=True, exist_ok=True)


def _install_fake_npm(bin_dir: Path, output_path: Path) -> None:
    _write_text(
        bin_dir / "npm",
        "\n".join(
            [
                "#!/bin/sh",
                "python3 - \"$@\" <<'PY'",
                "import json, os, sys",
                "from pathlib import Path",
                "path = Path(os.environ['FAKE_NPM_OUTPUT'])",
                "calls = []",
                "if path.exists():",
                "    calls = json.loads(path.read_text(encoding='utf-8'))",
                "calls.append({'argv': sys.argv[1:], 'cwd': os.getcwd()})",
                "path.write_text(json.dumps(calls, ensure_ascii=False), encoding='utf-8')",
                "PY",
            ]
        )
        + "\n",
    )
    (bin_dir / "npm").chmod(0o755)
    output_path.parent.mkdir(parents=True, exist_ok=True)


def _read_fake_node_output(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


def _read_fake_npm_output(path: Path) -> list[dict[str, object]]:
    if not path.exists():
        return []
    return json.loads(path.read_text(encoding="utf-8"))


def _run_launcher(repo_root: Path, *, env: dict[str, str], extra_args: list[str] | None = None) -> dict[str, object]:
    output_path = repo_root / "fake-node-output.json"
    npm_output_path = repo_root / "fake-npm-output.json"
    bin_dir = repo_root / "fake-bin"
    _install_fake_node(bin_dir, output_path)
    _install_fake_npm(bin_dir, npm_output_path)
    launcher_env = os.environ.copy()
    launcher_env.update(env)
    launcher_env["PATH"] = f"{bin_dir}:{launcher_env.get('PATH', '')}"
    launcher_env["FAKE_NODE_OUTPUT"] = str(output_path)
    launcher_env["FAKE_NPM_OUTPUT"] = str(npm_output_path)
    launcher_env["REAL_NODE_BIN"] = REAL_NODE_BIN

    subprocess.run(
        [str(repo_root / "tools" / "browser-mcp" / "scripts" / "start_browser_mcp.sh"), *(extra_args or [])],
        cwd=repo_root,
        env=launcher_env,
        check=True,
    )
    result = _read_fake_node_output(output_path)
    result["npm_calls"] = _read_fake_npm_output(npm_output_path)
    return result


def _prepare_repo(tmp_path: Path) -> Path:
    return _prepare_repo_state(tmp_path)


def _prepare_repo_state(
    tmp_path: Path,
    *,
    include_node_modules: bool = True,
    src_newer_than_dist: bool = False,
) -> Path:
    repo_root = tmp_path / "repo"
    _copy_launcher_scripts(repo_root)
    _seed_browser_package_state(
        repo_root,
        include_node_modules=include_node_modules,
        src_newer_than_dist=src_newer_than_dist,
    )
    return repo_root


def test_launcher_prefers_explicit_attach_descriptor_env_over_auto_discovery(tmp_path: Path) -> None:
    repo_root = _prepare_repo(tmp_path)
    scratch_root = repo_root / "framework_runtime" / "artifacts" / "scratch"
    _write_text(
        scratch_root / "older" / "TRACE_RESUME_MANIFEST.json",
        json.dumps(
            {
                "schema_version": "runtime-resume-manifest-v1",
                "event_transport_path": "/auto/discovered/runtime_event_transports/older.json",
                "updated_at": "2026-04-23T00:00:00+00:00",
            }
        )
        + "\n",
    )

    result = _run_launcher(
        repo_root,
        env={
            "BROWSER_MCP_RUNTIME_ATTACH_DESCRIPTOR_PATH": "/explicit/descriptor.json",
        },
        extra_args=["--transport", "http"],
    )

    assert result["cwd"] == str(repo_root / "tools" / "browser-mcp")
    assert result["argv"] == [
        "dist/index.js",
        "--runtime-attach-descriptor-path",
        "/explicit/descriptor.json",
        "--transport",
        "http",
    ]


def test_launcher_prefers_highest_priority_attach_env_across_full_precedence_ladder(
    tmp_path: Path,
) -> None:
    repo_root = _prepare_repo(tmp_path)

    result = _run_launcher(
        repo_root,
        env={
            "BROWSER_MCP_RUNTIME_ATTACH_DESCRIPTOR_PATH": "/explicit/descriptor.json",
            "BROWSER_MCP_RUNTIME_ATTACH_ARTIFACT_PATH": "/explicit/attach-artifact.json",
            "BROWSER_MCP_RUNTIME_BINDING_ARTIFACT_PATH": "/compat/binding.json",
            "BROWSER_MCP_RUNTIME_HANDOFF_PATH": "/compat/handoff.json",
            "BROWSER_MCP_RUNTIME_RESUME_MANIFEST_PATH": "/compat/resume.json",
        },
    )

    assert result["argv"] == [
        "dist/index.js",
        "--runtime-attach-descriptor-path",
        "/explicit/descriptor.json",
    ]


def test_launcher_auto_discovers_sqlite_backed_attach_artifact(tmp_path: Path) -> None:
    repo_root = _prepare_repo(tmp_path)
    db_path = (
        repo_root
        / "framework_runtime"
        / "artifacts"
        / "scratch"
        / "sqlite-run"
        / "runtime_checkpoint_store.sqlite3"
    )
    db_path.parent.mkdir(parents=True, exist_ok=True)
    connection = sqlite3.connect(db_path)
    connection.execute(
        "CREATE TABLE runtime_storage_payloads (payload_key TEXT PRIMARY KEY, payload_text TEXT NOT NULL)"
    )
    connection.execute(
        "INSERT INTO runtime_storage_payloads (payload_key, payload_text) VALUES (?, ?)",
        (
            "runtime-data/TRACE_RESUME_MANIFEST.json",
            json.dumps(
                {
                    "schema_version": "runtime-resume-manifest-v1",
                    "event_transport_path": "/logical/sqlite/runtime_event_transports/session__job.json",
                    "updated_at": "2026-04-23T00:10:00+00:00",
                }
            ),
        ),
    )
    connection.commit()
    connection.close()

    result = _run_launcher(repo_root, env={})

    assert result["argv"] == [
        "dist/index.js",
        "--runtime-attach-artifact-path",
        "runtime-data/TRACE_RESUME_MANIFEST.json",
    ]


def test_launcher_auto_discovers_filesystem_resume_manifest_as_canonical_attach_artifact(
    tmp_path: Path,
) -> None:
    repo_root = _prepare_repo(tmp_path)
    manifest_path = (
        repo_root
        / "framework_runtime"
        / "artifacts"
        / "scratch"
        / "run-a"
        / "TRACE_RESUME_MANIFEST.json"
    )
    _write_text(
        manifest_path,
        json.dumps(
            {
                "schema_version": "runtime-resume-manifest-v1",
                "event_transport_path": "/auto/discovered/runtime_event_transports/session__job.json",
                "updated_at": "2026-04-23T00:10:00+00:00",
            }
        )
        + "\n",
    )

    result = _run_launcher(repo_root, env={})

    assert result["argv"] == [
        "dist/index.js",
        "--runtime-attach-artifact-path",
        str(manifest_path.resolve()),
    ]


def test_launcher_auto_discovers_legacy_runtime_scratch_root_as_fallback(tmp_path: Path) -> None:
    repo_root = _prepare_repo(tmp_path)
    manifest_path = (
        repo_root
        / "codex_agno_runtime"
        / "artifacts"
        / "scratch"
        / "legacy-run"
        / "TRACE_RESUME_MANIFEST.json"
    )
    _write_text(
        manifest_path,
        json.dumps(
            {
                "schema_version": "runtime-resume-manifest-v1",
                "event_transport_path": "/auto/discovered/runtime_event_transports/legacy.json",
                "updated_at": "2026-04-23T00:11:00+00:00",
            }
        )
        + "\n",
    )

    result = _run_launcher(repo_root, env={})

    assert result["argv"] == [
        "dist/index.js",
        "--runtime-attach-artifact-path",
        str(manifest_path.resolve()),
    ]


def test_launcher_prefers_attach_artifact_env_over_compatibility_aliases(tmp_path: Path) -> None:
    repo_root = _prepare_repo(tmp_path)

    result = _run_launcher(
        repo_root,
        env={
            "BROWSER_MCP_RUNTIME_ATTACH_ARTIFACT_PATH": "/explicit/attach-artifact.json",
            "BROWSER_MCP_RUNTIME_BINDING_ARTIFACT_PATH": "/compat/binding.json",
            "BROWSER_MCP_RUNTIME_HANDOFF_PATH": "/compat/handoff.json",
        },
    )

    assert result["argv"] == [
        "dist/index.js",
        "--runtime-attach-artifact-path",
        "/explicit/attach-artifact.json",
    ]


def test_launcher_prefers_descriptor_env_over_all_lower_priority_attach_envs(tmp_path: Path) -> None:
    repo_root = _prepare_repo(tmp_path)

    result = _run_launcher(
        repo_root,
        env={
            "BROWSER_MCP_RUNTIME_ATTACH_DESCRIPTOR_PATH": "/explicit/descriptor.json",
            "BROWSER_MCP_RUNTIME_ATTACH_ARTIFACT_PATH": "/explicit/attach-artifact.json",
            "BROWSER_MCP_RUNTIME_BINDING_ARTIFACT_PATH": "/compat/binding.json",
            "BROWSER_MCP_RUNTIME_HANDOFF_PATH": "/compat/handoff.json",
            "BROWSER_MCP_RUNTIME_RESUME_MANIFEST_PATH": "/compat/resume.json",
        },
    )

    assert result["argv"] == [
        "dist/index.js",
        "--runtime-attach-descriptor-path",
        "/explicit/descriptor.json",
    ]


def test_launcher_falls_back_to_plain_start_when_no_attach_input_exists(tmp_path: Path) -> None:
    repo_root = _prepare_repo(tmp_path)

    result = _run_launcher(repo_root, env={}, extra_args=["--headless", "false"])

    assert result["argv"] == [
        "dist/index.js",
        "--headless",
        "false",
    ]


def test_launcher_passes_through_binding_env_when_higher_priority_inputs_are_absent(tmp_path: Path) -> None:
    repo_root = _prepare_repo(tmp_path)

    result = _run_launcher(
        repo_root,
        env={
            "BROWSER_MCP_RUNTIME_BINDING_ARTIFACT_PATH": "/compat/binding.json",
        },
    )

    assert result["argv"] == [
        "dist/index.js",
        "--runtime-binding-artifact-path",
        "/compat/binding.json",
    ]


def test_launcher_passes_through_handoff_env_when_it_is_the_only_attach_input(tmp_path: Path) -> None:
    repo_root = _prepare_repo(tmp_path)

    result = _run_launcher(
        repo_root,
        env={
            "BROWSER_MCP_RUNTIME_HANDOFF_PATH": "/compat/handoff.json",
        },
    )

    assert result["argv"] == [
        "dist/index.js",
        "--runtime-handoff-path",
        "/compat/handoff.json",
    ]


def test_launcher_passes_through_resume_manifest_env_when_it_is_the_only_attach_input(
    tmp_path: Path,
) -> None:
    repo_root = _prepare_repo(tmp_path)

    result = _run_launcher(
        repo_root,
        env={
            "BROWSER_MCP_RUNTIME_RESUME_MANIFEST_PATH": "/compat/resume.json",
        },
    )

    assert result["argv"] == [
        "dist/index.js",
        "--runtime-resume-manifest-path",
        "/compat/resume.json",
    ]


def test_launcher_runs_npm_install_when_node_modules_is_missing(tmp_path: Path) -> None:
    repo_root = _prepare_repo_state(tmp_path, include_node_modules=False)

    result = _run_launcher(repo_root, env={})

    assert result["argv"] == ["dist/index.js"]
    assert result["npm_calls"] == [
        {
            "argv": ["install"],
            "cwd": str(repo_root / "tools" / "browser-mcp"),
        }
    ]


def test_launcher_runs_npm_build_when_sources_are_newer_than_dist(tmp_path: Path) -> None:
    repo_root = _prepare_repo_state(tmp_path, src_newer_than_dist=True)

    result = _run_launcher(repo_root, env={})

    assert result["argv"] == ["dist/index.js"]
    assert result["npm_calls"] == [
        {
            "argv": ["run", "build"],
            "cwd": str(repo_root / "tools" / "browser-mcp"),
        }
    ]


def test_launcher_runs_install_then_build_when_modules_are_missing_and_dist_is_stale(
    tmp_path: Path,
) -> None:
    repo_root = _prepare_repo_state(
        tmp_path,
        include_node_modules=False,
        src_newer_than_dist=True,
    )

    result = _run_launcher(repo_root, env={})

    assert result["argv"] == ["dist/index.js"]
    assert result["npm_calls"] == [
        {
            "argv": ["install"],
            "cwd": str(repo_root / "tools" / "browser-mcp"),
        },
        {
            "argv": ["run", "build"],
            "cwd": str(repo_root / "tools" / "browser-mcp"),
        },
    ]


def test_launcher_runs_npm_install_then_build_when_both_preflight_conditions_apply(tmp_path: Path) -> None:
    repo_root = _prepare_repo_state(
        tmp_path,
        include_node_modules=False,
        src_newer_than_dist=True,
    )

    result = _run_launcher(repo_root, env={})

    assert result["argv"] == ["dist/index.js"]
    assert result["npm_calls"] == [
        {
            "argv": ["install"],
            "cwd": str(repo_root / "tools" / "browser-mcp"),
        },
        {
            "argv": ["run", "build"],
            "cwd": str(repo_root / "tools" / "browser-mcp"),
        },
    ]
