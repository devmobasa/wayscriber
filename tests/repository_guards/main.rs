//! Source-level contracts that keep ownership boundaries reviewable.
//!
//! These read the repository's sources rather than compiled items, so they
//! are guardrails over spelling, not proofs; each module says what its guard
//! can and cannot see. They run with every `cargo test`, so they need no tool
//! beyond Cargo, and each has regression cases showing its escapes fail.

mod config_writers;
mod no_python;
mod process_sites;
mod shared_dependencies;
mod source;
