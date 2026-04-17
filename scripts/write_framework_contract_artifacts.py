#!/usr/bin/env python3
"""Write framework-profile adapter and compatibility artifacts."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "codex_agno_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from codex_agno_runtime.framework_profile import FrameworkProfile
from codex_agno_runtime.profile_artifacts import emit_framework_contract_artifacts
from codex_agno_runtime.rust_router import RustRouteAdapter


def main() -> int:
    parser = argparse.ArgumentParser(description="Write framework contract artifacts.")
    parser.add_argument("--framework-profile", type=Path, required=True, help="Input framework_profile JSON.")
    parser.add_argument("--output-dir", type=Path, required=True, help="Output directory for emitted artifacts.")
    parser.add_argument(
        "--include-rust-bundle",
        action="store_true",
        help="Also compile the Rust-side profile bundle via router-rs.",
    )
    parser.add_argument(
        "--include-legacy-alias-artifact",
        action="store_true",
        help="Force legacy codex_desktop_host_adapter artifacts to be written alongside the parity-first defaults.",
    )
    args = parser.parse_args()

    profile = FrameworkProfile.from_dict(
        json.loads(args.framework_profile.read_text(encoding="utf-8"))
    )
    rust_adapter = RustRouteAdapter(PROJECT_ROOT) if args.include_rust_bundle else None
    paths = emit_framework_contract_artifacts(
        args.output_dir,
        profile=profile,
        rust_adapter=rust_adapter,
        include_legacy_alias_artifact=args.include_legacy_alias_artifact,
    )
    print(json.dumps(paths, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
