//! Icon drawing functions for the toolbar UI.
//!
//! All icons are drawn using Cairo paths for perfect scaling at any DPI.

mod actions;
mod controls;
mod history;
mod security;
mod tools;
mod zoom;

pub use actions::*;
pub use controls::*;
pub use history::*;
pub use security::*;
pub use tools::*;
pub use zoom::*;
