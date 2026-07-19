//! Legacy shim over the canonical theme tokens.
//!
//! Values moved to [`crate::ui::theme::toolbar`] (M1). Every name that
//! existed here is re-exported unchanged so call sites keep compiling; new
//! code should use `theme::` directly.

#![allow(dead_code)] // Some constants are reserved for future use

pub use crate::ui::theme::set_color;
pub use crate::ui::theme::toolbar::*;
