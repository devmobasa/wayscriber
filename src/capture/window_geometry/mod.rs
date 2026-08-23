//! Compositor window geometry queries for the native region picker.
//!
//! Queries are synchronous because the process broker is synchronous. Callers
//! must run [`query_window_targets`] from an off-event-loop worker.

use std::ffi::OsStr;

use crate::process_broker::ProcessBroker;

mod geometry;
mod hyprland;
mod query;
mod sway;
mod types;

pub(crate) use query::query_window_targets;
pub(crate) use types::{WindowGeometryError, WindowQueryContext, WindowQueryResult, WindowTarget};

#[cfg(test)]
pub(in crate::capture) use hyprland::parse_targets as parse_hyprland_targets;
#[cfg(test)]
pub(in crate::capture) use sway::parse_targets as parse_sway_targets;

/// Compositor backend used to discover windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowGeometryBackend {
    Hyprland,
    Sway,
}

/// Synchronous compositor window query executed by the picker worker.
pub(crate) trait WindowGeometryProvider {
    fn backend(&self) -> WindowGeometryBackend;

    fn query(
        &self,
        broker: &ProcessBroker,
        context: &WindowQueryContext,
    ) -> Result<Vec<WindowTarget>, WindowGeometryError>;
}

/// Detect a supported provider from already-captured environment values.
///
/// Hyprland wins when both markers are present because nested compositor test
/// sessions can inherit the outer Sway socket.
pub(crate) fn detect_backend_from_env(
    hyprland_signature: Option<&OsStr>,
    sway_socket: Option<&OsStr>,
) -> Option<WindowGeometryBackend> {
    let present = |value: Option<&OsStr>| value.is_some_and(|value| !value.is_empty());
    if present(hyprland_signature) {
        Some(WindowGeometryBackend::Hyprland)
    } else if present(sway_socket) {
        Some(WindowGeometryBackend::Sway)
    } else {
        None
    }
}

/// Detect the current compositor from its session environment marker.
pub(crate) fn detect_backend() -> Option<WindowGeometryBackend> {
    let hyprland = std::env::var_os(crate::env_vars::HYPRLAND_INSTANCE_SIGNATURE_ENV);
    let sway = std::env::var_os(crate::env_vars::SWAYSOCK_ENV);
    detect_backend_from_env(hyprland.as_deref(), sway.as_deref())
}
