mod acquisition;
mod backend;
mod capture;
mod clipboard;
mod config_edits;
mod frozen;
mod frozen_geometry;
mod handlers;
pub(crate) mod input_monitor;
mod overlay_passthrough;
mod portal_capture;
mod portal_task;
mod runtime_operation;
mod runtime_ui_state;
mod session;
mod state;
mod surface;
#[cfg(feature = "tablet-input")]
mod tablet_types;
mod toolbar;
mod toolbar_intent;

// The GTK toolbar frontend reuses the width-degradation plan so both
// frontends overflow identically.
#[cfg(feature = "toolbar-gtk")]
pub(crate) use state::clamp_floating_axis_offset;
#[cfg(feature = "toolbar-gtk")]
pub(crate) use toolbar::top_size as top_toolbar_size;
#[cfg(all(test, feature = "toolbar-gtk"))]
pub(crate) use toolbar::view::WidgetKind as TopToolbarWidgetKind;
#[cfg(all(test, feature = "toolbar-gtk"))]
pub(crate) use toolbar::view::top::build_top_view as build_top_toolbar_view;
#[cfg(feature = "toolbar-gtk")]
pub(crate) use toolbar::view::top::plan_top_strip;
mod zoom;

pub use backend::WaylandBackend;
pub(crate) use backend::runtime_wake::{RuntimeWakeHandle, RuntimeWakeSource};
pub(in crate::backend::wayland) use runtime_operation::{
    RuntimeOperationController, RuntimeOperationIdSource, RuntimeOperationPoll,
    RuntimeOperationSubmitFailure,
};
#[cfg(feature = "tablet-input")]
pub use tablet_types::TabletToolType;
