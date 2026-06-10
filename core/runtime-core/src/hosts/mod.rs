//! Host providers: re-export from host-projection.
//!
//! All hosts/ logic has been migrated to `host-projection`. This module
//! re-exports the public API for backward compatibility with `crate::hosts::*` paths.

pub use host_projection::hosts::*;
