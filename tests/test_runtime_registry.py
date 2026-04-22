from __future__ import annotations

import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_SRC = PROJECT_ROOT / "codex_agno_runtime" / "src"
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))
if str(RUNTIME_SRC) not in sys.path:
    sys.path.insert(0, str(RUNTIME_SRC))

from codex_agno_runtime.host_adapters import DEFAULT_HOST_PEER_SET, get_host_adapter
from codex_agno_runtime.runtime_registry import default_host_peer_set, host_adapter_record


def test_host_adapter_specs_are_materialized_from_runtime_registry() -> None:
    assert tuple(DEFAULT_HOST_PEER_SET) == default_host_peer_set()

    claude_record = host_adapter_record("claude_code_adapter")
    claude_spec = get_host_adapter("claude_code_adapter")
    assert claude_spec.host_id == claude_record["host_id"]
    assert claude_spec.transport == claude_record["transport"]
    assert list(claude_spec.host_capabilities) == claude_record["host_capabilities"]
    assert list(claude_spec.protocol_hints["plugin_hook_manifest_paths"]) == claude_record["protocol_hints"][
        "plugin_hook_manifest_paths"
    ]

    legacy_record = host_adapter_record("codex_desktop_host_adapter", include_legacy_aliases=True)
    legacy_spec = get_host_adapter("codex_desktop_host_adapter", include_legacy_aliases=True)
    assert legacy_spec.protocol_hints["canonical_adapter_id"] == legacy_record["protocol_hints"][
        "canonical_adapter_id"
    ]
