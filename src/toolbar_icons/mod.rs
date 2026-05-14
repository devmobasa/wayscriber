//! Icon drawing functions for the toolbar UI.
//!
//! All toolbar icons are procedural Cairo paths. The `svg` module name is kept
//! for the tool icon call sites that used to render embedded SVG files.

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
