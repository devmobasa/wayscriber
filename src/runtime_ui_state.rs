//! Seed-guarded runtime UI state authority and persistence coordination.
//!
//! This module deliberately owns no toolbar event routing and performs no file
//! I/O. It is the serialized controller boundary between runtime UI producers
//! and the runtime-state I/O actor added by the storage layer.

mod controller;
mod model;
mod pipeline;
mod preview;
mod recovery;
mod seeds;
mod source_revision;
mod types;

pub(crate) use controller::*;
pub(crate) use model::*;
pub(crate) use pipeline::*;
pub(crate) use preview::*;
pub(crate) use recovery::*;
pub(crate) use seeds::*;
pub(crate) use source_revision::*;
pub(crate) use types::*;

#[cfg(test)]
mod tests;
