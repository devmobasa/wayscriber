//! Seed-guarded runtime UI state authority and persistence coordination.
//!
//! This module deliberately owns no toolbar event routing. It contains the
//! serialized controller boundary plus the isolated runtime-state wire, store,
//! and writer machinery used to exercise that boundary before UI integration.

mod controller;
mod model;
mod pipeline;
mod preview;
mod recovery;
mod seeds;
mod source_revision;
mod store;
mod types;
mod wire;
mod writer;

pub(crate) use crate::config::ToolbarItemVisibilitySetting as ItemVisibilitySetting;

pub(crate) use controller::*;
pub(crate) use model::*;
pub(crate) use pipeline::*;
pub(crate) use preview::*;
pub(crate) use recovery::*;
pub(crate) use seeds::*;
pub(crate) use source_revision::*;
pub(crate) use store::*;
pub(crate) use types::*;
pub(crate) use wire::*;
#[allow(unused_imports)]
pub(crate) use writer::*;

#[cfg(test)]
mod tests;
