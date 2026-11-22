//! The local arena (SPEC annex E).
//!
//! The grid, the agents, the turn loop with its snapshot and rollback (E.2), sensors
//! (E.3), actions (E.4) and the end-of-game rules (E.5). This is where the MVP match
//! between two agents runs.
#![forbid(unsafe_code)]
