//! The PBC v1 execution engine: one turn of one agent (SPEC annex B).
//!
//! [`run_turn`] runs a [`pbc::Program`] against a [`Memory`] that persists between turns,
//! with a gas budget, and asks a [`Host`] whenever the program senses, acts or draws a
//! random value. It returns a [`TurnResult`]: the gas used, the [`Fault`] if any, the
//! attempted action and the trace.
//!
//! The engine knows nothing of the world, the arena or the network: those are hosts.
//! A turn never allocates after it starts (journal D-008).
//!
//! [`verify()`] is the static verifier (bonus, journal D-014): it proves before any run
//! that a program cannot fault on decoding, jumps or the stacks, and bounds its stack.
#![forbid(unsafe_code)]

mod fault;
pub mod gas;
mod host;
mod ops;
mod stack;
mod turn;
pub mod verify;

pub use fault::Fault;
pub use host::{ActRecord, Action, Host, SenseAnswer};
pub use turn::{CALL_STACK_DEPTH, DATA_STACK_SLOTS, MEMORY_CELLS, Memory, TurnResult, run_turn};
pub use verify::{Rejection, Verified, verify};

#[cfg(test)]
mod tests;
