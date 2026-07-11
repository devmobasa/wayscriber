mod backend;
mod capture;
mod clipboard;
mod frozen;
mod frozen_geometry;
mod handlers;
mod overlay_passthrough;
mod portal_capture;
mod session;
mod state;
mod surface;
#[cfg(tablet)]
mod tablet_types;
mod toolbar;
mod toolbar_intent;

// The GTK toolbar frontend reuses the width-degradation plan so both
// frontends overflow identically.
#[cfg(feature = "toolbar-gtk")]
pub(crate) use toolbar::view::top::{TopStripPlan, plan_top_strip};
mod zoom;

pub use backend::WaylandBackend;
#[cfg(tablet)]
pub use tablet_types::TabletToolType;
