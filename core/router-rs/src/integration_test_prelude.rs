//! Re-exports for `main_tests` (crate root stays intentionally thin after CLI split).
//!
//! Each symbol below is actually referenced by at least one test in `main_tests.rs`
//! (see `rg -c '\\b<symbol>\\b' src/main_tests.rs`). Drop the global `allow(unused_imports)`
//! so a future drift (re-export of a truly dead symbol) surfaces as a compile warning
//! rather than rotting silently behind the wildcard pull-in.


