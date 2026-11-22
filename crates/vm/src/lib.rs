//! The PBC execution engine.
//!
//! Runs one agent for one turn (SPEC annex B and D): data stack, call stack, memory,
//! gas and the 16 faults. It does no I/O and knows nothing of the world or the
//! network: `SENSE`, `ACT` and `RAND` are answered by whoever runs it (`arena` locally,
//! `exec` over the network).
#![forbid(unsafe_code)]
