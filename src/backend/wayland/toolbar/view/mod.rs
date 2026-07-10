//! Declarative view engine for the toolbars.
//!
//! One pure build pass per bar produces a [`WidgetTree`] of typed
//! [`WidgetNode`]s in logical coordinates. A single painter draws the tree
//! and the hit-tester, focus traversal, tooltips, and drag routing all query
//! the same nodes — so every rect exists in exactly one place. This replaces
//! the historical split where render code pushed hit regions while drawing
//! and a parallel test-only builder re-derived the same geometry.
//!
//! Coordinates: trees are always built in logical units; the surface layer
//! applies one uniform scale when mapping input in and rects out. No
//! per-node coordinate fixups exist by design.

pub mod node;
pub mod popover;
pub mod top;
pub mod tree;

pub use node::{ButtonStyle, WidgetKind, WidgetNode};
pub use tree::WidgetTree;
