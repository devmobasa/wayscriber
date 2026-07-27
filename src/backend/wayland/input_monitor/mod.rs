//! System-wide input capture for the input HUD (`input-monitor` feature).
//!
//! A Wayland client only ever receives input delivered to its own surfaces, so
//! the HUD's overlay source cannot show what flows to the app underneath during
//! Light Mode passthrough. This module adds the second source: a libinput
//! reader on `/dev/input` that reports everything on the seat.
//!
//! The capability probe is always compiled so mode resolution and
//! `--runtime-capabilities` answer the same question in every build; the reader
//! thread itself is behind the feature.

mod probe;

#[cfg(feature = "input-monitor")]
mod monitor;
#[cfg(feature = "input-monitor")]
mod translate;

pub(crate) use probe::system_input_available;
#[cfg(feature = "input-monitor")]
pub(in crate::backend::wayland) use probe::{EventNodeAccess, current_seat, event_node_access};

#[cfg(feature = "input-monitor")]
pub(in crate::backend::wayland) use monitor::{InputMonitor, SystemInputEvent, SystemInputFailure};
