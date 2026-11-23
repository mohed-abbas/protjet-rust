//! The PBC v1 execution engine: one turn of one agent (SPEC annex B).
//!
//! This first part holds the types the other crates code against: the runtime
//! [`Fault`]s and the [`Host`] trait through which the program senses, acts and draws
//! random values. The engine itself (`run_turn`) comes next.
#![forbid(unsafe_code)]

mod fault;
mod host;

pub use fault::Fault;
pub use host::{ActRecord, Action, Host, SenseAnswer};
