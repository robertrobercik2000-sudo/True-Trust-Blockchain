//! Backwards-compatible alias for the Proof-of-Trust consensus module.
//!
//! Historically the project exposed its host-side consensus primitives via a
//! `consensus` module.  The modern implementation lives in `pot.rs`, but a
//! number of downstream tools (including legacy binaries and integration
//! tests) still refer to `quantum_falcon_wallet::consensus::*`.  To keep those
//! callers working without maintaining two separate code paths, this module
//! simply re-exports the types and helpers from the `pot` (Proof-of-Trust)
//! module as well as the optional `pot_node` runtime.
//!
//! Wherever possible new code should import from `crate::pot` / `crate::pot_node`
//! directly; the re-exports here exist purely for compatibility.

pub use crate::pot::*;
pub use crate::pot_node::*;
