//! The PBC v1 bytecode format.
//!
//! Loads and validates `.pbc` files (SPEC annex A) and decodes instructions (annex B.3).
//! Depends on nothing else in the workspace.
#![forbid(unsafe_code)]

/// The version of `SPEC-PBC-v1.md` this implementation follows.
///
/// This is the only place it appears in the code (SPEC annex H.2): `READY` and every
/// report read it from here.
pub const SPEC_VERSION: &str = "1.9.0";
