//! The remote executor (SPEC annex K).
//!
//! Frame encoding and decoding, the per-connection state machine
//! (`Greeting -> Idle <-> Executing`) and the TCP server behind
//! `palestre exec --listen <addr>`. The server holds no world: it forwards
//! `SENSE`, `ACT` and `RAND` to the client as callbacks.
#![forbid(unsafe_code)]
