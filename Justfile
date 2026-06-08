# Rust quality commands
fmt:
    cargo fmt --manifest-path core/router-rs/Cargo.toml

clippy:
    cargo clippy --manifest-path core/router-rs/Cargo.toml --all-targets -- -D warnings

test:
    cargo test --manifest-path core/router-rs/Cargo.toml

test-all:
    cargo test --manifest-path core/router-rs/Cargo.toml
    cargo test --manifest-path core/antigravity/Cargo.toml
    cargo test --test policy_contracts
    cargo test --test host_integration

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

ci: validate-skills test-all
