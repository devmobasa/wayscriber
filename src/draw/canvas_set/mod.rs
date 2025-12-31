//! Multi-frame canvas management for board modes.

mod pages;
mod set;

pub use pages::{BoardPages, PageDeleteOutcome};
pub use set::CanvasSet;

#[cfg(test)]
mod tests;
