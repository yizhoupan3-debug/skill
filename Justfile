# Rust quality commands
fmt:
    cargo fmt --manifest-path scripts/router-rs/Cargo.toml

clippy:
    cargo clippy --manifest-path scripts/router-rs/Cargo.toml --all-targets -- -D warnings

test:
    cargo test --manifest-path scripts/router-rs/Cargo.toml

test-all:
    cargo test --manifest-path scripts/router-rs/Cargo.toml
    cargo test --test policy_contracts
    cargo test --test host_integration

audit:
    cargo deny --manifest-path scripts/router-rs/Cargo.toml check

check: fmt clippy test

compile-skills:
    cargo run --manifest-path scripts/skill-compiler-rs/Cargo.toml -- \
        --skills-root skills \
        --source-manifest skills/SKILL_SOURCE_MANIFEST.json \
        --apply

sync-entrypoints:
    cargo run --manifest-path scripts/router-rs/Cargo.toml -- \
        framework sync-entrypoints --repo-root "{{PWD}}"

publish:
    ROUTER_RS_UPDATE_PUBLISH_HOST_SKILLS=1 cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework maint update-one-shot

doctor:
    cargo run --manifest-path scripts/router-rs/Cargo.toml -- framework doctor --repo-root "{{PWD}}"

ci: compile-skills test-all
