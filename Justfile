# Rust quality commands
fmt:
    cargo fmt --manifest-path core/router-rs/Cargo.toml

fmt-all:
    cargo fmt --workspace --check

clippy:
    cargo clippy --manifest-path core/router-rs/Cargo.toml --all-targets -- -D warnings

clippy-all:
    cargo clippy --workspace --all-targets -- -D warnings

test:
    cargo test --manifest-path core/router-rs/Cargo.toml

test-all:
    cargo test --manifest-path core/router-rs/Cargo.toml
    cargo test --test policy_contracts
    cargo test --test host_integration
    cargo test --test browser_mcp_scripts
    cargo test --test rust_cli_tools
    cargo test --manifest-path core/runtime-core/Cargo.toml --lib
    cargo test --manifest-path core/host-projection/Cargo.toml --lib
    cargo test --manifest-path core/core-state/Cargo.toml --lib
    cargo test --manifest-path core/framework-kernel/Cargo.toml --lib
    cargo test --manifest-path core/routing-engine/Cargo.toml --lib
    cargo test --manifest-path core/loop-engine/Cargo.toml --lib
    cargo test --manifest-path core/core-policy/Cargo.toml --lib
    cargo test --manifest-path core/fr-exec/Cargo.toml --lib
    cargo test --manifest-path core/runtime-core-contracts/Cargo.toml --lib
    cargo test --manifest-path core/research-harness/Cargo.toml --lib

test-workspace:
    cargo test --workspace

# --- Performance benchmarks ---

bench:
    SEARCH_BENCH=1 cargo bench --manifest-path core/router-rs/Cargo.toml --bench search_bench

bench-all:
    SEARCH_BENCH=1 cargo bench --manifest-path core/router-rs/Cargo.toml
    cargo bench --manifest-path rust_tools/pdf_tool_rs/Cargo.toml

# --- Debug / analysis ---

miri:
    cargo +nightly miri test --lib -p core-state -p framework-kernel -p core-policy

coverage:
    cargo llvm-cov --workspace --lcov --output-path lcov.info

memory-profile:
    @echo "TODO: implement bench-memory.sh — memory profiling not yet available" >&2
    exit 1

audit:
    cargo deny --manifest-path core/router-rs/Cargo.toml check

check: fmt clippy test

validate-skills:
    cargo run --manifest-path core/router-rs/Cargo.toml -- \
        framework skills validate --framework-root "{{PWD}}"

compile-skills:
    cargo run --manifest-path core/router-rs/Cargo.toml -- \
        framework skills refresh --framework-root "{{PWD}}" --write

sync-entrypoints:
    cargo run --manifest-path core/router-rs/Cargo.toml -- \
        framework sync-entrypoints --repo-root "{{PWD}}"

publish:
    ROUTER_RS_UPDATE_PUBLISH_HOST_SKILLS=1 cargo run --manifest-path core/router-rs/Cargo.toml -- framework maint update-one-shot

doctor:
    cargo run --manifest-path core/router-rs/Cargo.toml -- framework doctor --repo-root "{{PWD}}"

install-pdf:
    bash scripts/install-pdf-tool.sh

install-ooxml:
    bash scripts/install-ooxml-tool.sh

install-ppt:
    bash scripts/install-ppt-tool.sh

install-office-tools: install-pdf install-ooxml install-ppt

# Remove workspace Rust build trees (/tmp/skill-cargo-target via .cargo/config.toml) and repo-local target dirs.
clean:
    cargo clean
    cargo clean --manifest-path rust_tools/Cargo.toml
    cargo clean --manifest-path core/router-rs/Cargo.toml
    rm -rf target target-router-rs-subagent

ci: validate-skills test-all test-workspace
