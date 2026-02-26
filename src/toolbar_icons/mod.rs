//! Icon drawing functions for the toolbar UI.
//!
//! Tool icons are rendered from embedded SVG files (see `svg` module).
//! Other icons (actions, controls, history, zoom, security) still use
//! procedural Cairo paths.

mod actions;
mod controls;
mod history;
mod security;
pub(crate) mod svg;
mod tools;
mod zoom;

pub use actions::*;
pub use controls::*;
pub use history::*;
pub use security::*;
pub use tools::*;
pub use zoom::*;
