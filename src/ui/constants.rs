//! Legacy shim over the canonical theme tokens.
//!
//! Values and helpers moved to [`crate::ui::theme`] (M1). Every name that
//! existed here is re-exported unchanged so call sites keep compiling; new
//! code should use `theme::` directly.

#![allow(dead_code)] // Some constants reserved for future use

pub use super::theme::overlay::*;
pub use super::theme::{lerp_color, set_color, set_color_alpha, with_alpha};
