//! The PBC v1 bytecode format.
//!
//! - [`Program`]: a validated constant table and code section (SPEC annex A).
//! - [`Instruction`] and [`decode_at`]: the instruction set and how bytes become
//!   instructions (SPEC B.2 steps 1–3, B.3).
//! - `load`: turning the bytes of a `.pbc` file into a [`Program`]. Not written yet
//!   (Ilia, Phase 1): it goes in a `load` module and returns [`LoadError`].
//!
//! Depends on nothing else in the workspace.
#![forbid(unsafe_code)]

mod instruction;
mod program;

pub use instruction::{DecodeError, Instruction, JUMPDEST, decode_at, encode_all};
pub use program::{LoadError, MAX_CODE_LEN, MAX_CONSTANTS, Program};

/// The version of `SPEC-PBC-v1.md` this implementation follows.
///
/// This is the only place it appears in the code (SPEC annex H.2): `READY` and every
/// report read it from here.
pub const SPEC_VERSION: &str = "1.9.0";
